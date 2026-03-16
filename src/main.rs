mod bot_data;
mod commands;
mod common;

use suppaftp::NativeTlsFtpStream;
use twilight_gateway::StreamExt as _;

use common::{AnyError, ansi, discord, get_env};

use crate::commands::edit_server_uploaders;

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    dotenv::dotenv()?;

    let token = get_env("DISCORD_TOKEN");
    let intents = discord::Intents::GUILD_MESSAGES | discord::Intents::MESSAGE_CONTENT;

    let mut shard = discord::Shard::new(discord::ShardId::ONE, token.clone(), intents);
    let http = std::sync::Arc::new(discord::HttpClient::new(token));

    let application_id = http.current_user_application().await?.model().await?.id;
    let test_guild_id = discord::Id::new(get_env("TEST_GUILD_ID").parse()?);

    let interaction_client = http.interaction(application_id);

    interaction_client
        .set_guild_commands(
            test_guild_id,
            &[
                commands::upload_blueprints::create_command(),
                commands::add_server::create_command(),
                commands::edit_server_uploaders::create_command(),
            ],
        )
        .await?;

    let cache = discord::DefaultInMemoryCache::builder()
        .resource_types(discord::ResourceType::MESSAGE)
        .build();

    while let Some(item) = shard.next_event(discord::EventTypeFlags::all()).await {
        let Ok(event) = item else {
            tracing::warn!(source = ?item.unwrap_err(), "error receiving event");

            continue;
        };
        cache.update(&event);

        tokio::spawn(handle_event(event, std::sync::Arc::clone(&http)));
    }

    Ok(())
}

async fn handle_event(
    event: discord::Event,
    http: std::sync::Arc<discord::HttpClient>,
) -> Result<(), AnyError> {
    match event {
        discord::Event::InteractionCreate(interaction) => {
            handle_interaction_create(&interaction, &http).await;
        }
        _ => {}
    }

    Ok(())
}

async fn handle_interaction_create(
    interaction: &discord::InteractionCreate,
    http: &discord::HttpClient,
) {
    let interaction_client = http.interaction(interaction.application_id);

    match &interaction.data {
        Some(discord::InteractionData::ApplicationCommand(command)) => {
            match command.name.as_str() {
                commands::upload_blueprints::COMMAND => {
                    commands::upload_blueprints::process_command(&interaction, interaction_client)
                        .await
                        .unwrap();
                }
                commands::add_server::COMMAND => {
                    commands::add_server::process_command(&interaction, interaction_client)
                        .await
                        .unwrap();
                }
                commands::edit_server_uploaders::COMMAND => {
                    commands::edit_server_uploaders::process_command(
                        &interaction,
                        interaction_client,
                    )
                    .await
                    .unwrap();
                }
                _ => {}
            }
        }

        Some(discord::InteractionData::ModalSubmit(submit_data)) => {
            match submit_data.custom_id.as_str() {
                commands::upload_blueprints::MODAL_ID => {
                    commands::upload_blueprints::process_modal_submition(
                        &interaction,
                        submit_data,
                        interaction_client,
                    )
                    .await
                    .unwrap();
                }
                commands::add_server::MODAL_ID => {
                    commands::add_server::process_modal_submition(
                        &interaction,
                        submit_data,
                        interaction_client,
                    )
                    .await
                    .unwrap();
                }
                _ => {}
            }
        }

        Some(discord::InteractionData::MessageComponent(message_component)) => {
            match message_component.custom_id.as_str() {
                "server_select" => edit_server_uploaders::process_server_select(
                    interaction,
                    message_component,
                    interaction_client,
                )
                .await
                .unwrap(),
                "confirm_edit_uploaders" => edit_server_uploaders::process_uploaders_submition(
                    interaction,
                    interaction_client,
                )
                .await
                .unwrap(),
                "users_list" => edit_server_uploaders::process_users_select(
                    interaction,
                    message_component,
                    interaction_client,
                )
                .await
                .unwrap(),
                _ => {}
            }
        }

        _ => {}
    }
}

async fn upload_files(files: Vec<commands::upload_blueprints::Attachment>, servers: Vec<String>) {
    let mut ftp_servers = Vec::<suppaftp::ImplFtpStream<_>>::new();

    for server_name in &servers {
        let server_creds = bot_data::get_server_creds(&server_name).expect("Missing server creds");

        if server_creds.connection != bot_data::ConnectionType::FTP {
            // TODO: Implement SFTP if needed.
            panic!("Unknown server connection type");
        }

        let mut ftp_stream = NativeTlsFtpStream::connect(server_creds.full_ip).unwrap();
        ftp_stream
            .login(&server_creds.user, &server_creds.password)
            .unwrap();
        ftp_stream
            .cwd(format!(
                ".config/Epic/FactoryGame/Saved/SaveGames/blueprints/{}",
                server_creds.world_name
            ))
            .unwrap();

        ftp_servers.push(ftp_stream);
    }

    for file in &files {
        let file_response = reqwest::get(file.url.clone()).await.unwrap();
        let bytes = file_response.bytes().await.unwrap();
        let mut reader = std::io::Cursor::new(bytes);

        for server in &mut ftp_servers {
            server.put_file(file.filename.clone(), &mut reader).unwrap();
        }
    }

    for server in &mut ftp_servers {
        server.quit().unwrap();
    }
}

// TODO: Logging (tracing crate?)
// TODO: Remove the unwraps
// TODO: Limit the size of blueprints folder
// TODO: Add more display errors for unhappy pathes
