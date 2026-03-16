pub mod discord {
    pub use twilight_http::Client as HttpClient;

    pub use twilight_model::channel::message::Component;
    pub use twilight_model::channel::message::MessageFlags;
    pub use twilight_model::channel::message::component;

    pub use twilight_model::gateway::payload::incoming::InteractionCreate;

    pub use twilight_model::http::interaction::InteractionResponse;

    pub use twilight_model::application::interaction::InteractionData;

    pub use twilight_model::application::interaction::modal::ModalInteractionComponent;
    pub use twilight_model::application::interaction::modal::ModalInteractionData;

    pub use twilight_model::application::interaction::message_component::MessageComponentInteractionData;

    pub use twilight_model::id::Id;
    pub use twilight_model::id::marker;

    pub use twilight_model::guild::PartialMember;

    pub use twilight_gateway::Event;
    pub use twilight_gateway::EventTypeFlags;
    pub use twilight_gateway::Intents;
    pub use twilight_gateway::Shard;
    pub use twilight_gateway::ShardId;

    pub use twilight_cache_inmemory::DefaultInMemoryCache;
    pub use twilight_cache_inmemory::ResourceType;

    pub use twilight_util::builder::InteractionResponseDataBuilder;

    pub use twilight_util::builder::command::CommandBuilder;

    pub use twilight_util::builder::message::ActionRowBuilder;
    pub use twilight_util::builder::message::ButtonBuilder;
    pub use twilight_util::builder::message::FileUploadBuilder;
    pub use twilight_util::builder::message::LabelBuilder;
    pub use twilight_util::builder::message::SelectMenuBuilder;
    pub use twilight_util::builder::message::SelectMenuOptionBuilder;

    pub struct TextInputBuilder<'a> {
        pub custom_id: &'a str,

        pub label: &'a str,
        pub description: Option<&'a str>,

        pub placeholder: Option<&'a str>,
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

pub fn get_env(var: &'static str) -> String {
    std::env::var(var).expect(&format!(
        "{} environment variable is not found. Consider adding it in the .env file in the workdir.",
        var
    ))
}

pub fn ansi(formatted: String) -> String {
    format!("```ansi\n{}\n```", formatted)
}
