use colored::Colorize as _;
use itertools::Itertools;

use crate::{ansi, bot_data, commands, common, discord, logging};

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
    interaction_client: discord::InteractionClient<'_>,
) {
    logging::info!("Processing command `/{COMMAND}`");
    process_command_impl(interaction, interaction_client).await;
    logging::info!("Finished processing command `/{COMMAND}`");
}

pub async fn process_modal_submition(
    interaction: &discord::InteractionCreate,
    submit_data: &discord::ModalInteractionData,
    interaction_client: discord::InteractionClient<'_>,
) {
    logging::info!("Processing modal `{MODAL_ID}`");
    process_modal_submition_impl(interaction, submit_data, interaction_client).await;
    logging::info!("Finished processing modal `{MODAL_ID}`");
}

/* Impl functions */

async fn process_command_impl(
    interaction: &discord::InteractionCreate,
    interaction_client: discord::InteractionClient<'_>,
) {
    let servers_amount = bot_data::get_data().servers.len();

    if servers_amount == 0 {
        logging::info!("`/{COMMAND}` issued while amount of servers is 0");

        let error = format!(
            "✗ No servers have been set up yet. It can be done via the `/{}` command.",
            commands::add_server::COMMAND
        );
        discord::negative_response(interaction, &interaction_client, &error).await;

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

    let response_data = discord::InteractionResponseDataBuilder::new()
        .title("Select servers for removal")
        .custom_id(MODAL_ID)
        .flags(discord::MessageFlags::IS_COMPONENTS_V2)
        .components([server_select_label])
        .build();

    let interaction_response = discord::InteractionResponse {
        kind: discord::InteractionResponseType::Modal,
        data: Some(response_data),
    };

    logging::info!("Sending response to `/{COMMAND}`");

    interaction_client
        .create_response(interaction.id, &interaction.token, &interaction_response)
        .await
        .map_err(|err| {
            logging::error!("Couldn't display the `{MODAL_ID}` modal: {err}");
        })
        .ok();
}

async fn process_modal_submition_impl(
    interaction: &discord::InteractionCreate,
    submit_data: &discord::ModalInteractionData,
    interaction_client: discord::InteractionClient<'_>,
) {
    let Some(selected_servers) = unwrap_selected_servers(submit_data) else {
        logging::error!("Wrong interaction structure. Sending the error message response.");

        let error = "✗ Unexpected error occured: the modal submit data has wrong format.";
        discord::negative_response(interaction, &interaction_client, error).await;

        return;
    };

    // The function for creating server select menu specifies minimum quantity as 1 so this shouldn't be possible in normal circumstances.
    if selected_servers.is_empty() {
        logging::error!("Selected servers list is empty. Sending the error message response.");

        let error = "✗ Unexpected error occured: no servers were selected.";
        discord::negative_response(interaction, &interaction_client, error).await;

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
                "✓ Succesfully removed the following servers: {}",
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

    let response = ansi(response_lines.into_iter().join("\n\n"));

    let data = discord::InteractionResponseDataBuilder::new()
        .content(&response)
        .build();

    let response = discord::InteractionResponse {
        kind: discord::InteractionResponseType::ChannelMessageWithSource,
        data: Some(data),
    };

    logging::info!("Sending response to `{MODAL_ID}`");

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .map_err(|err| {
            crate::logging::error!("Couldn't respond to the `{MODAL_ID}` modal submission: {err}")
        })
        .ok();
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
