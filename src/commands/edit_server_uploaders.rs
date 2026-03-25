use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

use colored::Colorize as _;
use itertools::Itertools;
use tokio::sync::Mutex;

use crate::{
    bot_data,
    common::{AnyError, ansi},
    discord,
};

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
) -> Result<(), AnyError> {
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
        .unwrap();

    let followup = interaction_client
        .create_followup(&interaction.token)
        .components(construct_message_components(None).as_slice())
        .flags(discord::MessageFlags::IS_COMPONENTS_V2 | discord::MessageFlags::EPHEMERAL)
        .await
        .unwrap();

    ACTIVE_FORMS.lock().await.insert(
        followup.model().await.unwrap().id,
        MessageFormData::new(interaction.token.clone()),
    );

    Ok(())
}

pub async fn process_server_select(
    interaction: &discord::InteractionCreate,
    interaction_data: &discord::MessageComponentInteractionData,
    interaction_client: discord::InteractionClient<'_>,
) -> Result<(), AnyError> {
    let mut server_name: Option<&str> = None;

    if let [selected_server_name] = interaction_data.values.as_slice() {
        server_name = Some(&selected_server_name);
    }

    let server_name = server_name.expect("Couldn't retrieve selected server name.");

    if let Some(interaction_message) = &interaction.message {
        if let Some(form_data) = ACTIVE_FORMS.lock().await.get_mut(&interaction_message.id) {
            form_data.server_name = Some(server_name.to_string());
        }
    }

    let data = discord::InteractionResponseDataBuilder::new()
        .flags(discord::MessageFlags::IS_COMPONENTS_V2 | discord::MessageFlags::EPHEMERAL)
        .components(construct_message_components(Some(server_name)))
        .build();

    let response = discord::InteractionResponse {
        kind: discord::InteractionResponseType::UpdateMessage,
        data: Some(data),
    };

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .unwrap();

    Ok(())
}

pub async fn process_users_select(
    interaction: &discord::InteractionCreate,
    interaction_data: &discord::MessageComponentInteractionData,
    interaction_client: discord::InteractionClient<'_>,
) -> Result<(), AnyError> {
    let response = discord::InteractionResponse {
        kind: discord::InteractionResponseType::DeferredUpdateMessage,
        data: None,
    };

    if let Some(interaction_message) = &interaction.message {
        if let Some(form_data) = ACTIVE_FORMS.lock().await.get_mut(&interaction_message.id) {
            let selected_users = form_data.selected_users.get_or_insert_default();
            selected_users.clear();

            if let Some(selected_mentionables) = interaction_data.resolved.as_ref() {
                for (user_id, _user) in &selected_mentionables.users {
                    selected_users.insert(bot_data::Mentionable::User(*user_id));
                }

                for (role_id, _role) in &selected_mentionables.roles {
                    selected_users.insert(bot_data::Mentionable::Role(*role_id));
                }
            }
        }
    }

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .unwrap();

    Ok(())
}

fn create_users_diff(
    original: &HashSet<bot_data::Mentionable>,
    new: &HashSet<bot_data::Mentionable>,
) -> (String, String) {
    let added_users = new.difference(original).map(|m| m.to_mention()).join(", ");
    let removed_users = original.difference(new).map(|m| m.to_mention()).join(", ");

    let mut added_users_text = String::new();

    if !added_users.is_empty() {
        added_users_text = format!("\nAdded: {}.", added_users);
    }

    let mut removed_users_text = String::new();

    if !removed_users.is_empty() {
        removed_users_text = format!("\nRemoved: {}.", removed_users);
    }

    (added_users_text, removed_users_text)
}

pub async fn process_uploaders_submition(
    interaction: &discord::InteractionCreate,
    interaction_client: discord::InteractionClient<'_>,
) -> Result<(), AnyError> {
    let response = discord::InteractionResponse {
        kind: discord::InteractionResponseType::DeferredUpdateMessage,
        data: None,
    };

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .unwrap();

    if let Some(interaction_message) = &interaction.message {
        if let Some(form_data) = ACTIVE_FORMS.lock().await.get(&interaction_message.id) {
            if let (command_interaction_token, Some(server_name), Some(selected_users)) = (
                &form_data.command_interaction_token,
                &form_data.server_name,
                &form_data.selected_users,
            ) {
                let mut response = String::new();

                bot_data::update_data(|data| {
                    if let Some(server) = data.servers.get_mut(server_name) {
                        let (added_users, removed_users) =
                            create_users_diff(&server.uploaders, selected_users);

                        let response_title = ansi(
                            format!(
                                "✓ Uploaders list for the server \"{server_name}\" is successfully updated."
                            )
                            .green()
                            .to_string(),
                        );

                        response = format!("{response_title}{added_users}{removed_users}");

                        server.uploaders = selected_users.clone();
                    }
                });

                interaction_client
                    .update_response(command_interaction_token)
                    .content(Some(&response))
                    .flags(discord::MessageFlags::SUPPRESS_NOTIFICATIONS)
                    .await
                    .unwrap();
            }
        }
    }

    interaction_client
        .delete_response(&interaction.token)
        .await
        .unwrap();

    Ok(())
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
        let server = bot_data
            .servers
            .get(server_name)
            .expect("Couldn't find server by name");

        let mut current_uploaders = Vec::<discord::component::SelectDefaultValue>::new();
        for uploader in &server.uploaders {
            match *uploader {
                bot_data::Mentionable::User(user_id) => {
                    current_uploaders.push(discord::component::SelectDefaultValue::User(user_id))
                }
                bot_data::Mentionable::Role(role_id) => {
                    current_uploaders.push(discord::component::SelectDefaultValue::Role(role_id))
                }
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
    }

    components
}
