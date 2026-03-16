use colored::Colorize as _;
use twilight_model::guild::Permissions;

use crate::{AnyError, bot_data, common::ansi, discord};

pub const COMMAND: &'static str = "add_server";
pub const MODAL_ID: &'static str = "add_server_modal";

pub fn create_command() -> twilight_model::application::command::Command {
    discord::CommandBuilder::new(
        COMMAND,
        "Adds a new server for blueprints uploading",
        twilight_model::application::command::CommandType::ChatInput,
    )
    .default_member_permissions(Permissions::ADMINISTRATOR)
    .build()
}

async fn respond_to_unauthorized(
    interaction: &discord::InteractionCreate,
    interaction_client: twilight_http::client::InteractionClient<'_>,
) {
    let data = discord::InteractionResponseDataBuilder::new()
        .content(ansi(
            "✗ Only members with administrator-allowed role are allowed to use this command."
                .red()
                .to_string(),
        ))
        .flags(discord::MessageFlags::EPHEMERAL)
        .build();

    let response = discord::InteractionResponse {
        kind: twilight_model::http::interaction::InteractionResponseType::ChannelMessageWithSource,
        data: Some(data),
    };

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .unwrap();
}

pub async fn process_command(
    interaction: &discord::InteractionCreate,
    interaction_client: twilight_http::client::InteractionClient<'_>,
) -> Result<(), AnyError> {
    if !interaction
        .member
        .as_ref()
        .unwrap()
        .permissions
        .unwrap()
        .contains(Permissions::ADMINISTRATOR)
    {
        respond_to_unauthorized(interaction, interaction_client).await;

        return Ok(());
    }

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
            placeholder: Some("123.45.67.89:21"),
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
            custom_id: "world_name",
            label: "Satisfactory world name",
            description: Some(
                "The name of the folder under `.config/Epic/FactoryGame/Saved/SaveGames/blueprints/`.",
            ),
            placeholder: None,
        }
        .build(),
    ];

    let data = discord::InteractionResponseDataBuilder::new()
        .title("Add a server for blueprint uploading")
        .custom_id(MODAL_ID)
        .flags(discord::MessageFlags::IS_COMPONENTS_V2)
        .components(components)
        .build();

    let response = discord::InteractionResponse {
        kind: twilight_model::http::interaction::InteractionResponseType::Modal,
        data: Some(data),
    };

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .unwrap();

    Ok(())
}

pub async fn process_modal_submition(
    interaction: &discord::InteractionCreate,
    submit_data: &discord::ModalInteractionData,
    interaction_client: twilight_http::client::InteractionClient<'_>,
) -> Result<(), AnyError> {
    let mut server_name: Option<&str> = None;

    let mut full_ip: Option<&str> = None;
    let mut ftp_username: Option<&str> = None;
    let mut ftp_password: Option<&str> = None;
    let mut world_name: Option<&str> = None;

    for component in &submit_data.components {
        if let discord::ModalInteractionComponent::Label(label) = component {
            if let discord::ModalInteractionComponent::TextInput(text_input) =
                label.component.as_ref()
            {
                match text_input.custom_id.as_str() {
                    "server_name" => server_name = Some(&text_input.value),
                    "full_ip" => full_ip = Some(&text_input.value),
                    "ftp_username" => ftp_username = Some(&text_input.value),
                    "ftp_password" => ftp_password = Some(&text_input.value),
                    "world_name" => world_name = Some(&text_input.value),
                    _ => {}
                }
            }
        }
    }

    let data = (server_name, full_ip, ftp_password, ftp_username, world_name);

    if let (Some(server_name), Some(full_ip), Some(password), Some(user), Some(world_name)) = data {
        let credentials = bot_data::ServerCredentials {
            connection: bot_data::ConnectionType::FTP,
            full_ip: full_ip.to_string(),
            password: password.to_string(),
            user: user.to_string(),
            world_name: world_name.to_string(),
        };

        bot_data::BOT_DATA
            .lock()
            .unwrap()
            .servers
            .insert(server_name.to_string(), bot_data::Server::new(credentials));

        bot_data::save_data();

        let response_data = discord::InteractionResponseDataBuilder::new()
            .content(ansi(format!(
                "{}",
                format!("✓ Server \"{}\" is successfully added.", server_name).green()
            )))
            .build();

        let response = discord::InteractionResponse {
            kind:
                twilight_model::http::interaction::InteractionResponseType::ChannelMessageWithSource,
            data: Some(response_data),
        };

        interaction_client
            .create_response(interaction.id, &interaction.token, &response)
            .await
            .unwrap();
    } else {
        panic!("add_server misses data from modal submission: {:?}", data)
    }

    Ok(())
}
