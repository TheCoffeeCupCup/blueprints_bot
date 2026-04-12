use itertools::Itertools as _;

pub mod discord {
    pub use twilight_http::Client as HttpClient;
    pub use twilight_http::client::ClientBuilder;
    pub use twilight_http::client::InteractionClient;

    pub use twilight_model::channel::message::AllowedMentions;
    pub use twilight_model::channel::message::Component;
    pub use twilight_model::channel::message::MessageFlags;
    pub use twilight_model::channel::message::component;

    pub use twilight_model::gateway::payload::incoming::InteractionCreate;

    pub use twilight_model::http::interaction::InteractionResponse;
    pub use twilight_model::http::interaction::InteractionResponseType;

    pub use twilight_model::application::command::Command;
    pub use twilight_model::application::command::CommandType;

    pub use twilight_model::application::interaction::InteractionData;

    pub use twilight_model::application::interaction::modal::ModalInteractionComponent;
    pub use twilight_model::application::interaction::modal::ModalInteractionData;

    pub use twilight_model::application::interaction::message_component::MessageComponentInteractionData;

    pub use twilight_model::id::Id;
    pub use twilight_model::id::marker;

    pub use twilight_model::guild::PartialMember;
    pub use twilight_model::guild::Permissions;

    pub use twilight_gateway::Event;
    pub use twilight_gateway::EventTypeFlags;
    pub use twilight_gateway::Intents;
    pub use twilight_gateway::Shard;
    pub use twilight_gateway::ShardId;

    pub use twilight_util::builder::InteractionResponseDataBuilder;

    pub use twilight_util::builder::command::CommandBuilder;

    pub use twilight_util::builder::message::ActionRowBuilder;
    pub use twilight_util::builder::message::ButtonBuilder;
    pub use twilight_util::builder::message::FileUploadBuilder;
    pub use twilight_util::builder::message::LabelBuilder;
    pub use twilight_util::builder::message::SelectMenuBuilder;
    pub use twilight_util::builder::message::SelectMenuOptionBuilder;
    pub use twilight_util::builder::message::TextDisplayBuilder;

    pub struct TextInputBuilder<'a> {
        pub custom_id: &'a str,

        pub label: &'a str,
        pub description: Option<&'a str>,

        pub placeholder: Option<&'a str>,
    }

    pub async fn negative_response(
        interaction: &InteractionCreate,
        interaction_client: &InteractionClient<'_>,
        text: &str,
    ) {
        use colored::Colorize as _;

        let data = InteractionResponseDataBuilder::new()
            .content(super::ansi(text.red().to_string()))
            .flags(MessageFlags::EPHEMERAL)
            .build();

        let response = InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(data),
        };

        interaction_client
            .create_response(interaction.id, &interaction.token, &response)
            .await
            .map_err(|err| {
                crate::logging::error!("Couldn't send negative response \"{text}\": {err}")
            })
            .ok();
    }

    pub async fn delete_interaction_message(
        interaction: &InteractionCreate,
        interaction_client: &InteractionClient<'_>,
    ) {
        // For whatever reason I first need to create deferred update response to delete the message.
        let response = InteractionResponse {
            kind: InteractionResponseType::DeferredUpdateMessage,
            data: None,
        };

        interaction_client
            .create_response(interaction.id, &interaction.token, &response)
            .await
            .map_err(|err| {
                crate::logging::error!(
                    "Couldn't send defer update response for message deletion: {err}"
                );
            })
            .ok();

        interaction_client
            .delete_response(&interaction.token)
            .await
            .map_err(|err| {
                crate::logging::error!("Couldn't delete the message: {err}");
            })
            .ok();
    }
}

impl discord::TextInputBuilder<'_> {
    pub fn build(self) -> discord::Component {
        let mut builder = discord::LabelBuilder::new(
            self.label,
            discord::Component::TextInput(discord::component::TextInput {
                id: None,
                custom_id: self.custom_id.to_string(),
                max_length: None,
                min_length: Some(1),
                placeholder: self.placeholder.map(|p| p.to_string()),
                required: None,
                style: discord::component::TextInputStyle::Short,
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

pub type AnyError = Box<dyn std::error::Error + Send + Sync>;

pub fn ansi(formatted: String) -> String {
    format!("```ansi\n{}\n```", formatted)
}

pub fn list_to_string<'a, T, I>(list: &'a T) -> String
where
    &'a T: IntoIterator<Item = I>,
    I: std::fmt::Debug,
{
    list.into_iter().map(|item| format!("{item:?}")).join(", ")
}
