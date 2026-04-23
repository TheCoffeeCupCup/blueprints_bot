use itertools::Itertools as _;

pub mod discord {
    pub use twilight_http::Client as HttpClient;
    pub use twilight_http::client::ClientBuilder;
    pub use twilight_http::client::InteractionClient;

    pub use twilight_model::channel::Attachment;
    pub use twilight_model::channel::Message;

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

    pub use twilight_model::application::interaction::application_command::CommandData;

    pub use twilight_model::id::Id;
    pub use twilight_model::id::marker;

    pub type UserId = Id<marker::UserMarker>;
    pub type RoleId = Id<marker::RoleMarker>;
    pub type GuildId = Id<marker::GuildMarker>;
    pub type MessageId = Id<marker::MessageMarker>;

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

    pub use twilight_http::request::application::interaction::CreateFollowup;
    pub use twilight_http::request::application::interaction::UpdateResponse;
}

pub type AnyError = Box<dyn std::error::Error + Send + Sync>;

pub fn ansi(formatted: impl Into<String>) -> String {
    let formatted: String = formatted.into();
    format!("```ansi\n{}\n```", formatted)
}

pub fn list_to_string<'a, T, I>(list: &'a T) -> String
where
    &'a T: IntoIterator<Item = I>,
    I: std::fmt::Debug,
{
    list.into_iter().map(|item| format!("{item:?}")).join(", ")
}

pub fn file_size_to_string(bytes: usize) -> String {
    humansize::format_size(bytes, humansize::BINARY)
}
