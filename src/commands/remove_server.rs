use colored::Colorize as _;
use itertools::Itertools;

use crate::{
    ansi, bot_data, commands, common, discord, discord_utils,
    logging::{self, LogError},
};

/* Constants */

pub const COMMAND: &'static str = "remove_server";
pub const MODAL_ID: &'static str = "remove_server_modal";

/* Interface functions */

pub fn create_command() -> discord::Command {
    logging::info!("Creating command `/{COMMAND}`");

    discord::CommandBuilder::new(
        COMMAND,
        "Remove one or more servers from the previously set up for blueprints uploading",
        discord::CommandType::ChatInput,
    )
    .default_member_permissions(discord::Permissions::ADMINISTRATOR)
    .build()
}

pub async fn process_command(
    interaction: &discord::InteractionCreate,
    http_client: &discord::HttpClient,
) {
    logging::info!("Processing command `/{COMMAND}`");
    process_command_impl(interaction, http_client).await;
    logging::info!("Finished processing command `/{COMMAND}`");
}

pub async fn process_modal_submission(
    interaction: &discord::InteractionCreate,
    submit_data: &discord::ModalInteractionData,
    http_client: &discord::HttpClient,
) {
    logging::info!("Processing modal `{MODAL_ID}`");
    process_modal_submission_impl(interaction, submit_data, http_client).await;
    logging::info!("Finished processing modal `{MODAL_ID}`");
}

/* Impl functions */

async fn process_command_impl(
    interaction: &discord::InteractionCreate,
    http_client: &discord::HttpClient,
) {
    let servers_amount = bot_data::get_data().servers.len();

    if servers_amount == 0 {
        logging::info!("`/{COMMAND}` issued while amount of servers is 0");

        let error = format!(
            "No servers have been set up yet. It can be done via the `/{}` command.",
            commands::add_server::COMMAND
        );
        discord_utils::error_message_response(error, interaction, http_client).await;

        return;
    }

    let server_select_label = discord::Component::Label(
        discord::LabelBuilder::new(
            "Servers to remove",
            discord::Component::SelectMenu(bot_data::create_server_select_menu(None, None, None)),
        )
        .description("You can choose any amount of the servers.")
        .build(),
    );

    logging::info!("Sending response to `/{COMMAND}`");

    discord_utils::InteractionResponse::new(interaction, http_client)
        .show_modal(discord_utils::Modal::new(
            MODAL_ID,
            "Select servers for removal",
            [server_select_label],
        ))
        .await
        .log_error();
}

async fn process_modal_submission_impl(
    interaction: &discord::InteractionCreate,
    submit_data: &discord::ModalInteractionData,
    http_client: &discord::HttpClient,
) {
    let Some(selected_servers) = unwrap_selected_servers(submit_data) else {
        logging::error!("Wrong interaction structure. Sending the error message response.");

        let error = "Unexpected error occurred: the modal submit data has wrong format.";
        discord_utils::error_message_response(error, interaction, http_client).await;

        return;
    };

    // The function for creating server select menu specifies minimum quantity as 1 so this shouldn't be possible in normal circumstances.
    if selected_servers.is_empty() {
        logging::error!("Selected servers list is empty. Sending the error message response.");

        let error = "Unexpected error occurred: no servers were selected.";
        discord_utils::error_message_response(error, interaction, http_client).await;

        return;
    }

    logging::info!("Removing selected servers: {selected_servers:?}");

    let mut removed_servers = Vec::new();
    let mut missing_servers = Vec::new();

    bot_data::update_data(|bot_data| {
        for selected_server_name in selected_servers {
            let removed_server = bot_data.servers.remove(selected_server_name);

            if removed_server.is_some() {
                removed_servers.push(selected_server_name);
            } else {
                missing_servers.push(selected_server_name);
            }
        }
    });

    let mut response_lines = Vec::new();

    if !removed_servers.is_empty() {
        response_lines.push(
            format!(
                "✓ Successfully removed the following servers: {}",
                common::list_to_string(&removed_servers)
            )
            .green(),
        );
    }

    if !missing_servers.is_empty() {
        logging::info!(
            "Servers {missing_servers:?} requested for removal weren't found in the bot data"
        );

        response_lines.push(
            format!(
                "✗ Couldn't remove the following servers: {}",
                common::list_to_string(&missing_servers)
            )
            .red(),
        );
    }

    logging::info!("Sending response to `{MODAL_ID}`");

    let response = ansi(response_lines.into_iter().join("\n\n"));

    discord_utils::InteractionResponse::new(interaction, http_client)
        .send_message(discord_utils::Message::text(response))
        .await
        .log_error();
}

/* Helper functions */

fn unwrap_selected_servers(submit_data: &discord::ModalInteractionData) -> Option<&Vec<String>> {
    let [discord::ModalInteractionComponent::Label(label)] = submit_data.components.as_slice()
    else {
        logging::error!("Missing label component inside the modal submit data");
        return None;
    };

    let discord::ModalInteractionComponent::StringSelect(server_select) = label.component.as_ref()
    else {
        logging::error!("Missing nested string select component inside the modal submit data");
        return None;
    };

    let id = &server_select.custom_id;

    if id != "server_select" {
        logging::error!("String select component doesn't have the expected id: `{id}`");
        return None;
    }

    return Some(&server_select.values);
}
