use std::collections::HashSet;
use std::sync::{Arc, LazyLock, RwLock, RwLockReadGuard};

use serde::{Deserialize, Serialize};

use crate::{bot_data, discord, logging};

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
    bot_data::get_data()
        .servers
        .get(server_name)
        .map(|s| s.credentials.clone())
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum Mentionable {
    User(discord::Id<discord::marker::UserMarker>),
    Role(discord::Id<discord::marker::RoleMarker>),
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

///////////////////////////////////////////////////

const BOT_DATA_FILE: &'static str = "bot_data.json";
const BACKUP_BOT_DATA_FILE: &'static str = "bot_data_backup.json";

static BOT_DATA: LazyLock<Arc<RwLock<BotData>>> = LazyLock::new(|| {
    backup_data_file();

    let data = load_data().unwrap_or_default();

    Arc::new(RwLock::new(data))
});

fn load_data() -> Option<BotData> {
    logging::info!("Loading bot data");

    let data_json = std::fs::read_to_string(BOT_DATA_FILE)
        .map_err(|err| logging::warning!("Couldn't read bot data file: {}", err))
        .ok()?;

    let data = serde_json::from_str(&data_json)
        .map_err(|err| logging::warning!("Couldn't parse bot data file: {}", err))
        .ok();

    data
}

fn save_data() {
    logging::info!("Saving bot data");

    if let Some(file) = std::fs::File::create(BOT_DATA_FILE)
        .map_err(|err| logging::error!("Couldn't create a file for bot data: {}", err))
        .ok()
    {
        let data: &BotData = &get_data();
        serde_json::to_writer_pretty(file, data)
            .map_err(|err| logging::error!("Couldn't write bot data to a file: {}", err))
            .ok();
    }
}

fn backup_data_file() {
    logging::info!("Backing up bot data");

    let data = std::fs::read_to_string(BOT_DATA_FILE)
        .map_err(|err| logging::warning!("Couldn't read bot data file: {}", err));

    if let Ok(data) = data {
        std::fs::write(BACKUP_BOT_DATA_FILE, data)
            .map_err(|err| logging::error!("Couldn't write bot data backup: {}", err))
            .ok();
    }
}

pub fn get_data() -> RwLockReadGuard<'static, BotData> {
    BOT_DATA.read().expect("Failed to lock bot data mutex.")
}

pub fn update_data<F>(updater: F)
where
    F: FnOnce(&mut BotData),
{
    if let Some(mut data) = BOT_DATA
        .write()
        .map_err(|err| logging::error!("Couldn't lock bot data for write {}", err))
        .ok()
    {
        updater(&mut data);
    }

    save_data();
}

///////////////////////////////////////////////////

pub fn create_server_select_menu(
    amount_limit: Option<u8>,
    default_value: Option<&str>,
    issuing_user: Option<&discord::PartialMember>,
) -> discord::component::SelectMenu {
    let mut servers = Vec::new();

    for (server_name, server_data) in &bot_data::get_data().servers {
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

    let mut select_menu =
        discord::SelectMenuBuilder::new("server_select", discord::component::SelectMenuType::Text)
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
