pub mod discord {
    pub use twilight_http::Client as HttpClient;

    pub use twilight_model::channel::message::Component;
    pub use twilight_model::channel::message::MessageFlags;

    pub use twilight_model::gateway::payload::incoming::InteractionCreate;

    pub use twilight_model::http::interaction::InteractionResponse;

    pub use twilight_model::application::interaction::InteractionData;
    pub use twilight_model::application::interaction::modal::ModalInteractionComponent;
    pub use twilight_model::application::interaction::modal::ModalInteractionData;

    pub use twilight_model::id::Id;
    pub use twilight_model::id::marker::RoleMarker;
    pub use twilight_model::id::marker::UserMarker;

    pub use twilight_gateway::Event;
    pub use twilight_gateway::EventTypeFlags;
    pub use twilight_gateway::Intents;
    pub use twilight_gateway::Shard;
    pub use twilight_gateway::ShardId;

    pub use twilight_cache_inmemory::DefaultInMemoryCache;
    pub use twilight_cache_inmemory::ResourceType;

    pub use twilight_util::builder::InteractionResponseDataBuilder;

    pub use twilight_util::builder::command::CommandBuilder;

    pub use twilight_util::builder::message::FileUploadBuilder;
    pub use twilight_util::builder::message::LabelBuilder;
    pub use twilight_util::builder::message::SelectMenuBuilder;
    pub use twilight_util::builder::message::SelectMenuOptionBuilder;
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
