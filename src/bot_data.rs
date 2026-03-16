use std::collections::HashSet;
use std::io::Read;
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};

use crate::{bot_data, discord};

#[derive(PartialEq, Debug, Serialize, Deserialize, Clone)]
pub enum ConnectionType {
    FTP,
    // SFTP,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerCredentials {
    pub connection: ConnectionType,
    pub full_ip: String,
    pub user: String,
    pub password: String,
    pub world_name: String,
}

pub fn get_server_creds(server_name: &str) -> Option<ServerCredentials> {
    BOT_DATA
        .lock()
        .unwrap()
        .servers
        .get(server_name)
        .map(|s| s.credentials.clone())
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum Mentionable {
    User(twilight_model::id::Id<discord::marker::UserMarker>),
    Role(twilight_model::id::Id<discord::marker::RoleMarker>),
}

impl Mentionable {
    pub fn to_mention(&self) -> String {
        match self {
            Mentionable::User(user) => format!("<@{}>", user.get()),
            Mentionable::Role(role) => format!("<@&{}>", role.get()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Server {
    pub credentials: ServerCredentials,
    pub uploaders: HashSet<Mentionable>,
}

impl Server {
    pub fn new(credentials: ServerCredentials) -> Self {
        Self {
            credentials,
            uploaders: HashSet::default(),
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct BotData {
    pub servers: std::collections::HashMap<String, Server>,
}

pub static BOT_DATA: LazyLock<Mutex<BotData>> = LazyLock::new(|| Mutex::new(BotData::default()));

const BOT_DATA_FILE: &'static str = "bot_data.json";

pub fn save_data() {
    let file = std::fs::File::create(BOT_DATA_FILE).unwrap();
    let data: &BotData = &BOT_DATA.lock().unwrap();
    serde_json::to_writer_pretty(file, data).unwrap();
}

pub fn load_data() {
    if let Ok(mut file) = std::fs::File::open(BOT_DATA_FILE) {
        let mut bot_data_json = String::new();
        std::fs::File::read_to_string(&mut file, &mut bot_data_json).unwrap();

        if let Ok(data) = serde_json::from_str::<BotData>(&bot_data_json) {
            BOT_DATA.lock().unwrap().servers = data.servers;
        } else {
            println!("Error parsing bot data.");
        }
    }
}

pub fn create_server_select_menu(
    amount_limit: Option<u8>,
    default_value: Option<&str>,
    issuing_user: Option<&discord::PartialMember>,
) -> discord::component::SelectMenu {
    let mut servers = Vec::new();

    for (server_name, server_data) in &bot_data::BOT_DATA.lock().unwrap().servers {
        if let Some(issuing_user) = issuing_user {
            if !server_data
                .uploaders
                .iter()
                .find(|uploader| {
                    match uploader {
                        Mentionable::User(uploader_id) => {
                            if issuing_user.user.as_ref().unwrap().id == *uploader_id {
                                return true;
                            }
                        }
                        Mentionable::Role(role_id) => {
                            if issuing_user.roles.contains(role_id) {
                                return true;
                            }
                        }
                    }

                    return false;
                })
                .is_some()
            {
                continue;
            }
        }

        let mut select_menu =
            discord::SelectMenuOptionBuilder::new(server_name, server_name).build();

        if let Some(default_value) = default_value {
            if default_value == server_name {
                select_menu.default = true;
            }
        }

        servers.push(select_menu);
    }

    let mut select_menu = discord::SelectMenuBuilder::new(
        "server_select",
        twilight_model::channel::message::component::SelectMenuType::Text,
    )
    .min_values(1)
    .required(true);

    if let Some(limit) = amount_limit {
        select_menu = select_menu.max_values(limit);
    }

    let servers_amount = servers.len();
    for server in servers {
        select_menu = select_menu.option(server);
    }

    select_menu
        .max_values(servers_amount.try_into().unwrap())
        .build()
}
