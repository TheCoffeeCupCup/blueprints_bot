use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

use colored::Colorize as _;
use itertools::Itertools;
use tokio::sync::Mutex;

use crate::{bot_data, commands, common::ansi, discord, logging};

pub const COMMAND: &'static str = "edit_server_uploaders";

pub fn create_command() -> discord::Command {
    discord::CommandBuilder::new(
        COMMAND,
        "Modify the list of uploaders for one of the available servers",
        discord::CommandType::ChatInput,
    )
    .default_member_permissions(discord::Permissions::ADMINISTRATOR)
    .build()
}

struct MessageFormData {
    pub command_interaction_token: String,
    pub server_name: Option<String>,
    pub selected_users: Option<HashSet<bot_data::Mentionable>>,
}

impl MessageFormData {
    fn new(token: String) -> Self {
        Self {
            command_interaction_token: token,
            server_name: None,
            selected_users: None,
        }
    }
}

static ACTIVE_FORMS: LazyLock<
    Mutex<HashMap<discord::Id<discord::marker::MessageMarker>, MessageFormData>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub async fn process_command(
    interaction: &discord::InteractionCreate,
    interaction_client: discord::InteractionClient<'_>,
) {
    let servers_amount = bot_data::get_data().servers.len();

    if servers_amount == 0 {
        logging::info!("Edit server uploaders command issued while amount of servers is 0");

        let error = format!(
            "✗ No servers have been set up yet. It can be done via the `/{}` command.",
            commands::add_server::COMMAND
        );
        discord::negative_response(interaction, interaction_client, &error).await;

        return;
    }

    let data = discord::InteractionResponseDataBuilder::new()
        .content("Waiting for settings submition...")
        .build();

    let response = discord::InteractionResponse {
        kind: discord::InteractionResponseType::ChannelMessageWithSource,
        data: Some(data),
    };

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .map_err(|err| {
            logging::error!(
                "Couldn't send \"Waiting for submission\" message for edit server uploaders: {err}"
            );
        })
        .ok();

    let followup_result = interaction_client
        .create_followup(&interaction.token)
        .components(construct_message_components(None).as_slice())
        .flags(discord::MessageFlags::IS_COMPONENTS_V2 | discord::MessageFlags::EPHEMERAL)
        .await;

    match followup_result {
        Ok(response) => match response.model().await.map(|followup| followup.id) {
            Ok(followup_id) => {
                ACTIVE_FORMS
                    .lock()
                    .await
                    .insert(followup_id, MessageFormData::new(interaction.token.clone()));
            }
            Err(err) => {
                logging::error!("Couldn't retrieve followup id: {err}");
            }
        },
        Err(err) => {
            logging::error!("Couldn't send edit server uploaders followup: {err}");
        }
    }

    logging::info!("Responded to edit server uploaders command");
}

pub async fn process_server_select(
    interaction: &discord::InteractionCreate,
    interaction_data: &discord::MessageComponentInteractionData,
    interaction_client: discord::InteractionClient<'_>,
) {
    let mut server_name: Option<&str> = None;

    if let [selected_server_name] = interaction_data.values.as_slice() {
        server_name = Some(&selected_server_name);
    }

    let data = match server_name {
        Some(server_name) => {
            if let Some(interaction_message) = &interaction.message {
                logging::info!("Updating server name form data for edit server uploaders.");

                if let Some(form_data) = ACTIVE_FORMS.lock().await.get_mut(&interaction_message.id)
                {
                    form_data.server_name = Some(server_name.to_string());
                }
            }

            let components = construct_message_components(Some(server_name));

            discord::InteractionResponseDataBuilder::new()
                .flags(discord::MessageFlags::IS_COMPONENTS_V2 | discord::MessageFlags::EPHEMERAL)
                .components(components)
                .build()
        }
        None => {
            logging::error!("Couldn't retrieve selected server name.");

            let mut components = construct_message_components(None);

            components.push(discord::Component::TextDisplay(
                discord::TextDisplayBuilder::new(ansi(
                    "⚠ Something went wrong. Please try again."
                        .red()
                        .to_string(),
                ))
                .build(),
            ));

            discord::InteractionResponseDataBuilder::new()
                .flags(discord::MessageFlags::IS_COMPONENTS_V2 | discord::MessageFlags::EPHEMERAL)
                .components(components)
                .build()
        }
    };

    let response = discord::InteractionResponse {
        kind: discord::InteractionResponseType::UpdateMessage,
        data: Some(data),
    };

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .map_err(|err| {
            logging::error!("Couldn't update edit server uploaders message: {err}");
        })
        .ok();
}

pub async fn process_users_select(
    interaction: &discord::InteractionCreate,
    interaction_data: &discord::MessageComponentInteractionData,
    interaction_client: discord::InteractionClient<'_>,
) {
    let response = discord::InteractionResponse {
        kind: discord::InteractionResponseType::DeferredUpdateMessage,
        data: None,
    };

    if let Some(interaction_message) = &interaction.message {
        let mut form_data = ACTIVE_FORMS.lock().await;

        logging::info!("Updating uploaders form data for edit server uploaders.");

        let selected_users = form_data.get_mut(&interaction_message.id).map(|form_data| {
            let selected_users = form_data.selected_users.get_or_insert_default();
            selected_users.clear();
            selected_users
        });

        if let Some(selected_users) = selected_users {
            if let Some(selected_mentionables) = interaction_data.resolved.as_ref() {
                for user_id in selected_mentionables.users.keys() {
                    selected_users.insert(bot_data::Mentionable::User(*user_id));
                }

                for role_id in selected_mentionables.roles.keys() {
                    selected_users.insert(bot_data::Mentionable::Role(*role_id));
                }
            }
        }
    }

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .map_err(|err| {
            logging::error!("Couldn't defer update for edit server uploaders message: {err}");
        })
        .ok();
}

fn create_uploaders_diff(
    original: &HashSet<bot_data::Mentionable>,
    new: &Option<HashSet<bot_data::Mentionable>>,
) -> (String, String) {
    let (mut added_users, mut removed_users) = match new {
        Some(new) => (
            new.difference(original).map(|m| m.to_mention()).join(", "),
            original.difference(new).map(|m| m.to_mention()).join(", "),
        ),
        None => (String::new(), String::new()),
    };

    if added_users.is_empty() {
        added_users = "none".to_string();
    }

    if removed_users.is_empty() {
        removed_users = "none".to_string();
    }

    (
        format!("Added: {}.", added_users),
        format!("Removed: {}.", removed_users),
    )
}

async fn delete_interaction_message(
    interaction: &discord::InteractionCreate,
    interaction_client: &discord::InteractionClient<'_>,
) {
    // For whatever reason I first need to create deferred update response to delete the message.
    let response = discord::InteractionResponse {
        kind: discord::InteractionResponseType::DeferredUpdateMessage,
        data: None,
    };

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .map_err(|err| {
            logging::error!("Couldn't send defer update response for message deletion: {err}");
        })
        .ok();

    interaction_client
        .delete_response(&interaction.token)
        .await
        .map_err(|err| {
            logging::error!("Couldn't delete the message: {err}");
        })
        .ok();
}

pub async fn process_uploaders_submition(
    interaction: &discord::InteractionCreate,
    interaction_client: discord::InteractionClient<'_>,
) {
    logging::info!("Processing edit uploaders submission");

    delete_interaction_message(interaction, &interaction_client).await;

    let Some(interaction_message) = &interaction.message else {
        logging::error!("Couldn't get the message from interaction");
        return;
    };

    let form_lock = ACTIVE_FORMS.lock().await;
    let Some(form_data) = form_lock.get(&interaction_message.id) else {
        logging::error!("Couldn't associate the form data with interaction message id");
        return;
    };

    let Some(server_name) = &form_data.server_name else {
        logging::error!("Server name is missing in the form data");
        return;
    };

    let mut response = String::new();

    logging::info!("Updating server uploaders for server \"{server_name}\"");

    bot_data::update_data(|data| {
        if let Some(server) = data.servers.get_mut(server_name) {
            let response_title = ansi(
                format!(
                    "✓ Uploaders list for the server \"{server_name}\" is successfully updated."
                )
                .green()
                .to_string(),
            );

            let selected_users = &form_data.selected_users;

            let (added_users, removed_users) =
                create_uploaders_diff(&server.uploaders, selected_users);

            logging::info!("{added_users}");
            logging::info!("{removed_users}");

            response = format!("{response_title}\n{added_users}\n{removed_users}");

            if let Some(selected_users) = selected_users {
                server.uploaders = selected_users.clone();
            }
        } else {
            let error = format!("Server \"{server_name}\" not found in the bot data");
            logging::error!("{error}");
            response = ansi(format!("✗ {error}").red().to_string());
        }
    });

    interaction_client
        .update_response(&form_data.command_interaction_token)
        .content(Some(&response))
        .flags(discord::MessageFlags::SUPPRESS_NOTIFICATIONS)
        .await
        .map_err(|err| {
            logging::error!("Couldn't update edit server uploaders status message: {err}");
        })
        .ok();
}

fn construct_message_components(selected_server: Option<&str>) -> Vec<discord::Component> {
    let mut components = Vec::<discord::Component>::new();

    let select_menu = bot_data::create_server_select_menu(Some(1), selected_server, None);

    components.push(discord::Component::ActionRow(
        discord::ActionRowBuilder::new()
            .component(select_menu)
            .build(),
    ));

    // If there's a selected server then uploaders select menu and submit button will be shown.
    if let Some(server_name) = selected_server {
        let bot_data = &bot_data::get_data();
        if let Some(server) = bot_data.servers.get(server_name) {
            let mut current_uploaders = Vec::<discord::component::SelectDefaultValue>::new();
            for uploader in &server.uploaders {
                match *uploader {
                    bot_data::Mentionable::User(user_id) => current_uploaders
                        .push(discord::component::SelectDefaultValue::User(user_id)),
                    bot_data::Mentionable::Role(role_id) => current_uploaders
                        .push(discord::component::SelectDefaultValue::Role(role_id)),
                }
            }

            components.push(discord::Component::ActionRow(
                discord::ActionRowBuilder::new()
                    .component(
                        discord::SelectMenuBuilder::new(
                            "users_list",
                            discord::component::SelectMenuType::Mentionable,
                        )
                        .default_values(current_uploaders)
                        .min_values(0)
                        .max_values(25)
                        .placeholder("Uploaders")
                        .build(),
                    )
                    .build(),
            ));

            components.push(discord::Component::ActionRow(
                discord::ActionRowBuilder::new()
                    .component(
                        discord::ButtonBuilder::new(discord::component::ButtonStyle::Success)
                            .custom_id("confirm_edit_uploaders")
                            .label("Save")
                            .build(),
                    )
                    .build(),
            ));
        } else {
            logging::error!(
                "Couldn't find server \"{server_name}\" selected in edit server uploaders in bot data"
            );
        }
    }

    components
}
