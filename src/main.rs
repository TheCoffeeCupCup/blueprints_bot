mod bot_data;
mod commands;
mod common;
mod ftp;
mod logging;

use itertools::Itertools as _;
use twilight_gateway::StreamExt as _;

use common::{AnyError, ansi, discord, get_env};

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    dotenv::dotenv()?;

    logging::init_log_file()?;
    log_info!("Initialized file logging");

    let token = get_env("DISCORD_TOKEN");
    let intents = discord::Intents::empty();

    let mut shard = discord::Shard::new(discord::ShardId::ONE, token.clone(), intents);
    let http = std::sync::Arc::new(discord::HttpClient::new(token));

    let application_id = http.current_user_application().await?.model().await?.id;
    let target_guild_id = discord::Id::new(get_env("GUILD_ID").parse()?);

    let interaction_client = http.interaction(application_id);

    log_info!("Setting guild commands");
    interaction_client
        .set_guild_commands(
            target_guild_id,
            &[
                commands::upload_blueprints::create_command(),
                commands::add_server::create_command(),
                commands::edit_server_uploaders::create_command(),
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
    target_guild_id: discord::Id<discord::marker::GuildMarker>,
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
    http: &discord::HttpClient,
) {
    let interaction_client = http.interaction(interaction.application_id);

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
                    commands::upload_blueprints::process_command(&interaction, interaction_client)
                        .await;
                }
                commands::add_server::COMMAND => {
                    commands::add_server::process_command(&interaction, interaction_client).await;
                }
                commands::edit_server_uploaders::COMMAND => {
                    commands::edit_server_uploaders::process_command(
                        &interaction,
                        interaction_client,
                    )
                    .await;
                }
                _ => {}
            }
        }

        Some(discord::InteractionData::ModalSubmit(submit_data)) => {
            let modal_id = submit_data.custom_id.as_str();

            logging::info!("Received modal submition \"{modal_id}\"");

            match modal_id {
                commands::upload_blueprints::MODAL_ID => {
                    commands::upload_blueprints::process_modal_submition(
                        &interaction,
                        submit_data,
                        interaction_client,
                    )
                    .await;
                }
                commands::add_server::MODAL_ID => {
                    commands::add_server::process_modal_submition(
                        &interaction,
                        submit_data,
                        interaction_client,
                    )
                    .await;
                }
                _ => {}
            }
        }

        Some(discord::InteractionData::MessageComponent(message_component)) => {
            let component_id = message_component.custom_id.as_str();

            logging::info!("Received message component interaction \"{component_id}\"");

            match component_id {
                "server_select" => {
                    commands::edit_server_uploaders::process_server_select(
                        interaction,
                        message_component,
                        interaction_client,
                    )
                    .await
                }
                "confirm_edit_uploaders" => {
                    commands::edit_server_uploaders::process_uploaders_submition(
                        interaction,
                        interaction_client,
                    )
                    .await
                }
                "users_list" => {
                    commands::edit_server_uploaders::process_users_select(
                        interaction,
                        message_component,
                        interaction_client,
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

// TODO: Limit the size of blueprints folder
// TODO: Add more display errors for unhappy pathes
// TODO: Command for editing uploader's servers (as opposed to server's uploaders)
// TODO: Command for removing servers?
// TODO: Encrypt bot data
