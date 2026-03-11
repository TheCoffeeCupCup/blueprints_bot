use colored::Colorize;

use crate::{AnyError, ansi, discord, get_env};

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

pub fn create_command() -> twilight_model::application::command::Command {
    discord::CommandBuilder::new(
        COMMAND,
        "Upload blueprint files to a server",
        twilight_model::application::command::CommandType::ChatInput,
    )
    .build()
}

pub async fn process_command(
    interaction: &discord::InteractionCreate,
    interaction_client: twilight_http::client::InteractionClient<'_>,
) -> Result<(), AnyError> {
    let servers = [
        discord::SelectMenuOptionBuilder::new(
            get_env("TEST_SERVER_NAME"),
            get_env("TEST_SERVER_NAME"),
        )
        .build(),
        discord::SelectMenuOptionBuilder::new("Server 2", "Server 2").build(),
        discord::SelectMenuOptionBuilder::new("Server 3", "Server 3").build(),
        discord::SelectMenuOptionBuilder::new("Server 4", "Server 4").build(),
        discord::SelectMenuOptionBuilder::new("Server 5", "Server 5").build(),
        discord::SelectMenuOptionBuilder::new("Server 6", "Server 6").build(),
    ];

    let mut select_menu = discord::SelectMenuBuilder::new(
        "server_select",
        twilight_model::channel::message::component::SelectMenuType::Text,
    )
    .min_values(1)
    .required(true);

    let servers_amount = servers.len();
    for server in servers {
        select_menu = select_menu.option(server);
    }

    let select_menu = select_menu
        .max_values(servers_amount.try_into().unwrap())
        .build();

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
        kind: twilight_model::http::interaction::InteractionResponseType::Modal,
        data: Some(data),
    };

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .unwrap();

    Ok(())
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
    interaction_client: twilight_http::client::InteractionClient<'_>,
) -> Result<(), AnyError> {
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

    let response_data = match verify_blueprints(&files, selected_servers) {
        Ok(text) => {
            tokio::spawn(crate::upload_files(files, selected_servers.clone()));
            discord::InteractionResponseDataBuilder::new()
                .content(text)
                .build()
        }
        Err(text) => discord::InteractionResponseDataBuilder::new()
            .content(text)
            .flags(discord::MessageFlags::EPHEMERAL) // In case of error the response will only be visible to the modal submitter.
            .build(),
    };

    // TODO: add status report (uploading, uploaded, error).

    let response = discord::InteractionResponse {
        kind: twilight_model::http::interaction::InteractionResponseType::ChannelMessageWithSource,
        data: Some(response_data),
    };

    interaction_client
        .create_response(interaction.id, &interaction.token, &response)
        .await
        .unwrap();

    Ok(())
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
            response += &format!("\n{}", format!("✗ {} ({})", full_file_name, error).red());

            has_error = true;
            continue;
        }

        if file_extension != "sbp" && file_extension != "sbpcfg" {
            let error = "wrong file extension, expected .sbp or .sbpcfg";
            response += &format!("\n{}", format!("✗ {} ({})", full_file_name, error).red());

            has_error = true;
            continue;
        }

        if let Some(entry) = blueprints.get(file_name) {
            if (file_extension == "sbp" && entry.sbp_count > 1)
                || (file_extension == "sbpcfg" && entry.sbpcfg_count > 1)
            {
                let error = "file with the same name is provided more than once";
                response += &format!("\n{}", format!("✗ {} ({})", full_file_name, error).red());

                has_error = true;
                continue;
            }

            if entry.sbp_count == 0 || entry.sbpcfg_count == 0 {
                let error = "a pair of .sbp and .sbpcfg is expected for each blueprint, but only one provided";
                response += &format!("\n{}", format!("✗ {} ({})", full_file_name, error).red());

                has_error = true;
                continue;
            }
        }

        // No errors
        response += &format!("\n{}", format!("✓ {}", full_file_name).green());
    }

    let error_prefix = "There are errors detected in the submitted files:"
        .red()
        .to_string();

    let success_prefix = "The following blueprint files are being uploaded:"
        .green()
        .to_string();

    let servers_suffix = format!(
        "\n\nSelected servers: {}.",
        servers
            .iter()
            .map(|s| format!("\"{}\"", s))
            .collect::<Vec<_>>()
            .join(", ")
    );

    match has_error {
        false => Ok(ansi(success_prefix + &response + &servers_suffix)),
        true => Err(ansi(error_prefix + &response)),
    }
}
