use colored::Colorize as _;

use crate::{AnyError, ansi, bot_data, commands, discord, ftp, logging};

pub const COMMAND: &'static str = "upload_blueprints";
pub const MODAL_ID: &'static str = "blueprints_upload_modal";

struct Blueprint {
    pub sbp_count: u8,
    pub sbpcfg_count: u8,
}

impl Default for Blueprint {
    fn default() -> Self {
        Self {
            sbp_count: 0,
            sbpcfg_count: 0,
        }
    }
}

pub struct Attachment {
    pub filename: String,
    pub url: String,
}

pub fn create_command() -> discord::Command {
    discord::CommandBuilder::new(
        COMMAND,
        "Upload blueprint files to a server",
        discord::CommandType::ChatInput,
    )
    .build()
}

pub async fn process_command(
    interaction: &discord::InteractionCreate,
    interaction_client: discord::InteractionClient<'_>,
) {
    let select_menu =
        bot_data::create_server_select_menu(None, None, Some(interaction.member.as_ref().unwrap()));

    let servers_amount = bot_data::get_data().servers.len();

    if servers_amount == 0 {
        logging::info!("Upload blueprints command issued while amount of servers is 0");

        let error = format!(
            "✗ No servers have been set up yet. Ask netrunners to add your server or use `/{}` if you are one.",
            commands::add_server::COMMAND
        );
        discord::negative_response(interaction, interaction_client, &error).await;

        return;
    }

    if select_menu.options.iter().len() == 0 {
        logging::info!("Non-uploader issued the upload blueprints command");

        let error = format!(
            "✗ You don't have access to blueprint uploading for any server. Ask netrunners for permission or use `/{}` if you are one.",
            commands::edit_server_uploaders::COMMAND
        );
        discord::negative_response(interaction, interaction_client, &error).await;

        return;
    }

    let server_select_label = discord::Component::Label(
        discord::LabelBuilder::new(
            "Servers to upload blueprints to",
            discord::Component::SelectMenu(select_menu),
        )
        .description("You can choose any amount of the servers you're allowed to upload to.")
        .build(),
    );

    let file_upload_label = discord::Component::Label(
        discord::LabelBuilder::new(
            "Files to upload",
            discord::Component::FileUpload(
                discord::FileUploadBuilder::new("blueprints_upload")
                    .min_values(2)
                    .max_values(10)
                    .required(true)
                    .build(),
            ),
        )
        .description(
            "You can send 1-5 blueprints, each consisting of a pair of .sbp and .sbpcfg files.",
        )
        .build(),
    );

    let data = discord::InteractionResponseDataBuilder::new()
        .title("Upload blueprint files")
        .custom_id(MODAL_ID)
        .flags(discord::MessageFlags::IS_COMPONENTS_V2)
        .components([server_select_label, file_upload_label])
        .build();

    let response = discord::InteractionResponse {
        kind: discord::InteractionResponseType::Modal,
        data: Some(data),
    };

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .unwrap();
}

fn get_selected_servers(
    submitted_components: &Vec<discord::ModalInteractionComponent>,
) -> Result<&Vec<String>, AnyError> {
    for component in submitted_components {
        if let discord::ModalInteractionComponent::Label(label) = component {
            if let discord::ModalInteractionComponent::StringSelect(select) =
                label.component.as_ref()
            {
                return Ok(&select.values);
            }
        }
    }

    Err("Selected servers not found in submitted components".into())
}

pub async fn process_modal_submition(
    interaction: &discord::InteractionCreate,
    submit_data: &discord::ModalInteractionData,
    interaction_client: discord::InteractionClient<'_>,
) {
    let mut files = Vec::<Attachment>::new();

    if let Some(resolved) = &submit_data.resolved {
        for (_, file) in &resolved.attachments {
            files.push(Attachment {
                filename: file.filename.clone(),
                url: file.url.clone(),
            });
        }
    }

    let selected_servers = get_selected_servers(&submit_data.components).unwrap();

    files.sort_by_cached_key(|f| f.filename.clone());

    let mut upload_files_future: Option<_> = None;
    let mut upload_files_text: Option<_> = None;

    let response_data = match verify_blueprints(&files, selected_servers) {
        Ok(text) => {
            upload_files_future = Some(tokio::spawn(ftp::upload_files(
                files,
                selected_servers.clone(),
            )));
            upload_files_text = Some(text.clone());

            discord::InteractionResponseDataBuilder::new()
                .content(ansi(text + "\n\nAwaiting for status update..."))
                .build()
        }
        Err(text) => discord::InteractionResponseDataBuilder::new()
            .content(ansi(text))
            .flags(discord::MessageFlags::EPHEMERAL) // In case of error the response will only be visible to the modal submitter.
            .build(),
    };

    // TODO: add real-time status report for each file (uploading, uploaded, error).

    let response = discord::InteractionResponse {
        kind: discord::InteractionResponseType::ChannelMessageWithSource,
        data: Some(response_data),
    };

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .unwrap();

    if let (Some(upload_files_future), Some(upload_files_text)) =
        (upload_files_future, upload_files_text)
    {
        let mut errors_list = String::new();

        if let Err(err) = upload_files_future.await.unwrap() {
            errors_list = format!("\n\n{}", &err.join("\n")).red().to_string();
        }

        interaction_client
            .update_response(&interaction.token)
            .content(Some(&ansi(upload_files_text + &errors_list)))
            .await
            .unwrap();
    }
}

fn verify_blueprints(
    attachments: &'_ Vec<Attachment>,
    servers: &Vec<String>,
) -> Result<String, String> {
    if attachments.len() == 0 {
        return Err(
            "Expected attached files to upload to the server but none were provided.".into(),
        );
    }

    let mut blueprints = std::collections::HashMap::<String, Blueprint>::new();

    for attachment in attachments {
        let full_file_name = &attachment.filename;
        let (file_name, file_extension) = full_file_name
            .rsplit_once('.')
            .unwrap_or((&full_file_name, ""));

        let entry = blueprints.entry(file_name.to_string()).or_default();

        match file_extension {
            "sbp" => entry.sbp_count += 1,
            "sbpcfg" => entry.sbpcfg_count += 1,
            _ => {}
        }
    }

    let mut response = String::new();
    let mut has_error = false;

    for attachment in attachments {
        let full_file_name = &attachment.filename;
        let (file_name, file_extension) = full_file_name
            .rsplit_once('.')
            .unwrap_or((&full_file_name, ""));

        if file_name.contains('/') || file_name.contains('\\') {
            let error = "you, hacking piece of shit, remove / and/or \\ from the filename";
            response += &format!("{}", format!("\n✗ {} ({})", full_file_name, error).red());

            has_error = true;
            continue;
        }

        if file_extension != "sbp" && file_extension != "sbpcfg" {
            let error = "wrong file extension, expected .sbp or .sbpcfg";
            response += &format!("{}", format!("\n✗ {} ({})", full_file_name, error).red());

            has_error = true;
            continue;
        }

        if let Some(entry) = blueprints.get(file_name) {
            if (file_extension == "sbp" && entry.sbp_count > 1)
                || (file_extension == "sbpcfg" && entry.sbpcfg_count > 1)
            {
                let error = "file with the same name is provided more than once";
                response += &format!("{}", format!("\n✗ {} ({})", full_file_name, error).red());

                has_error = true;
                continue;
            }

            if entry.sbp_count == 0 || entry.sbpcfg_count == 0 {
                let error = "a pair of .sbp and .sbpcfg is expected for each blueprint, but only one provided";
                response += &format!("{}", format!("\n✗ {} ({})", full_file_name, error).red());

                has_error = true;
                continue;
            }
        }

        // No errors
        response += &format!("{}", format!("\n✓ {}", full_file_name).green());
    }

    let error_prefix =
        "There are errors detected in the submitted files, nothing will be uploaded:"
            .red()
            .to_string();

    let success_prefix = "The following blueprint files are being uploaded:"
        .green()
        .to_string();

    let category_warning_suffix = "\n\n⚠ The blueprints will appear in the \"Unknown\" category when the servers are reloaded. Please move them when you log into the server to keep things organized.".yellow().to_string();

    let selected_servers = format!(
        "\n\nSelected servers: {}.",
        servers
            .iter()
            .map(|s| format!("\"{}\"", s))
            .collect::<Vec<_>>()
            .join(", ")
    );

    match has_error {
        false => Ok(success_prefix + &response + &category_warning_suffix + &selected_servers),
        true => Err(error_prefix + &response),
    }
}
