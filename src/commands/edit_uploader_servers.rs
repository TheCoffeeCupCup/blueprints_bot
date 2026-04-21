use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

use colored::Colorize as _;
use tokio::sync::{MappedMutexGuard, Mutex, MutexGuard};
use twilight_util::builder::message::TextDisplayBuilder;

use crate::{
    bot_data, commands,
    common::{self, discord},
    logging,
};

/* Constants */

macro_rules! command {
    // Rust can't concatenate const variables in compile time.
    () => {
        "edit_uploader_servers"
    };
}

pub const COMMAND: &'static str = command!();

pub const USER_SELECT_ID: &'static str = concat!(command!(), "/user_select");
pub const SERVERS_SELECT_ID: &'static str = concat!(command!(), "/servers_select");
pub const SUBMIT_BUTTON_ID: &'static str = concat!(command!(), "/submit");

static ACTIVE_FORMS: LazyLock<Mutex<HashMap<discord::MessageId, MessageFormData>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/* Interface functions */

pub fn create_command() -> discord::Command {
    logging::info!("Creating command `/{COMMAND}`");

    discord::CommandBuilder::new(
        COMMAND,
        "Modify the list of servers a specified user or role has access to",
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

pub async fn process_user_selected(
    interaction: &discord::InteractionCreate,
    interaction_data: &discord::MessageComponentInteractionData,
    interaction_client: discord::InteractionClient<'_>,
) {
    logging::info!("Processing `{USER_SELECT_ID}` interaction");
    process_user_selected_impl(interaction, interaction_data, interaction_client).await;
    logging::info!("Finished processing `{USER_SELECT_ID}` interaction");
}

pub async fn process_servers_selected(
    interaction: &discord::InteractionCreate,
    interaction_data: &discord::MessageComponentInteractionData,
    interaction_client: discord::InteractionClient<'_>,
) {
    logging::info!("Processing `{SERVERS_SELECT_ID}` interaction");
    process_servers_selected_impl(interaction, interaction_data, interaction_client).await;
    logging::info!("Finished processing `{SERVERS_SELECT_ID}` interaction");
}

pub async fn process_submit_clicked(
    interaction: &discord::InteractionCreate,
    _interaction_data: &discord::MessageComponentInteractionData,
    interaction_client: discord::InteractionClient<'_>,
) {
    logging::info!("Processing `{SUBMIT_BUTTON_ID}` interaction");
    process_submit_clicked_impl(interaction, interaction_client).await;
    logging::info!("Finished processing `{SUBMIT_BUTTON_ID}` interaction");
}

/* Impl functions */

async fn process_command_impl(
    interaction: &discord::InteractionCreate,
    interaction_client: discord::InteractionClient<'_>,
) {
    if reject_wrong_servers_amount(interaction, &interaction_client).await {
        return;
    };

    send_placeholder_message(interaction, &interaction_client).await;

    let response = interaction_client
        .create_followup(&interaction.token)
        .components(construct_message_components(None, None).as_slice())
        .flags(discord::MessageFlags::IS_COMPONENTS_V2 | discord::MessageFlags::EPHEMERAL)
        .await
        .map_err(|err| logging::error!("Couldn't send the followup: {err}"));

    let Some(response) = response.ok() else {
        interaction_client
            .delete_response(&interaction.token)
            .await
            .map_err(|err| logging::error!("Couldn't delete the placeholder message: {err}"))
            .ok();

        return;
    };

    match response.model().await.map(|followup| followup.id) {
        Ok(followup_id) => {
            ACTIVE_FORMS
                .lock()
                .await
                .insert(followup_id, MessageFormData::new(interaction.token.clone()));
        }
        Err(err) => {
            logging::error!("Couldn't retrieve followup id: {err}");
        }
    }
}

async fn process_user_selected_impl(
    interaction: &discord::InteractionCreate,
    interaction_data: &discord::MessageComponentInteractionData,
    interaction_client: discord::InteractionClient<'_>,
) {
    let Some(mut form_data) =
        get_relevant_form_data(USER_SELECT_ID, interaction, &interaction_client).await
    else {
        return;
    };

    let mut selected_user_roles: Option<Vec<discord::RoleId>> = None;

    if let Some(selected_mentionables) = interaction_data.resolved.as_ref() {
        let selected_mentionables_amount =
            selected_mentionables.users.len() + selected_mentionables.roles.len();

        if selected_mentionables_amount == 1 {
            for member in selected_mentionables.members.values() {
                selected_user_roles = Some(member.roles.clone());
            }

            for user_id in selected_mentionables.users.keys() {
                form_data.selected_mentionable = Some(bot_data::Mentionable::User(*user_id));
            }

            for role_id in selected_mentionables.roles.keys() {
                form_data.selected_mentionable = Some(bot_data::Mentionable::Role(*role_id));
            }
        } else {
            logging::error!(
                "Selected mentionables amount is {selected_mentionables_amount} but must be 1"
            );
        }
    }

    let components = construct_message_components(
        form_data.selected_mentionable.as_ref(),
        selected_user_roles.as_ref(),
    );

    let data = discord::InteractionResponseDataBuilder::new()
        .flags(discord::MessageFlags::IS_COMPONENTS_V2 | discord::MessageFlags::EPHEMERAL)
        .components(components)
        .build();

    let response = discord::InteractionResponse {
        kind: discord::InteractionResponseType::UpdateMessage,
        data: Some(data),
    };

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .map_err(|err| {
            logging::error!("Couldn't update `/{COMMAND}` main message: {err}");
        })
        .ok();
}

async fn process_servers_selected_impl(
    interaction: &discord::InteractionCreate,
    interaction_data: &discord::MessageComponentInteractionData,
    interaction_client: discord::InteractionClient<'_>,
) {
    let Some(mut form_data) =
        get_relevant_form_data(USER_SELECT_ID, interaction, &interaction_client).await
    else {
        return;
    };

    form_data.selected_servers = Some(HashSet::from_iter(interaction_data.values.iter().cloned()));

    let response = discord::InteractionResponse {
        kind: discord::InteractionResponseType::DeferredUpdateMessage,
        data: None,
    };

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .map_err(|err| {
            logging::error!("Couldn't acknowledge `{SERVERS_SELECT_ID}` interaction: {err}");
        })
        .ok();
}

async fn process_submit_clicked_impl(
    interaction: &discord::InteractionCreate,
    interaction_client: discord::InteractionClient<'_>,
) {
    discord::delete_interaction_message(interaction, &interaction_client).await;

    let Some(form_data) =
        get_relevant_form_data(USER_SELECT_ID, interaction, &interaction_client).await
    else {
        return;
    };

    let Some(selected_mentionable) = &form_data.selected_mentionable else {
        logging::error!(
            "Couldn't get the selected mentionable from the form data for `{SUBMIT_BUTTON_ID}` interaction"
        );

        discord::negative_response(
            interaction,
            &interaction_client,
            &format!("✗ Couldn't get the selected mentionable from the form data."),
        )
        .await;

        interaction_client
            .delete_response(&form_data.command_interaction_token)
            .await
            .map_err(|err| {
                logging::error!("Couldn't delete `/{COMMAND}` placeholder message: {err}");
            })
            .ok();

        return;
    };

    let mut added_servers = HashSet::new();
    let mut removed_servers = HashSet::new();

    if let Some(mut remaining_selected_servers) = form_data.selected_servers.clone() {
        bot_data::update_data(|bot_data| {
            for (server_name, server_data) in &mut bot_data.servers {
                let server_has_uploader = server_data.uploaders.contains(selected_mentionable);
                let selected_servers_have_server = remaining_selected_servers.contains(server_name);

                if selected_servers_have_server && !server_has_uploader {
                    server_data.uploaders.insert(*selected_mentionable);

                    added_servers.insert(server_name.clone());

                    remaining_selected_servers.remove(server_name);
                }

                if server_has_uploader && !selected_servers_have_server {
                    server_data.uploaders.remove(selected_mentionable);

                    removed_servers.insert(server_name.clone());

                    remaining_selected_servers.remove(server_name);
                }
            }
        });

        if remaining_selected_servers.len() > 0 {
            logging::error!(
                "Servers {remaining_selected_servers:?} couldn't be processed for `{SUBMIT_BUTTON_ID}`"
            );
        }
    }

    let mention = selected_mentionable.to_mention();

    let mut added_servers_text = "Added: ".to_string();
    if added_servers.len() > 0 {
        added_servers_text += &common::list_to_string(&added_servers);
    } else {
        added_servers_text += "none";
    }

    let mut removed_servers_text = "Removed: ".to_string();
    if removed_servers.len() > 0 {
        removed_servers_text += &common::list_to_string(&removed_servers);
    } else {
        removed_servers_text += "none";
    }

    logging::info!("Updated the list of the servers {selected_mentionable:?} can upload to");
    logging::info!("{added_servers_text}");
    logging::info!("{removed_servers_text}");

    let servers_info = common::ansi(
        format!("{added_servers_text}.\n{removed_servers_text}.")
            .green()
            .to_string(),
    );

    let response_text =
        format!("✓ Updated the list of servers available to {mention}.\n{servers_info}");

    interaction_client
        .update_response(&form_data.command_interaction_token)
        .content(Some(&response_text))
        .flags(discord::MessageFlags::SUPPRESS_NOTIFICATIONS)
        .await
        .map_err(|err| {
            logging::error!("Couldn't update the status message for {SUBMIT_BUTTON_ID}: {err}");
        })
        .ok();
}

/* Other Discord functions */

/// Returns `true` if interaction is rejected, `false` otherwise.
async fn reject_wrong_servers_amount(
    interaction: &discord::InteractionCreate,
    interaction_client: &discord::InteractionClient<'_>,
) -> bool {
    let servers_amount = bot_data::get_data().servers.len();

    if servers_amount == 0 {
        logging::info!("`/{COMMAND}` command issued while the amount of servers is 0");

        let error = format!(
            "✗ No servers have been set up yet. It can be done via the `/{}` command.",
            commands::add_server::COMMAND
        );
        discord::negative_response(interaction, interaction_client, &error).await;

        return true;
    }

    false
}

async fn send_placeholder_message(
    interaction: &discord::InteractionCreate,
    interaction_client: &discord::InteractionClient<'_>,
) {
    let data = discord::InteractionResponseDataBuilder::new()
        .content("Waiting for settings submission...")
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
                "Couldn't send \"Waiting for submission\" message for `/{COMMAND}`: {err}"
            );
        })
        .ok();
}

/* Main message components */

fn construct_message_components(
    selected_mentionable: Option<&bot_data::Mentionable>,
    selected_user_roles: Option<&Vec<discord::RoleId>>,
) -> Vec<discord::Component> {
    logging::info!("Constructing `/{COMMAND}` main message components");

    let mut components = Vec::<discord::Component>::new();

    components.push(make_user_select(selected_mentionable));

    if let Some(selected_user) = selected_mentionable.as_ref() {
        let (server_select, servers_via_roles) =
            make_server_select(selected_user, selected_user_roles);

        components.push(server_select);
        if let Some(servers_via_roles) = servers_via_roles {
            components.push(servers_via_roles);
        }

        components.push(make_submit_button());
    }

    components
}

fn make_user_select(selected_user: Option<&bot_data::Mentionable>) -> discord::Component {
    let mut user_select = discord::SelectMenuBuilder::new(
        USER_SELECT_ID,
        discord::component::SelectMenuType::Mentionable,
    )
    .min_values(1)
    .max_values(1)
    .required(true)
    .placeholder("Uploader");

    if let Some(selected_user) = selected_user {
        let selected_mentionable = match selected_user {
            bot_data::Mentionable::User(user_id) => {
                discord::component::SelectDefaultValue::User(*user_id)
            }
            bot_data::Mentionable::Role(role_id) => {
                discord::component::SelectDefaultValue::Role(*role_id)
            }
        };

        user_select = user_select.default_values(vec![selected_mentionable]);
    }

    let user_select = user_select.build();

    let user_select_action_row = discord::Component::ActionRow(
        discord::ActionRowBuilder::new()
            .component(discord::Component::SelectMenu(user_select))
            .build(),
    );

    user_select_action_row
}

fn make_server_select(
    selected_uploader: &bot_data::Mentionable,
    selected_user_roles: Option<&Vec<discord::RoleId>>,
) -> (discord::Component, Option<discord::Component>) {
    let mut servers_amount = 0u8;

    let mut select_builder = discord::SelectMenuBuilder::new(
        SERVERS_SELECT_ID,
        discord::component::SelectMenuType::Text,
    )
    .min_values(0);

    let bot_data = bot_data::get_data();

    let mut servers_via_roles = Vec::new();

    for (server_name, server_data) in &bot_data.servers {
        let mut option = discord::SelectMenuOptionBuilder::new(server_name, server_name).build();

        for uploader in &server_data.uploaders {
            if uploader == selected_uploader {
                option.default = true;
            }

            if let Some(selected_user_roles) = selected_user_roles {
                if let bot_data::Mentionable::Role(uploader_role) = uploader {
                    if selected_user_roles.contains(uploader_role) {
                        servers_via_roles.push(server_name);
                    }
                }
            }
        }

        select_builder = select_builder.option(option);
        servers_amount += 1;
    }

    // Discord doesn't allow to set the limit to the number higher than amount of options.
    select_builder = select_builder.max_values(servers_amount);

    let server_select_action_row = discord::Component::ActionRow(
        discord::ActionRowBuilder::new()
            .component(discord::Component::SelectMenu(select_builder.build()))
            .build(),
    );

    let mut servers_via_roles_component = None;

    if servers_via_roles.len() > 0 {
        let servers_via_roles = common::list_to_string(&servers_via_roles);

        let text = common::ansi(
            format!("⚠ Servers accessible via roles: {servers_via_roles}.")
                .yellow()
                .to_string(),
        );

        servers_via_roles_component = Some(discord::Component::TextDisplay(
            TextDisplayBuilder::new(text).build(),
        ));
    }

    (server_select_action_row, servers_via_roles_component)
}

fn make_submit_button() -> discord::Component {
    let button = discord::ButtonBuilder::new(discord::component::ButtonStyle::Success)
        .custom_id(SUBMIT_BUTTON_ID)
        .label("Save")
        .build();

    discord::Component::ActionRow(discord::ActionRowBuilder::new().component(button).build())
}

/* Helper functions */

#[derive(Debug)]
struct MessageFormData {
    pub command_interaction_token: String,
    pub selected_mentionable: Option<bot_data::Mentionable>,
    pub selected_servers: Option<HashSet<String>>,
}

impl MessageFormData {
    fn new(token: String) -> Self {
        Self {
            command_interaction_token: token,
            selected_mentionable: None,
            selected_servers: None,
        }
    }
}

async fn get_relevant_form_data(
    interaction_id: &str,
    interaction: &discord::InteractionCreate,
    interaction_client: &discord::InteractionClient<'_>,
) -> Option<MappedMutexGuard<'static, MessageFormData>> {
    let Some(interaction_message) = &interaction.message else {
        logging::error!("Couldn't retrieve interaction message");
        return None;
    };

    let mutex_guard = ACTIVE_FORMS.lock().await;

    if !mutex_guard.contains_key(&interaction_message.id) {
        logging::error!(
            "Couldn't find form data corresponding to the received `{interaction_id}` interaction"
        );

        discord::negative_response(
            interaction,
            &interaction_client,
            &format!("✗ Couldn't find form data corresponding to the received interaction."),
        )
        .await;

        return None;
    };

    Some(MutexGuard::map(mutex_guard, |map| {
        map.get_mut(&interaction_message.id).expect("Infallible")
    }))
}
