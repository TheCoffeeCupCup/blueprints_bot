use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{bot_data, commands, common, logging};

/* Static data */

const BLUEPRINTS_BASE_FOLDER: &'static str = ".config/Epic/FactoryGame/Saved/SaveGames/blueprints";

// The limit here is slightly below what we discovered by experiments because the value isn't set in stone.
const BLUEPRINTS_AMOUNT_LIMIT: usize = 600;

/* Functions */

pub async fn upload_files(
    files: Vec<File>,
    servers: Vec<String>,
    overwrite_files: bool,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    let ftp_servers = establish_ftp_connections(&servers, &mut errors).await;

    if ftp_servers.len() == 0 {
        let error = "Didn't connect to any servers, blueprint uploading cancelled".to_string();

        logging::warning!("{error}");
        errors.push(error);

        return Err(errors);
    }

    let forwarding_task = forward_all_files(files, &ftp_servers, overwrite_files);
    if let Err(forwarding_errors) = forwarding_task.await {
        errors.extend(forwarding_errors);
    }

    close_all_connections(ftp_servers).await;

    if errors.is_empty() {
        logging::info!("No errors while uploading files");
        Ok(())
    } else {
        errors.sort();
        Err(errors)
    }
}

pub async fn establish_ftp_connection(
    server_name: String,
    server_creds: bot_data::ServerCredentials,
    check_files_amount: bool,
) -> Result<Server, String> {
    logging::info!("Connecting to FTP server \"{server_name}\"");

    let mut server = Server::establish_connection(&server_name, &server_creds).await?;

    if check_files_amount {
        let files_amount = server.count_files_at_cwd().await?;

        // Each blueprint takes two files.
        let files_limit = BLUEPRINTS_AMOUNT_LIMIT * 2;

        if files_amount > files_limit {
            let basic_error = format!(
                "The amount of blueprints exceeds current limit {BLUEPRINTS_AMOUNT_LIMIT} on the server \"{server_name}\""
            );

            logging::warning!("{basic_error}");

            return Err(format!(
                "{basic_error}. Please ask netrunners to delete some before uploading new files to the server"
            ));
        } else {
            logging::info!("Server \"{server_name}\" has {files_amount} files in the bp folder");
        }
    }

    Ok(server)
}

async fn establish_ftp_connections(
    servers: &Vec<String>,
    errors: &mut Vec<String>,
) -> Vec<Arc<Mutex<Server>>> {
    let mut ftp_servers_tasks = tokio::task::JoinSet::new();

    for server_name in servers {
        let server_creds = match get_server_creds(&server_name) {
            Ok(server_creds) => server_creds,
            Err(err) => {
                errors.push(err);
                continue;
            }
        };

        // SFTP might be needed in future (used on Illuminate's servers).
        if server_creds.connection != bot_data::ConnectionType::FTP {
            let connection_type = &server_creds.connection;
            let error =
                format!("Unknown connection type {connection_type:?} on server \"{server_name}\"");

            logging::error!("{error}");
            errors.push(error);

            continue;
        }

        ftp_servers_tasks.spawn(establish_ftp_connection(
            server_name.clone(),
            server_creds,
            true,
        ));
    }

    let mut ftp_servers = Vec::new();

    for connection_result in ftp_servers_tasks.join_all().await {
        match connection_result {
            Ok(server) => ftp_servers.push(Arc::new(Mutex::new(server))),
            Err(err) => errors.push(err),
        };
    }

    ftp_servers
}

async fn forward_file(
    file: File,
    servers: Vec<Arc<Mutex<Server>>>,
    overwrite: bool,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    let bytes = match download_file(&file).await {
        Ok(bytes) => Arc::new(bytes),
        Err(err) => {
            errors.push(err);
            return Err(errors);
        }
    };

    let file_name = &file.filename;
    let file_size = common::file_size_to_string(bytes.len());
    logging::info!("Forwarding \"{file_name}\" ({file_size})");

    let mut upload_tasks = tokio::task::JoinSet::new();
    let file = Arc::new(file);

    for server in servers {
        let bytes = Arc::clone(&bytes);
        let file_info = Arc::clone(&file);

        upload_tasks.spawn(async move {
            let mut server = server.lock().await;
            server.upload_file(&file_info, &bytes, overwrite).await
        });
    }

    for upload_result in upload_tasks.join_all().await {
        if let Err(err) = upload_result {
            errors.push(err);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

async fn download_file(file: &File) -> Result<Vec<u8>, String> {
    let file_url = &file.url;
    let file_name = &file.filename;

    logging::info!("Downloading \"{file_name}\"");

    let file_response = reqwest::get(file_url).await.map_err(|err| {
        let error = format!("Couldn't download the attached file \"{file_name}\" from Discord");

        logging::error!("{error} (url: \"{file_url}\"): {err}");
        error
    })?;

    let bytes = file_response.bytes().await.map_err(|err| {
        let error = format!("Couldn't convert attached file \"{file_name}\" to bytes");

        logging::error!("{error} (url: \"{file_url}\"): {err}");
        error
    })?;

    Ok(bytes.to_vec())
}

/* Utility */

async fn forward_all_files(
    files: Vec<File>,
    servers: &Vec<Arc<Mutex<Server>>>,
    overwrite_files: bool,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    let mut file_forward_tasks = tokio::task::JoinSet::new();

    for file in files {
        file_forward_tasks.spawn({
            let ftp_servers = servers.clone();
            forward_file(file, ftp_servers, overwrite_files)
        });
    }

    for forwarding_result in file_forward_tasks.join_all().await {
        if let Err(forwarding_errors) = forwarding_result {
            errors.extend(forwarding_errors);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

async fn close_all_connections(servers: Vec<Arc<Mutex<Server>>>) {
    let mut connection_closing_tasks = tokio::task::JoinSet::new();

    for server in servers {
        connection_closing_tasks.spawn(async move {
            server.lock().await.close_connection().await;
        });
    }

    // If connections don't close before quitting the program,
    // FTP server will be left in less than desirable state.
    connection_closing_tasks.join_all().await;
}

fn get_server_creds(server_name: &str) -> Result<bot_data::ServerCredentials, String> {
    let Some(creds) = bot_data::get_server_creds(server_name) else {
        let creds_error = format!("Error retrieving credentials for the server \"{server_name}\"");

        logging::error!("{creds_error}");
        return Err(creds_error);
    };

    Ok(creds.clone())
}

/* Types */

type FtpStream = suppaftp::tokio::AsyncRustlsFtpStream;
type File = commands::upload_blueprints::Attachment;

pub struct Server {
    pub server_name: String,
    pub ftp_stream: FtpStream,
}

impl Server {
    async fn establish_connection(
        server_name: &str,
        credentials: &bot_data::ServerCredentials,
    ) -> Result<Self, String> {
        let mut server = Server::connect(&server_name, &credentials.full_ip).await?;

        server
            .login(&credentials.user, &credentials.password)
            .await?;

        let base_path = BLUEPRINTS_BASE_FOLDER;
        let folder_name = &credentials.session_name;

        let folder_path = format!("{base_path}/{folder_name}");

        server.cwd(&folder_path).await?;

        Ok(server)
    }

    async fn connect(server_name: &str, full_ip: &str) -> Result<Self, String> {
        let stream = FtpStream::connect(full_ip).await.map_err(|err| {
            let connection_error = format!("Couldn't connect to the server \"{server_name}\"");

            logging::warning!("{connection_error} via `{full_ip}`: {err}");
            connection_error
        })?;

        Ok(Self {
            server_name: server_name.to_string(),
            ftp_stream: stream,
        })
    }

    async fn login(&mut self, user: &str, password: &str) -> Result<(), String> {
        let server_name = &self.server_name;

        self.ftp_stream.login(user, password).await.map_err(|err| {
            let login_error = format!("Couldn't log into the server \"{server_name}\"");

            logging::warning!("{login_error}: {err}");
            login_error
        })?;

        Ok(())
    }

    async fn count_files_at_cwd(&mut self) -> Result<usize, String> {
        let server_name = &self.server_name;

        self.ftp_stream
        .list(None)
        .await
        .map(|files| files.len())
        .map_err(|err| {
            let counting_error = format!(
                "Couldn't count the files in the blueprints folder on the server \"{server_name}\""
            );

            logging::error!("{counting_error}: {err}");
            counting_error
        })
    }

    async fn cwd(&mut self, folder_path: &str) -> Result<(), String> {
        let server_name = &self.server_name;

        self.ftp_stream.cwd(folder_path).await.map_err(|err| {
            logging::warning!(
                "Couldn't access blueprints folder `{folder_path}` on the server \"{server_name}\": {err}"
            );

            format!("Couldn't access blueprints folder on the server \"{server_name}\"")
        })?;

        Ok(())
    }

    async fn check_file_exists_in_cwd(&mut self, file_name: &str) -> bool {
        self.ftp_stream.size(file_name).await.is_ok()
    }

    async fn upload_file(
        &mut self,
        file_info: &File,
        bytes: &Vec<u8>,
        overwrite: bool,
    ) -> Result<(), String> {
        if !overwrite && self.check_file_exists_in_cwd(&file_info.filename).await {
            let server_name = &self.server_name;
            let file_name = &file_info.filename;

            let error =
                format!("File \"{file_name}\" already exists on the server \"{server_name}\"");

            logging::info!("{error}");
            return Err(error);
        }

        let file_name = &file_info.filename;

        let mut reader = std::io::Cursor::new(bytes);

        self.ftp_stream
            .put_file(file_name, &mut reader)
            .await
            .map_err(|err| {
                let server_name = &self.server_name;
                let file_url = &file_info.url;

                let error =
                    format!("Couldn't upload \"{file_name}\" to the server \"{server_name}\"");

                logging::error!("{error} (url: \"{file_url}\"): {err}");
                error
            })?;

        Ok(())
    }

    async fn close_connection(&mut self) {
        let server_name = &self.server_name;

        self.ftp_stream
            .quit()
            .await
            .map_err(|err| {
                logging::error!("Error closing FTP connection for \"{server_name}\": {err}")
            })
            .ok();
    }
}
