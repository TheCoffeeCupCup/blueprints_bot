use colored::Colorize as _;

use crate::common::ansi;
use crate::discord_utils::IntoMessage;
use crate::logging::LogError;
use crate::{bot_data, discord, discord_utils, ftp, logging};

pub const COMMAND: &'static str = "add_server";
pub const MODAL_ID: &'static str = "add_server_modal";

pub fn create_command() -> discord::Command {
    logging::info!("Creating command `/{COMMAND}`");

    discord::CommandBuilder::new(
        COMMAND,
        "Add a new server for blueprints uploading",
        discord::CommandType::ChatInput,
    )
    .default_member_permissions(discord::Permissions::ADMINISTRATOR)
    .build()
}

pub async fn process_command(
    interaction: &discord::InteractionCreate,
    http_client: &discord::HttpClient,
) {
    logging::info!("Processing add_server command");

    let components = [
        discord_utils::TextInputBuilder::new("server_name", "Server name")
            .description("Can be set to whatever you want - it's only used for distinguishing servers in Discord.")
            .placeholder("NAC-2")
            .build(),
        discord_utils::TextInputBuilder::new("full_ip", "Full IP address")
            .description("IP and FTP port must be combined with a colon (:).")
            .placeholder("192.168.0.1:21")
            .build(),
        discord_utils::TextInputBuilder::new("ftp_username", "FTP username")
            .build(),
        discord_utils::TextInputBuilder::new("ftp_password", "FTP password")
            .build(),
        discord_utils::TextInputBuilder::new("session_name", "Satisfactory session name")
            .description("The name of the folder under `.config/Epic/FactoryGame/Saved/SaveGames/blueprints/`.")
            .build(),
    ];

    logging::info!("Displaying a modal for server adding");

    let modal_title = "Add a server for blueprint uploading";

    discord_utils::InteractionResponse::new(interaction, http_client)
        .show_modal(discord_utils::Modal::new(MODAL_ID, modal_title, components))
        .await
        .log_error();

    logging::info!("Finish processing add_server command");
}

struct ServerCredentialsModalData<'a> {
    pub server_name: Option<&'a str>,
    pub full_ip: Option<&'a str>,
    pub ftp_username: Option<&'a str>,
    pub ftp_password: Option<&'a str>,
    pub session_name: Option<&'a str>,

    pub components_data: &'a Vec<discord::ModalInteractionComponent>,
}

fn push_if_none<T, OptionT>(values: &mut Vec<T>, option: &Option<OptionT>, none_value: T) {
    if option.is_none() {
        values.push(none_value);
    };
}

fn verify_ip(ip: &str) -> bool {
    use std::sync::LazyLock;

    static REGEX: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^(\d{1,3}\.){3}\d{1,3}:\d{1,5}$").unwrap());

    REGEX.is_match(ip)
}

impl<'a> ServerCredentialsModalData<'a> {
    pub fn from_modal_submit_data(modal_submit_data: &'a discord::ModalInteractionData) -> Self {
        let mut result = Self {
            server_name: None,
            full_ip: None,
            ftp_username: None,
            ftp_password: None,
            session_name: None,

            components_data: &modal_submit_data.components,
        };

        for component in &modal_submit_data.components {
            if let discord::ModalInteractionComponent::Label(label) = component {
                if let discord::ModalInteractionComponent::TextInput(text_input) =
                    label.component.as_ref()
                {
                    match text_input.custom_id.as_str() {
                        "server_name" => result.server_name = Some(&text_input.value),
                        "full_ip" => result.full_ip = Some(&text_input.value.trim()),
                        "ftp_username" => result.ftp_username = Some(&text_input.value),
                        "ftp_password" => result.ftp_password = Some(&text_input.value),
                        "session_name" => result.session_name = Some(&text_input.value),
                        _ => {}
                    }
                }
            }
        }

        result
    }

    pub fn to_server_creds(&self) -> Result<(String, bot_data::ServerCredentials), String> {
        let mut missing_list = Vec::new();

        push_if_none(&mut missing_list, &self.server_name, "server_name");
        push_if_none(&mut missing_list, &self.full_ip, "full_ip");
        push_if_none(&mut missing_list, &self.ftp_username, "ftp_username");
        push_if_none(&mut missing_list, &self.ftp_password, "ftp_password");
        push_if_none(&mut missing_list, &self.session_name, "session_name");

        if missing_list.is_empty() {
            let full_ip = self.full_ip.unwrap();

            if !verify_ip(full_ip) {
                logging::info!("IP \"{full_ip}\" failed regex check");

                return Err(format!(
                    "✗ IP address \"{full_ip}\" has wrong format. It should look similar to this: \"192.168.0.1:21\"."
                ));
            }

            let server_creds = bot_data::ServerCredentials {
                connection: bot_data::ConnectionType::FTP,
                full_ip: full_ip.to_string(),
                user: self.ftp_username.unwrap().to_string(),
                password: self.ftp_password.unwrap().to_string(),
                session_name: self.session_name.unwrap().to_string(),
            };

            return Ok((self.server_name.unwrap().to_string(), server_creds));
        } else {
            logging::error!(
                "Couldn't extract add_server data from the modal submission. Missing: {}. Components data: {:?}",
                missing_list.join(", "),
                self.components_data
            );
            return Err("✗ Submitted modal misses necessary data.".to_string());
        }
    }
}

pub async fn process_modal_submission(
    interaction: &discord::InteractionCreate,
    submit_data: &discord::ModalInteractionData,
    http_client: &discord::HttpClient,
) {
    logging::info!("Server adding modal submitted");

    let server_modal_data = ServerCredentialsModalData::from_modal_submit_data(submit_data);

    let mut matched_server_name: Option<String> = None;
    let mut matched_server_creds: Option<bot_data::ServerCredentials> = None;

    match server_modal_data.to_server_creds() {
        Ok((server_name, server_creds)) => {
            logging::info!("Showing loading for server adding response");

            matched_server_name = Some(server_name.clone());
            matched_server_creds = Some(server_creds.clone());

            discord_utils::InteractionResponse::new(interaction, http_client)
                .show_loading()
                .await
                .log_error();
        }
        Err(err) => {
            logging::info!("Server adding rejected");

            discord_utils::InteractionResponse::new(interaction, http_client)
                .send_message(ansi(err.red().to_string()).into_message().ephemeral())
                .await
                .log_error();
        }
    };

    logging::info!("Responded to a server adding modal");

    if let (Some(server_name), Some(server_creds)) = (matched_server_name, matched_server_creds) {
        let connection_result =
            ftp::establish_ftp_connection(server_name.clone(), server_creds.clone(), false).await;

        let updated_content = match connection_result {
            Ok(_) => {
                let full_ip = &server_creds.full_ip;

                logging::info!("Adding server \"{server_name}\" IP: {full_ip}");

                bot_data::update_data(|data| {
                    data.servers
                        .insert(server_name.to_string(), bot_data::Server::new(server_creds));
                });

                format!("✓ Server \"{}\" is successfully added.", server_name)
                    .green()
                    .to_string()
            }
            Err(err) => {
                logging::info!(
                    "Adding server \"{server_name}\" is rejected since connection attempt failed"
                );

                format!("Error adding server \"{server_name}\"\n✗ {err}")
                    .red()
                    .to_string()
            }
        };

        logging::info!("Updating the response to add_server modal submission");

        discord_utils::InteractionResponse::new(interaction, http_client)
            .update(discord_utils::Message::text(ansi(updated_content)))
            .await
            .log_error();
    }
}
