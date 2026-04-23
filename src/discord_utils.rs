use twilight_http::response::marker::EmptyBody;

use crate::common::{ansi, discord};

/* Message */

pub enum MessageContent {
    Text(String),
    Components(Vec<discord::Component>),
}

pub struct Message {
    pub content: MessageContent,
    pub flags: discord::MessageFlags,
}

impl Message {
    pub fn text(content: impl Into<String>) -> Self {
        Message {
            content: MessageContent::Text(content.into()),
            flags: discord::MessageFlags::empty(),
        }
    }

    pub fn components(content: impl Into<Vec<discord::Component>>) -> Self {
        Message {
            content: MessageContent::Components(content.into()),
            flags: discord::MessageFlags::IS_COMPONENTS_V2,
        }
    }

    pub fn ephemeral(self) -> Self {
        self.enable_flags(discord::MessageFlags::EPHEMERAL)
    }

    pub fn enable_flags(mut self, flags: discord::MessageFlags) -> Self {
        self.flags.set(flags, true);
        self
    }
}

pub trait IntoMessage {
    fn into_message(self) -> Message;
}

impl<T> IntoMessage for T
where
    T: Into<String>,
{
    fn into_message(self) -> Message {
        Message::text(self)
    }
}

/* Modal */

pub struct Modal {
    pub title: String,
    pub custom_id: String,
    pub content: Vec<discord::Component>,
    pub flags: discord::MessageFlags,
}

impl Modal {
    pub fn new(
        custom_id: impl Into<String>,
        title: impl Into<String>,
        content: impl Into<Vec<discord::Component>>,
    ) -> Self {
        Modal {
            title: title.into(),
            custom_id: custom_id.into(),
            content: content.into(),
            flags: discord::MessageFlags::IS_COMPONENTS_V2,
        }
    }
}

/* Interaction response */

pub struct InteractionResponse<'a> {
    interaction: &'a discord::InteractionCreate,
    http_client: &'a discord::HttpClient,

    token_override: Option<&'a str>,
}

impl<'a> InteractionResponse<'a> {
    pub fn new(
        interaction: &'a discord::InteractionCreate,
        http_client: &'a discord::HttpClient,
    ) -> Self {
        Self {
            interaction,
            http_client,
            token_override: None,
        }
    }

    pub fn with_token(mut self, token: &'a str) -> Self {
        self.token_override = Some(token);
        self
    }

    pub async fn acknowledge(self) -> Result<twilight_http::Response<EmptyBody>, String> {
        use discord::InteractionResponseType::DeferredUpdateMessage;

        let response = discord::InteractionResponse {
            kind: DeferredUpdateMessage,
            data: None,
        };

        self.interaction_client()
            .create_response(self.interaction.id, self.token(), &response)
            .await
            .map_err(|err| format!("Couldn't acknowledge interaction: {err}"))
    }

    pub async fn show_loading(self) -> Result<twilight_http::Response<EmptyBody>, String> {
        use discord::InteractionResponseType::DeferredChannelMessageWithSource;

        let response = discord::InteractionResponse {
            kind: DeferredChannelMessageWithSource,
            data: None,
        };

        self.interaction_client()
            .create_response(self.interaction.id, self.token(), &response)
            .await
            .map_err(|err| format!("Couldn't send loading state interaction response: {err}"))
    }

    pub async fn update(
        self,
        message: Message,
    ) -> Result<twilight_http::Response<discord::Message>, String> {
        self.interaction_client()
            .update_response(self.token())
            .message_content(&message.content)
            .flags(message.flags)
            .await
            .map_err(|err| format!("Couldn't update interaction response: {err}"))
    }

    pub async fn delete_message(self) -> Result<twilight_http::Response<EmptyBody>, String> {
        // For whatever reason we first need to create deferred update response to delete the message.
        let response = discord::InteractionResponse {
            kind: discord::InteractionResponseType::DeferredUpdateMessage,
            data: None,
        };

        self.interaction_client()
            .create_response(self.interaction.id, self.token(), &response)
            .await
            .map_err(|err| {
                format!("Couldn't send defer update response for message deletion: {err}")
            })?;

        self.interaction_client()
            .delete_response(self.token())
            .await
            .map_err(|err| format!("Couldn't delete the message: {err}"))
    }

    pub async fn send_message(
        self,
        message: Message,
    ) -> Result<twilight_http::Response<EmptyBody>, String> {
        let response_data = discord::InteractionResponseDataBuilder::new();

        let response_data = response_data
            .message_content(message.content)
            .flags(message.flags);

        use discord::InteractionResponseType::ChannelMessageWithSource;
        let response = discord::InteractionResponse {
            kind: ChannelMessageWithSource,
            data: Some(response_data.build()),
        };

        self.interaction_client()
            .create_response(self.interaction.id, self.token(), &response)
            .await
            .map_err(|err| format!("Couldn't send interaction response: {err}"))
    }

    pub async fn update_message(
        self,
        message: Message,
    ) -> Result<twilight_http::Response<EmptyBody>, String> {
        let response_data = discord::InteractionResponseDataBuilder::new();

        let response_data = response_data
            .message_content(message.content)
            .flags(message.flags);

        use discord::InteractionResponseType::UpdateMessage;
        let response = discord::InteractionResponse {
            kind: UpdateMessage,
            data: Some(response_data.build()),
        };

        self.interaction_client()
            .create_response(self.interaction.id, self.token(), &response)
            .await
            .map_err(|err| format!("Couldn't update interaction response: {err}"))
    }

    pub async fn send_followup_message(
        self,
        message: Message,
    ) -> Result<twilight_http::Response<discord::Message>, String> {
        self.interaction_client()
            .create_followup(self.token())
            .message_content(&message.content)
            .flags(message.flags)
            .await
            .map_err(|err| format!("Couldn't send interaction response followup message: {err}"))
    }

    pub async fn show_modal(
        self,
        modal: Modal,
    ) -> Result<twilight_http::Response<EmptyBody>, String> {
        let response_data = discord::InteractionResponseDataBuilder::new()
            .custom_id(modal.custom_id)
            .title(modal.title)
            .components(modal.content)
            .flags(modal.flags);

        use discord::InteractionResponseType::Modal;

        let response = discord::InteractionResponse {
            kind: Modal,
            data: Some(response_data.build()),
        };

        self.interaction_client()
            .create_response(self.interaction.id, self.token(), &response)
            .await
            .map_err(|err| format!("Couldn't send modal interaction response: {err}"))
    }

    fn token(&self) -> &'a str {
        match self.token_override {
            Some(token_override) => token_override,
            None => &self.interaction.token,
        }
    }

    fn interaction_client(&self) -> discord::InteractionClient<'_> {
        self.http_client
            .interaction(self.interaction.application_id)
    }
}

/* Text input */

pub struct TextInputBuilder {
    pub custom_id: String,

    pub label: String,
    pub description: Option<String>,

    pub placeholder: Option<String>,
}

impl TextInputBuilder {
    pub fn new(custom_id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            custom_id: custom_id.into(),
            label: label.into(),
            description: None,
            placeholder: None,
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn build(self) -> discord::Component {
        let mut builder = discord::LabelBuilder::new(
            self.label,
            discord::Component::TextInput(discord::component::TextInput {
                id: None,
                custom_id: self.custom_id,
                max_length: None,
                min_length: Some(1),
                placeholder: self.placeholder,
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

/* Functions */

pub async fn error_message_response(
    text: impl Into<String>,
    interaction: &discord::InteractionCreate,
    http_client: &discord::HttpClient,
) {
    use crate::logging::{self, LogError as _};
    use colored::Colorize as _;

    logging::info!("Sending error message as interaction response");

    let text = format!("✗ {}", text.into());

    InteractionResponse::new(interaction, http_client)
        .send_message(ansi(text.red().to_string()).into_message().ephemeral())
        .await
        .map_err(|err| format!("Couldn't send error message response: {err}"))
        .log_error();
}

pub async fn followup_id(
    followup_response: twilight_http::Response<discord::Message>,
) -> Result<discord::MessageId, String> {
    followup_response
        .model()
        .await
        .map(|followup| followup.id)
        .map_err(|err| format!("Couldn't retrieve followup id: {err}"))
}

/* Private utility */

trait PutMessageContent {
    fn message_content(self, content: MessageContent) -> Self;
}

trait PutMessageContentRef<'a> {
    fn message_content(self, content: &'a MessageContent) -> Self;
}

impl PutMessageContent for discord::InteractionResponseDataBuilder {
    fn message_content(self, content: MessageContent) -> Self {
        match content {
            MessageContent::Text(text) => self.content(text),
            MessageContent::Components(components) => self.components(components),
        }
    }
}

impl<'a> PutMessageContentRef<'a> for discord::CreateFollowup<'a> {
    fn message_content(self, content: &'a MessageContent) -> Self {
        match content {
            MessageContent::Text(text) => self.content(text),
            MessageContent::Components(components) => self.components(components),
        }
    }
}

impl<'a> PutMessageContentRef<'a> for discord::UpdateResponse<'a> {
    fn message_content(self, content: &'a MessageContent) -> Self {
        match content {
            MessageContent::Text(text) => self.content(Some(text)),
            MessageContent::Components(components) => self.components(Some(components)),
        }
    }
}
