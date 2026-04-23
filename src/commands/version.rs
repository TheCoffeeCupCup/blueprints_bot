/* Constants */

use crate::{
    bot_data,
    common::discord,
    discord_utils,
    logging::{self, LogError},
};

pub const COMMAND: &'static str = "version";

/* Interface functions */

pub fn create_command() -> discord::Command {
    logging::info!("Creating command `/{COMMAND}`");

    discord::CommandBuilder::new(
        COMMAND,
        "Get the version of the bot",
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

/* Impl functions */

async fn process_command_impl(
    interaction: &discord::InteractionCreate,
    http_client: &discord::HttpClient,
) {
    let bot_version = format!("Bot version: `{}`", bot_data::CARGO_PKG_VERSION);
    let version_status = bot_data::get_git_version_status(bot_data::GIT_TAG).await;
    let git_tag = format!("Git tag: `{}` ({version_status})", bot_data::GIT_TAG);

    let message_content = format!("{bot_version}\n{git_tag}");

    discord_utils::InteractionResponse::new(interaction, http_client)
        .send_message(discord_utils::Message::text(message_content).ephemeral())
        .await
        .log_error();
}
