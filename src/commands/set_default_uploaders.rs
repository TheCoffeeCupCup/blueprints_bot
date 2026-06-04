use itertools::Itertools;

use crate::{
    bot_data,
    commands::edit_server_uploaders,
    common::discord,
    discord_utils::{self, IntoMessage as _},
    logging::{self, LogError as _},
};

/* Constants */

pub const COMMAND: &'static str = "set_default_uploaders";
pub const MODAL_ID: &'static str = "set_default_uploaders_modal";

/* Interface functions */

pub fn create_command() -> discord::Command {
    logging::info!("Creating command `/{COMMAND}`");

    discord::CommandBuilder::new(
        COMMAND,
        "Select the mentionables that will be added as uploaders to each newly created server by default",
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
    logging::info!("Processing `{MODAL_ID}` modal submission");
    process_modal_submission_impl(interaction, submit_data, http_client).await;
    logging::info!("Finished processing `{MODAL_ID}` modal submission");
}

/* Impl functions */

async fn process_command_impl(
    interaction: &discord::InteractionCreate,
    http_client: &discord::HttpClient,
) {
    let modal = discord_utils::Modal::new(MODAL_ID, "Set default uploaders", [make_user_select()]);

    discord_utils::InteractionResponse::new(interaction, http_client)
        .show_modal(modal)
        .await
        .log_error();
}

async fn process_modal_submission_impl(
    interaction: &discord::InteractionCreate,
    submit_data: &discord::ModalInteractionData,
    http_client: &discord::HttpClient,
) {
    bot_data::update_data(|bot_data| {
        bot_data.default_uploaders.clear();

        if let Some(selected_mentionables) = submit_data.resolved.as_ref() {
            for user_id in selected_mentionables.users.keys() {
                bot_data
                    .default_uploaders
                    .insert(bot_data::Mentionable::User(*user_id));
            }

            for role_id in selected_mentionables.roles.keys() {
                bot_data
                    .default_uploaders
                    .insert(bot_data::Mentionable::Role(*role_id));
            }
        }
    });

    let new_uploaders_list = if bot_data::get_data().default_uploaders.is_empty() {
        "no default uploaders".to_string()
    } else {
        bot_data::get_data()
            .default_uploaders
            .iter()
            .map(|u| u.to_mention())
            .join(", ")
    };

    logging::info!("Default uploaders set: {new_uploaders_list}");

    discord_utils::InteractionResponse::new(interaction, http_client)
        .send_message(
            format!("✓ Default uploaders list is successfully updated: {new_uploaders_list}.\nAll newly added servers will have the corresponding list of uploaders from the get-go.")
                .into_message(),
        )
        .await
        .log_error();
}

/* Modal functions */

fn make_user_select() -> discord::Component {
    let select_menu =
        edit_server_uploaders::make_user_select(&bot_data::get_data().default_uploaders);

    let label = discord::LabelBuilder::new(
        "Default uploaders",
        discord::Component::SelectMenu(select_menu),
    )
    .build();

    discord::Component::Label(label)
}
