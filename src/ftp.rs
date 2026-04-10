use crate::{bot_data, commands, logging};

type FtpStream = suppaftp::tokio::AsyncRustlsFtpStream;
type File = commands::upload_blueprints::Attachment;

const BLUEPRINTS_BASE_FOLDER: &'static str = ".config/Epic/FactoryGame/Saved/SaveGames/blueprints";

// The limit here is slightly below what we discovered by experiments because the value isn't set in stone.
const BLUEPRINTS_AMOUNT_LIMIT: usize = 600;

pub async fn establish_ftp_connection(
    server_name: String,
    server_creds: bot_data::ServerCredentials,
    check_files_amount: bool,
) -> Result<FtpStream, String> {
    logging::info!("Connecting to FTP \"{server_name}\"");

    let full_ip = &server_creds.full_ip;
    let mut ftp_stream = FtpStream::connect(full_ip).await.map_err(|err| {
        logging::warning!(
            "Couldn't connect to the server \"{server_name}\" via `{full_ip}`: {err}"
        );
        format!("Couldn't connect to the server \"{server_name}\".")
    })?;

    ftp_stream
        .login(&server_creds.user, &server_creds.password)
        .await
        .map_err(|err| {
            logging::warning!("Couldn't log into the server \"{server_name}\": {err}");
            format!("Couldn't log into the server \"{server_name}\".")
        })?;

    let folder_path = &format!("{}/{}", BLUEPRINTS_BASE_FOLDER, server_creds.session_name);

    ftp_stream.cwd(folder_path).await.map_err(|err| {
        logging::warning!(
            "Couldn't access blueprints folder `{folder_path}` on the server \"{server_name}\": {err}"
        );
        format!("Couldn't access blueprints folder on the server \"{server_name}\".")
    })?;

    if check_files_amount {
        let files_amount = ftp_stream
        .list(None)
        .await
        .map(|files| files.len())
        .map_err(|err| {
            logging::error!("Couldn't count the files in the blueprints folder on the server \"{server_name}\": {err}");
            format!("Couldn't count the files in the blueprints folder on the server \"{server_name}\".")
        })?;

        // Each blueprint takes two files.
        let files_limit = BLUEPRINTS_AMOUNT_LIMIT * 2;

        if files_amount > files_limit {
            let basic_error = format!(
                "The amount of blueprints exceeds current limit {BLUEPRINTS_AMOUNT_LIMIT} on the server \"{server_name}\""
            );

            logging::warning!("{basic_error}");
            return Err(format!(
                "{basic_error}. Please ask netrunners to delete some before uploading new files to the server."
            ));
        }
    }

    logging::info!("Connected to FTP \"{server_name}\"");

    Ok(ftp_stream)
}

async fn establish_ftp_connections(
    servers: &Vec<String>,
    errors: &mut Vec<String>,
) -> Vec<FtpStream> {
    let mut ftp_servers_tasks = tokio::task::JoinSet::new();

    for server_name in servers {
        let Some(server_creds) = bot_data::get_server_creds(&server_name) else {
            logging::error!(
                "Couldn't connect to the server \"{server_name}\" - credentials not found in bot data"
            );

            errors.push(format!(
                "⚠ Error retrieving credentials for the server \"{server_name}\"."
            ));

            continue;
        };

        // SFTP might be needed in future (used on Illuminate's servers).
        if server_creds.connection != bot_data::ConnectionType::FTP {
            logging::error!(
                "Unknown server connection type {:?}",
                server_creds.connection
            );

            errors.push(format!(
                "⚠ Unknown connection type on server \"{server_name}\"."
            ));

            continue;
        }

        ftp_servers_tasks.spawn(establish_ftp_connection(
            server_name.clone(),
            server_creds,
            true,
        ));
    }

    let mut ftp_servers = Vec::<FtpStream>::new();

    for connection_result in ftp_servers_tasks.join_all().await {
        match connection_result {
            Ok(ftp_stream) => ftp_servers.push(ftp_stream),
            Err(err) => errors.push(format!("⚠ {err}")),
        };
    }

    ftp_servers
}

async fn forward_files(
    files: &Vec<File>,
    ftp_servers: &mut Vec<FtpStream>,
    errors: &mut Vec<String>,
) {
    logging::info!("Forwarding files");

    for file in files {
        let file_url = &file.url;
        let file_name = &file.filename;

        logging::info!("Downloading `{file_name}`");
        let file_response = reqwest::get(file_url)
            .await
            .map_err(|err| {
                logging::warning!(
                    "Couldn't download the attached file \"{file_url}\" from Discord: {err}"
                );

                errors.push(format!(
                    "⚠ Couldn't download the attached file \"{file_name}\" from Discord."
                ));
            })
            .expect("Infallible");

        let bytes = file_response
            .bytes()
            .await
            .map_err(|err| {
                logging::warning!("Couldn't convert attached file \"{file_url}\" to bytes: {err}");

                errors.push(format!(
                    "⚠ Couldn't convert attached file \"{file_name}\" to bytes."
                ));
            })
            .expect("Infallible");

        let mut reader = std::io::Cursor::new(bytes);

        logging::info!("Uploading `{file_name}`");
        for server in ftp_servers.iter_mut() {
            server
                .put_file(file.filename.clone(), &mut reader)
                .await
                .map_err(|err| {
                    logging::warning!(
                        "Couldn't upload attached file \"{file_url}\" to a server: {err}"
                    );

                    errors.push(format!(
                        "⚠ Couldn't upload attached file \"{file_name}\" to a server."
                    ));
                })
                .expect("Infallible");
        }
    }

    logging::info!("Files forwarded");
}

pub async fn upload_files(files: Vec<File>, servers: Vec<String>) -> Result<(), Vec<String>> {
    logging::info!("Uploading files");

    let mut errors = Vec::<String>::new();

    let mut ftp_servers = establish_ftp_connections(&servers, &mut errors).await;

    forward_files(&files, &mut ftp_servers, &mut errors).await;

    for server in &mut ftp_servers {
        server
            .quit()
            .await
            .map_err(|err| logging::error!("Error closing FTP connection: {err}"))
            .ok();
    }

    logging::info!("Uploading files done");

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
