mod bot_data;
mod commands;
mod common;
mod discord_utils;
mod encryption;
mod ftp;
mod logging;
mod secrets;

use itertools::Itertools as _;
use twilight_gateway::StreamExt as _;

use common::{AnyError, ansi, discord};

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    // Required for displaying colors in Discord messages that use ansi code block trick.
    colored::control::set_override(true);

    logging::init_log_file()?;
    log_info!("Initialized file logging");

    logging::info!("CARGO_PKG_VERSION: {}", bot_data::CARGO_PKG_VERSION);
    logging::info!("GIT_TAG: {}", bot_data::GIT_TAG);
    bot_data::get_git_version_status(bot_data::GIT_TAG).await;

    let intents = discord::Intents::empty();

    let mut shard;

    let http;
    {
        let token = secrets::discord_token();

        shard = discord::Shard::new(discord::ShardId::ONE, token.clone(), intents);

        let client_builder = discord::ClientBuilder::new()
            .token(token)
            .default_allowed_mentions(discord::AllowedMentions {
                parse: Vec::new(),
                replied_user: true,
                roles: Vec::new(),
                users: Vec::new(),
            });
        http = std::sync::Arc::new(client_builder.build());
    }

    let application_id = http.current_user_application().await?.model().await?.id;
    let target_guild_id = discord::Id::new(secrets::guild_id().parse()?);

    let interaction_client = http.interaction(application_id);

    log_info!("Setting guild commands");
    interaction_client
        .set_guild_commands(
            target_guild_id,
            &[
                commands::upload_blueprints::create_command(),
                commands::upload_blueprints::create_message_command(),
                commands::add_server::create_command(),
                commands::remove_server::create_command(),
                commands::edit_server_uploaders::create_command(),
                commands::edit_uploader_servers::create_command(),
                commands::version::create_command(),
            ],
        )
        .await?;

    log_info!("Starting the loop");
    while let Some(item) = shard.next_event(discord::EventTypeFlags::all()).await {
        match item {
            Ok(event) => {
                tokio::spawn(handle_event(
                    event,
                    std::sync::Arc::clone(&http),
                    target_guild_id,
                ));
            }
            Err(err) => logging::error!("Error receiving event: {}", err),
        }
    }

    Ok(())
}

async fn handle_event(
    event: discord::Event,
    http: std::sync::Arc<discord::HttpClient>,
    target_guild_id: discord::GuildId,
) {
    // If event type is undesired we ignore it without logging any warnings.
    let discord::Event::InteractionCreate(interaction) = &event else {
        return;
    };

    let Some(guild_id) = event.guild_id() else {
        logging::error!("Event rejected: couldn't check event's guild ID");
        return;
    };

    if guild_id != target_guild_id {
        logging::warning!("Event rejected: guild id `{guild_id}` doesn't correspond to the target");
        return;
    }

    handle_interaction_create(&interaction, &http).await
}

fn get_author_names(interaction: &discord::InteractionCreate) -> Vec<&str> {
    let mut names = Vec::new();

    if let Some(member) = interaction.member.as_ref() {
        if let Some(nick) = member.nick.as_ref() {
            names.push(nick.as_str());
        }

        if let Some(user) = member.user.as_ref() {
            if let Some(global_name) = user.global_name.as_ref() {
                names.push(global_name.as_str());
            }

            names.push(user.name.as_str());
        }
    }

    names
}

async fn handle_interaction_create(
    interaction: &discord::InteractionCreate,
    http_client: &discord::HttpClient,
) {
    let author_names = get_author_names(interaction);

    let author_names = if author_names.is_empty() {
        "unknown".to_string()
    } else {
        author_names
            .iter()
            .map(|name| format!("\"{name}\""))
            .join(" aka ")
    };

    logging::info!("Received interaction from {author_names}");

    match &interaction.data {
        Some(discord::InteractionData::ApplicationCommand(command)) => {
            let command_name = command.name.as_str();

            logging::info!("Received application command \"{command_name}\"");

            match command_name {
                commands::upload_blueprints::COMMAND => {
                    commands::upload_blueprints::process_command(&interaction, http_client).await;
                }
                commands::add_server::COMMAND => {
                    commands::add_server::process_command(&interaction, http_client).await;
                }
                commands::remove_server::COMMAND => {
                    commands::remove_server::process_command(&interaction, http_client).await;
                }
                commands::edit_server_uploaders::COMMAND => {
                    commands::edit_server_uploaders::process_command(&interaction, http_client)
                        .await;
                }
                commands::edit_uploader_servers::COMMAND => {
                    commands::edit_uploader_servers::process_command(&interaction, http_client)
                        .await;
                }
                commands::version::COMMAND => {
                    commands::version::process_command(&interaction, http_client).await;
                }

                commands::upload_blueprints::MESSAGE_COMMAND => {
                    commands::upload_blueprints::process_message_command(
                        &interaction,
                        command,
                        http_client,
                    )
                    .await;
                }

                _ => {}
            }
        }

        Some(discord::InteractionData::ModalSubmit(submit_data)) => {
            let modal_id = submit_data.custom_id.as_str();

            logging::info!("Received modal submission \"{modal_id}\"");

            match modal_id {
                commands::upload_blueprints::MODAL_ID => {
                    commands::upload_blueprints::process_modal_submission(
                        &interaction,
                        submit_data,
                        http_client,
                    )
                    .await;
                }
                commands::add_server::MODAL_ID => {
                    commands::add_server::process_modal_submission(
                        &interaction,
                        submit_data,
                        http_client,
                    )
                    .await;
                }
                commands::remove_server::MODAL_ID => {
                    commands::remove_server::process_modal_submission(
                        &interaction,
                        submit_data,
                        http_client,
                    )
                    .await;
                }

                _ => {
                    if modal_id.starts_with(commands::upload_blueprints::FROM_MESSAGE_MODAL_ID) {
                        commands::upload_blueprints::process_from_message_modal_submission(
                            &interaction,
                            submit_data,
                            http_client,
                        )
                        .await;
                    }
                }
            }
        }

        Some(discord::InteractionData::MessageComponent(message_component)) => {
            let component_id = message_component.custom_id.as_str();

            logging::info!("Received message component interaction \"{component_id}\"");

            match component_id {
                "edit_uploaders_server_select" => {
                    commands::edit_server_uploaders::process_server_select(
                        interaction,
                        message_component,
                        http_client,
                    )
                    .await
                }
                "edit_uploaders_users_list" => {
                    commands::edit_server_uploaders::process_users_select(
                        interaction,
                        message_component,
                        http_client,
                    )
                    .await
                }
                "confirm_edit_uploaders" => {
                    commands::edit_server_uploaders::process_uploaders_submission(
                        interaction,
                        http_client,
                    )
                    .await
                }

                commands::edit_uploader_servers::USER_SELECT_ID => {
                    commands::edit_uploader_servers::process_user_selected(
                        interaction,
                        message_component,
                        http_client,
                    )
                    .await
                }
                commands::edit_uploader_servers::SERVERS_SELECT_ID => {
                    commands::edit_uploader_servers::process_servers_selected(
                        interaction,
                        message_component,
                        http_client,
                    )
                    .await
                }
                commands::edit_uploader_servers::SUBMIT_BUTTON_ID => {
                    commands::edit_uploader_servers::process_submit_clicked(
                        interaction,
                        message_component,
                        http_client,
                    )
                    .await
                }

                _ => {}
            }
        }

        _ => {
            logging::info!("Received unhandled interaction {:?}", interaction.kind);
        }
    }
}
