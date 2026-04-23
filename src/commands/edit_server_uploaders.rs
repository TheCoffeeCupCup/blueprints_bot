use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

use colored::Colorize as _;
use itertools::Itertools;
use tokio::sync::Mutex;

use crate::{
    bot_data, commands,
    common::ansi,
    discord,
    discord_utils::{self, IntoMessage},
    logging::{self, LogError as _},
};

pub const COMMAND: &'static str = "edit_server_uploaders";

pub fn create_command() -> discord::Command {
    logging::info!("Creating command `/{COMMAND}`");

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

static ACTIVE_FORMS: LazyLock<Mutex<HashMap<discord::MessageId, MessageFormData>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub async fn process_command(
    interaction: &discord::InteractionCreate,
    http_client: &discord::HttpClient,
) {
    let servers_amount = bot_data::get_data().servers.len();

    if servers_amount == 0 {
        logging::info!("Edit server uploaders command issued while amount of servers is 0");

        let error = format!(
            "No servers have been set up yet. It can be done via the `/{}` command.",
            commands::add_server::COMMAND
        );

        discord_utils::error_message_response(error, interaction, http_client).await;

        return;
    }

    logging::info!("Sending placeholder message");

    discord_utils::InteractionResponse::new(interaction, http_client)
        .send_message("Waiting for settings submission...".into_message())
        .await
        .log_error();

    send_form_message(interaction, http_client)
        .await
        .log_error();

    logging::info!("Responded to edit server uploaders command");
}

pub async fn process_server_select(
    interaction: &discord::InteractionCreate,
    interaction_data: &discord::MessageComponentInteractionData,
    http_client: &discord::HttpClient,
) {
    let mut server_name: Option<&str> = None;

    if let [selected_server_name] = interaction_data.values.as_slice() {
        server_name = Some(&selected_server_name);
    }

    let components = match server_name {
        Some(server_name) => {
            if let Some(interaction_message) = &interaction.message {
                logging::info!("Updating server name form data for edit server uploaders.");

                if let Some(form_data) = ACTIVE_FORMS.lock().await.get_mut(&interaction_message.id)
                {
                    form_data.server_name = Some(server_name.to_string());
                }
            }

            construct_message_components(Some(server_name))
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

            components
        }
    };

    discord_utils::InteractionResponse::new(interaction, http_client)
        .update_message(discord_utils::Message::components(components).ephemeral())
        .await
        .log_error();
}

pub async fn process_users_select(
    interaction: &discord::InteractionCreate,
    interaction_data: &discord::MessageComponentInteractionData,
    http_client: &discord::HttpClient,
) {
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

    discord_utils::InteractionResponse::new(interaction, http_client)
        .acknowledge()
        .await
        .log_error();
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

pub async fn process_uploaders_submission(
    interaction: &discord::InteractionCreate,
    http_client: &discord::HttpClient,
) {
    logging::info!("Processing edit uploaders submission");

    discord_utils::InteractionResponse::new(interaction, http_client)
        .delete_message()
        .await
        .log_error();

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

    discord_utils::InteractionResponse::new(interaction, http_client)
        .with_token(&form_data.command_interaction_token)
        .update(discord_utils::Message::text(response).ephemeral())
        .await
        .log_error();
}

fn construct_message_components(selected_server: Option<&str>) -> Vec<discord::Component> {
    let mut components = Vec::<discord::Component>::new();

    let select_menu = bot_data::create_server_select_menu_custom_id(
        Some(1),
        selected_server,
        None,
        "edit_uploaders_server_select",
    );

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
                            "edit_uploaders_users_list",
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

async fn send_form_message(
    interaction: &discord::InteractionCreate,
    http_client: &discord::HttpClient,
) -> Result<(), String> {
    let form_message =
        discord_utils::Message::components(construct_message_components(None)).ephemeral();

    let response = discord_utils::InteractionResponse::new(interaction, http_client)
        .send_followup_message(form_message)
        .await?;

    ACTIVE_FORMS.lock().await.insert(
        discord_utils::followup_id(response).await?,
        MessageFormData::new(interaction.token.clone()),
    );

    Ok(())
}
