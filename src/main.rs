use colored::Colorize;
use std::collections::HashMap;
use std::collections::hash_map::Entry;

use serenity::async_trait;
use serenity::prelude::*;

use serenity::all as discord;

struct Handler;

pub type AnyError = Box<dyn std::error::Error>;

struct Blueprint<'a> {
    pub sbp: Option<&'a serenity::all::Attachment>,
    pub sbpcfg: Option<&'a serenity::all::Attachment>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct UsersList {
    users: Vec<discord::UserId>,
    roles: Vec<discord::RoleId>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct GameServer {
    name: String,

    address: String,
    ftp_username: String,
    ftp_password: String,
    upload_path: String,

    uploaders: UsersList, // People who are allowed to upload blueprints to the server.
}

#[derive(serde::Serialize, serde::Deserialize)]
struct BotData {
    admins: UsersList, // People who can manage servers and uploaders.
    servers: Vec<GameServer>,
}

fn verify_blueprints(
    attachments: &'_ Vec<discord::Attachment>,
) -> Result<HashMap<String, Blueprint<'_>>, String> {
    if attachments.len() == 0 {
        return Err(
            "Expected attached files to upload to the server but none are provided, dummy.".into(),
        );
    }

    let mut blueprints = HashMap::new();

    for attachment in attachments {
        let full_file_name = &attachment.filename;
        let (file_name, file_extension) = full_file_name
            .rsplit_once('.')
            .unwrap_or((&full_file_name, ""));

        if file_extension != "sbp" && file_extension != "sbpcfg" {
            return Err(format!(
                "Wrong file extension \".{}\" in \"{}\". Expected \".sbp\" or \".sbpcfg\", dummy.",
                file_extension, full_file_name
            ));
        }

        match blueprints.entry(file_name.to_string()) {
            Entry::Vacant(entry) => {
                if file_extension == "sbp" {
                    entry.insert(Blueprint {
                        sbp: Some(attachment),
                        sbpcfg: None,
                    });
                } else if file_extension == "sbpcfg" {
                    entry.insert(Blueprint {
                        sbp: None,
                        sbpcfg: Some(attachment),
                    });
                }
            }
            Entry::Occupied(mut entry) => {
                if file_extension == "sbp" {
                    if entry.get().sbp.is_some() {
                        return Err(format!(
                            "The same file \"{}\" is sent twice, dummy.",
                            full_file_name
                        ));
                    } else {
                        entry.get_mut().sbp = Some(attachment);
                    }
                } else if file_extension == "sbpcfg" {
                    if entry.get().sbpcfg.is_some() {
                        return Err(format!(
                            "The same file \"{}\" is sent twice, dummy.",
                            full_file_name
                        ));
                    } else {
                        entry.get_mut().sbpcfg = Some(attachment);
                    }
                }
            }
        }
    }

    for (blueprint_name, blueprint_files) in &blueprints {
        if blueprint_files.sbp.is_none() {
            return Err(format!(
                "Blueprint \"{}\" is missing the sbp file, dummy.",
                blueprint_name
            ));
        }

        if blueprint_files.sbpcfg.is_none() {
            return Err(format!(
                "Blueprint \"{}\" is missing the sbpcfg file, dummy.",
                blueprint_name
            ));
        }
    }

    Ok(blueprints)
}

fn ansi(formatted: &colored::ColoredString) -> String {
    format!("```ansi\n{}\n```", formatted)
}

fn parameter_mentionable(description: &str) -> discord::CreateCommandOption {
    discord::CreateCommandOption::new(discord::CommandOptionType::Mentionable, "who", description)
        .required(true)
}

fn parameter_server_name() -> discord::CreateCommandOption {
    discord::CreateCommandOption::new(
        discord::CommandOptionType::String,
        "server",
        "Name of the server",
    )
    .required(true)
}

async fn try_process_blueprint_upload(ctx: Context, interaction: discord::Interaction) {
    let Some(command) = interaction.command() else {
        return;
    };

    if command.data.name != "upload" {
        return;
    }

    let [subcommand] = command.data.options.as_slice() else {
        return;
    };

    if subcommand.name != "blueprints" {
        return;
    }

    let upload_blueprints_modal = discord::CreateInteractionResponse::Modal(
        discord::CreateModal::new("upload_blueprints_modal", "Upload blueprints").components(
            [discord::CreateActionRow::InputText(
                discord::CreateInputText::new(discord::InputTextStyle::Short, "Test", "MyId"),
            )]
            .to_vec(),
        ),
    );

    let response = command.create_response(ctx, upload_blueprints_modal).await;

    if let Err(err) = response {
        println!("Error creating modal for blueprint upload command: {}", err);
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, interaction: discord::Interaction) {
        try_process_blueprint_upload(ctx, interaction).await;
    }

    async fn ready(&self, ctx: Context, _ready: discord::Ready) {
        let command_upload_blueprints = discord::CreateCommand::new("upload")
            .description("Upload files to a specified server")
            .add_option(
                discord::CreateCommandOption::new(
                    discord::CommandOptionType::SubCommand,
                    "blueprints",
                    "Upload blueprint files to a specified server",
                )
                .add_sub_option(parameter_server_name()),
            )
            .add_option(
                discord::CreateCommandOption::new(
                    discord::CommandOptionType::SubCommand,
                    "blueprintsa",
                    "Upload blueprint files to a specified server",
                )
                .add_sub_option(parameter_server_name()),
            );

        let command_uploaders_add = discord::CreateCommand::new("uploaders")
            .description("Manage uploaders for a specified server")
            .add_option(
                discord::CreateCommandOption::new(
                    discord::CommandOptionType::SubCommand,
                    "add",
                    "Add user or role to the uploaders list for the specified server",
                )
                .add_sub_option(parameter_server_name())
                .add_sub_option(parameter_mentionable(
                    "User or role to add to the uploaders list for the specified server",
                )),
            )
            .add_option(
                discord::CreateCommandOption::new(
                    discord::CommandOptionType::SubCommand,
                    "remove",
                    "Remove user or role from the uploaders list for the specified server",
                )
                .add_sub_option(parameter_server_name())
                .add_sub_option(
                    discord::CreateCommandOption::new(
                        discord::CommandOptionType::Attachment,
                        "att",
                        "attata",
                    )
                    .required(true),
                ),
            );

        // For testing (Instant): Using a specific Guild ID
        let guild_id = discord::GuildId::new(std::env::var("TEST_GUILD_ID").expect(
            "TEST_GUILD_ID environment variable is not found. Consider adding it in the .env file in the workdir.",
        ).parse().expect("Error parsing TEST_GUILD_ID"));

        if let Err(why) = guild_id
            .set_commands(&ctx, vec![command_uploaders_add, command_upload_blueprints])
            .await
        {
            println!("Error registering guild commands: {:?}", why);
        }

        /* For Production (Global - can take an hour):
        if let Err(why) = Command::set_global_commands(&ctx.http, vec![command_uploaders_add]).await {
            println!("Error registering global commands: {:?}", why);
        }
        */
    }

    async fn message(&self, ctx: Context, msg: discord::Message) {
        if msg.content.starts_with("/") {
            println!("Executed {}", msg.content);
        }

        if msg.content.starts_with("/upload-blueprints") {
            let response = match verify_blueprints(&msg.attachments) {
                Err(err) => ansi(&format!("⚠ {} ⚠", err).yellow().bold()),
                Ok(blueprints) => {
                    let mut success_message = format!(
                        "Yes sir! Uploading {} blueprints to the server:",
                        blueprints.len()
                    );

                    for (blueprint_name, _) in &blueprints {
                        success_message += &format!("\n✔ \"{}\"", blueprint_name);
                    }

                    ansi(&success_message.green().bold())
                }
            };

            if let Err(err) = msg.channel_id.say(&ctx.http, response).await {
                println!("Error sending message: {err}");
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    dotenv::dotenv()?;

    let token = std::env::var("DISCORD_TOKEN").expect(
        "DISCORD_TOKEN environment variable is not found. Consider adding it in the .env file in the workdir.",
    );
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    if let Err(err) = client.start().await {
        println!("Client error: {err}");
    }

    Ok(())
}
