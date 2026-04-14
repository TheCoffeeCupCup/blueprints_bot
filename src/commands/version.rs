/* Constants */

use crate::{bot_data, common::discord, logging};

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
    interaction_client: discord::InteractionClient<'_>,
) {
    logging::info!("Processing command `/{COMMAND}`");
    process_command_impl(interaction, interaction_client).await;
    logging::info!("Finished processing command `/{COMMAND}`");
}

/* Impl functions */

async fn process_command_impl(
    interaction: &discord::InteractionCreate,
    interaction_client: discord::InteractionClient<'_>,
) {
    let bot_version = format!("Bot version: `{}`", bot_data::CARGO_PKG_VERSION);
    let git_tag = format!("Git tag: `{}`", bot_data::GIT_TAG);

    let data = discord::InteractionResponseDataBuilder::new()
        .content(format!("{bot_version}\n{git_tag}"))
        .flags(discord::MessageFlags::EPHEMERAL)
        .build();

    let response = discord::InteractionResponse {
        kind: discord::InteractionResponseType::ChannelMessageWithSource,
        data: Some(data),
    };

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .map_err(|err| crate::logging::error!("Couldn't display bot's version: {err}"))
        .ok();
}
