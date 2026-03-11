use crate::discord;

pub const COMMAND: &'static str = "edit_server";

pub fn create_command() -> twilight_model::application::command::Command {
    discord::CommandBuilder::new(
        COMMAND,
        "Modify one of the servers available for blueprints uploading",
        twilight_model::application::command::CommandType::ChatInput,
    )
    .build()
}
