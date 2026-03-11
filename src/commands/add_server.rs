use twilight_model::channel::message::component::{TextInput, TextInputStyle};

use crate::{AnyError, discord};

pub const COMMAND: &'static str = "add_server";
pub const MODAL_ID: &'static str = "add_server_modal";

pub fn create_command() -> twilight_model::application::command::Command {
    discord::CommandBuilder::new(
        COMMAND,
        "Adds a new server for blueprints uploading",
        twilight_model::application::command::CommandType::ChatInput,
    )
    .build()
}

struct TextInputBuilder<'a> {
    custom_id: &'a str,

    label: &'a str,
    description: Option<&'a str>,

    placeholder: Option<&'a str>,
}

impl TextInputBuilder<'_> {
    fn build(self) -> discord::Component {
        let mut builder = discord::LabelBuilder::new(
            self.label,
            discord::Component::TextInput(TextInput {
                id: None,
                custom_id: self.custom_id.to_string(),
                max_length: None,
                min_length: Some(1),
                placeholder: self.placeholder.map(|p| p.to_string()),
                required: None,
                style: TextInputStyle::Short,
                value: None,

                #[allow(deprecated)]
                label: None,
            }),
        );

        if let Some(description) = self.description {
            builder = builder.description(description);
        }

        discord::Component::Label(builder.build())
    }
}

pub async fn process_command(
    interaction: &discord::InteractionCreate,
    interaction_client: twilight_http::client::InteractionClient<'_>,
) -> Result<(), AnyError> {
    let components = [
        TextInputBuilder {
            custom_id: "server_name",
            label: "Server name",
            description: None,
            placeholder: Some("NAC-2"),
        }
        .build(),
        TextInputBuilder {
            custom_id: "full_ip",
            label: "Full IP address",
            description: Some("IP and FTP port must be combined with a colon (:)."),
            placeholder: Some("123.45.67.89:21"),
        }
        .build(),
        TextInputBuilder {
            custom_id: "ftp_username",
            label: "FTP username",
            description: None,
            placeholder: None,
        }
        .build(),
        TextInputBuilder {
            custom_id: "ftp_password",
            label: "FTP password",
            description: None,
            placeholder: None,
        }
        .build(),
        TextInputBuilder {
            custom_id: "world_name",
            label: "Satisfactory world name",
            description: Some(
                "The name of the folder under `.config/Epic/FactoryGame/Saved/SaveGames/blueprints/`.",
            ),
            placeholder: None,
        }
        .build(),
    ];

    let data = discord::InteractionResponseDataBuilder::new()
        .title("Add a server for blueprint uploading")
        .custom_id(MODAL_ID)
        .flags(discord::MessageFlags::IS_COMPONENTS_V2)
        .components(components)
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
