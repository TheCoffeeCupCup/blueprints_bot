use std::collections::HashSet;
use std::sync::{Arc, LazyLock, Once, RwLock, RwLockReadGuard};

use serde::{Deserialize, Serialize};

use crate::{bot_data, discord, encryption, logging, secrets};

pub const CARGO_PKG_VERSION: &'static str = env!("CARGO_PKG_VERSION");
pub const GIT_TAG: &'static str = env!("GIT_TAG");

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerCredentials {
    pub full_ip: String,
    pub user: String,
    pub password: String,
    pub session_name: String,
}

pub fn get_server_creds(server_name: &str) -> Option<ServerCredentials> {
    bot_data::get_data()
        .servers
        .get(server_name)
        .map(|s| s.credentials.clone())
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Copy)]
pub enum Mentionable {
    User(discord::UserId),
    Role(discord::RoleId),
}

impl Mentionable {
    pub fn to_mention(&self) -> String {
        match self {
            Mentionable::User(user) => format!("<@{}>", user.get()),
            Mentionable::Role(role) => format!("<@&{}>", role.get()),
        }
    }

    pub fn corresponds_to_member(&self, member: &discord::PartialMember) -> bool {
        match self {
            Mentionable::User(uploader_id) => {
                if let Some(user) = member.user.as_ref() {
                    if user.id == *uploader_id {
                        return true;
                    }
                } else {
                    logging::error!(
                        "Couldn't resolve the user while checking corresponds_to_member"
                    );
                }
            }
            Mentionable::Role(role_id) => {
                if member.roles.contains(role_id) {
                    return true;
                }
            }
        }

        return false;
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

    #[serde(default)]
    pub next_component_id: u64,

    #[serde(default)]
    sftp_migration_completed: bool,
}

///////////////////////////////////////////////////

fn get_bot_data_keys() -> Result<encryption::Passphrase, String> {
    let primary = secrets::bot_data_key();
    let secondary = secrets::last_bot_data_key();

    if primary.is_empty() {
        return Err("Bot data key must not be empty".to_string());
    }

    let secondary = if secondary.is_empty() {
        None
    } else {
        Some(secondary)
    };

    Ok(encryption::Passphrase { primary, secondary })
}

///////////////////////////////////////////////////

const BOT_DATA_FILE: &'static str = "bot_data.bin";
const BACKUP_BOT_DATA_FILE: &'static str = "bot_data_backup.bin";

static BOT_DATA: LazyLock<Arc<RwLock<BotData>>> = LazyLock::new(|| {
    let mut data = load_data().unwrap_or_else(|| {
        logging::warning!("Loading bot data from the default value");
        BotData::default()
    });

    if !data.sftp_migration_completed {
        perform_sftp_migration(&mut data);
    }

    Arc::new(RwLock::new(data))
});

fn load_data() -> Option<BotData> {
    logging::info!("Lazily loading the bot data");

    let encrypted_data = std::fs::read(BOT_DATA_FILE)
        .map_err(|err| logging::warning!("Couldn't read bot data file: {err}"))
        .ok()?;

    let passphrase = get_bot_data_keys()
        .map_err(|err| logging::error!("Couldn't retrieve bot data keys: {err}"))
        .ok()?;

    let decrypted_data = encryption::decrypt(&encrypted_data, &passphrase)?;

    let data = serde_json::from_slice(&decrypted_data)
        .map_err(|err| logging::warning!("Couldn't parse bot data file: {err}"))
        .ok();

    logging::info!("Bot data is loaded from the file");

    data
}

fn save_data() {
    static BACKUP: Once = Once::new();

    BACKUP.call_once(|| {
        backup_data_file();
    });

    logging::info!("Saving bot data");

    let data: &BotData = &get_data();

    let bytes = match serde_json::to_vec(data) {
        Ok(bytes) => bytes,
        Err(err) => {
            logging::error!("Couldn't serialize bot data: {err}");
            return;
        }
    };

    let passphrase = match get_bot_data_keys() {
        Ok(bytes) => bytes,
        Err(err) => {
            logging::error!("Couldn't retrieve bot data key: {err}");
            return;
        }
    };

    let encrypted_data = match encryption::encrypt(&bytes, &passphrase.primary) {
        Ok(encrypted_data) => encrypted_data,
        Err(err) => {
            logging::error!("Couldn't encrypt bot data: {err}");
            return;
        }
    };

    std::fs::write(BOT_DATA_FILE, encrypted_data)
        .map_err(|err| logging::error!("Couldn't write bot data to a file: {err}"))
        .ok();

    logging::info!("Bot data successfully saved");
}

fn backup_data_file() {
    logging::info!("Backing up the bot data");

    std::fs::copy(BOT_DATA_FILE, BACKUP_BOT_DATA_FILE)
        .map_err(|err| logging::error!("Couldn't copy bot data into backup: {err}"))
        .ok();
}

pub fn get_data() -> RwLockReadGuard<'static, BotData> {
    BOT_DATA
        .read()
        .map_err(|err| logging::error!("Failed to lock bot data mutex: {err}"))
        .unwrap()
}

pub fn update_data<F>(updater: F)
where
    F: FnOnce(&mut BotData),
{
    if let Some(mut data) = BOT_DATA
        .write()
        .map_err(|err| logging::error!("Couldn't lock bot data for write {err}"))
        .ok()
    {
        updater(&mut data);
    }

    save_data();
}

fn perform_sftp_migration(data: &mut BotData) {
    logging::warning!("Performing SFTP migration");

    for (name, server) in data.servers.iter_mut() {
        let full_ip = &mut server.credentials.full_ip;

        if let Some(stripped) = full_ip.strip_suffix(":21") {
            logging::warning!(
                "SFTP migration: Replacing \":21\" with \":22\" in \"{name}\" server's IP"
            );

            *full_ip = format!("{}:22", stripped.to_string());
        }
    }

    data.sftp_migration_completed = true;

    logging::info!("SFTP migration complete");
}

///////////////////////////////////////////////////

pub fn create_server_select_menu(
    amount_limit: Option<u8>,
    default_value: Option<&str>,
    issuing_user: Option<&discord::PartialMember>,
) -> discord::component::SelectMenu {
    create_server_select_menu_custom_id(amount_limit, default_value, issuing_user, "server_select")
}

pub fn create_server_select_menu_custom_id(
    amount_limit: Option<u8>,
    default_value: Option<&str>,
    issuing_user: Option<&discord::PartialMember>,
    custom_id: &str,
) -> discord::component::SelectMenu {
    let mut servers = Vec::new();

    for (server_name, server_data) in &bot_data::get_data().servers {
        if let Some(issuing_user) = issuing_user {
            if !server_data
                .uploaders
                .iter()
                .find(|uploader| uploader.corresponds_to_member(issuing_user))
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
        discord::SelectMenuBuilder::new(custom_id, discord::component::SelectMenuType::Text)
            .min_values(1)
            .required(true);

    let servers_amount = servers.len();
    for server in servers {
        select_menu = select_menu.option(server);
    }

    select_menu = match amount_limit {
        Some(limit) => select_menu.max_values(limit),
        None => select_menu.max_values(servers_amount.try_into().unwrap()),
    };

    select_menu.build()
}

pub async fn get_git_version_status(current_version: &str) -> String {
    const REPO_NAME: &'static str = "TheCoffeeCupCup/blueprints_bot";
    let repo_url = format!("https://api.github.com/repos/{REPO_NAME}/commits/main");

    let client = reqwest::Client::new();
    let response = client
        .get(&repo_url)
        // GitHub requires a User-Agent
        .header(reqwest::header::USER_AGENT, "rust-reqwest-client")
        // Will return only the commit SHA string
        .header(reqwest::header::ACCEPT, "application/vnd.github.sha")
        .send()
        .await;

    let error_text = "couldn't check latest version".to_string();

    let Ok(response) = response
        .map_err(|err| logging::error!("Error retrieving latest commit hash from GitHub: {err}"))
    else {
        return error_text;
    };

    let Ok(remote_hash) = response
        .text()
        .await
        .map_err(|err| logging::error!("Error retrieving latest commit hash from GitHub: {err}"))
    else {
        return error_text;
    };

    let current_version = current_version
        .strip_suffix("-dirty")
        .unwrap_or(current_version);

    let short_hash_size = current_version.len();
    let short_hash = &remote_hash[..short_hash_size];

    if current_version == short_hash {
        logging::info!("Bot is up-to-date");
        format!("up-to-date")
    } else {
        logging::warning!("Bot is outdated. Current: `{current_version}`. Latest: `{remote_hash}`");
        format!("outdated - latest is `{remote_hash}`")
    }
}
