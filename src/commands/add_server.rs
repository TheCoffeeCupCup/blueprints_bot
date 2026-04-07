use colored::Colorize as _;

use crate::common::ansi;
use crate::{bot_data, discord, ftp, logging};

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
    interaction_client: discord::InteractionClient<'_>,
) {
    logging::info!("Processing add_server command");

    let components = [
        discord::TextInputBuilder {
            custom_id: "server_name",
            label: "Server name",
            description: Some("Can be set to whatever you want - it's only used for distinguishing servers in Discord."),
            placeholder: Some("NAC-2"),
        }
        .build(),
        discord::TextInputBuilder {
            custom_id: "full_ip",
            label: "Full IP address",
            description: Some("IP and FTP port must be combined with a colon (:)."),
            placeholder: Some("192.168.0.1:21"),
        }
        .build(),
        discord::TextInputBuilder {
            custom_id: "ftp_username",
            label: "FTP username",
            description: None,
            placeholder: None,
        }
        .build(),
        discord::TextInputBuilder {
            custom_id: "ftp_password",
            label: "FTP password",
            description: None,
            placeholder: None,
        }
        .build(),
        discord::TextInputBuilder {
            custom_id: "session_name",
            label: "Satisfactory session name",
            description: Some(
                "The name of the folder under `.config/Epic/FactoryGame/Saved/SaveGames/blueprints/`.",
            ),
            placeholder: None,
        }
        .build(),
    ];

    logging::info!("Displaying a modal for server adding");

    let data = discord::InteractionResponseDataBuilder::new()
        .title("Add a server for blueprint uploading")
        .custom_id(MODAL_ID)
        .flags(discord::MessageFlags::IS_COMPONENTS_V2)
        .components(components)
        .build();

    let response = discord::InteractionResponse {
        kind: discord::InteractionResponseType::Modal,
        data: Some(data),
    };

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .map_err(|err| {
            logging::error!("Couldn't respond to add_server command: {err}");
        })
        .ok();

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
                logging::warning!("IP \"{full_ip}\" failed regex check");

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

pub async fn process_modal_submition(
    interaction: &discord::InteractionCreate,
    submit_data: &discord::ModalInteractionData,
    interaction_client: discord::InteractionClient<'_>,
) {
    logging::info!("Server adding modal submitted");

    let server_modal_data = ServerCredentialsModalData::from_modal_submit_data(submit_data);

    let mut matched_server_name: Option<String> = None;
    let mut matched_server_creds: Option<bot_data::ServerCredentials> = None;

    let response = match server_modal_data.to_server_creds() {
        Ok((server_name, server_creds)) => {
            logging::info!("Showing loading for server adding response");

            matched_server_name = Some(server_name.clone());
            matched_server_creds = Some(server_creds.clone());

            discord::InteractionResponse {
                kind: discord::InteractionResponseType::DeferredChannelMessageWithSource,
                data: None,
            }
        }
        Err(err) => {
            logging::info!("Server adding rejected");

            let response_data = discord::InteractionResponseDataBuilder::new()
                .content(ansi(err.red().to_string()))
                .flags(discord::MessageFlags::EPHEMERAL)
                .build();

            discord::InteractionResponse {
                kind: discord::InteractionResponseType::ChannelMessageWithSource,
                data: Some(response_data),
            }
        }
    };

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .map_err(|err| {
            logging::error!("Couldnt't send response to add_server modal submission: {err}");
        })
        .ok();

    logging::info!("Responded to a server adding modal");

    if let (Some(server_name), Some(server_creds)) = (matched_server_name, matched_server_creds) {
        let connection_result =
            ftp::establish_ftp_connection(server_name.clone(), server_creds.clone()).await;

        let updated_content = match connection_result {
            Ok(_) => {
                logging::info!(
                    "Adding server \"{server_name}\" IP: {}",
                    server_creds.full_ip
                );

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
        interaction_client
            .update_response(&interaction.token)
            .content(Some(&ansi(updated_content)))
            .await
            .map_err(|err| {
                logging::error!(
                    "Couldnt't send updated response to add_server modal submission: {err}"
                );
            })
            .ok();
    }
}
