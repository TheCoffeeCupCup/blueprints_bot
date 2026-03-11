use crate::{discord, get_env};

#[derive(PartialEq)]
pub enum ConnectionType {
    FTP,
    // SFTP,
}

pub struct ServerCredentials {
    pub connection: ConnectionType,
    pub ip: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub world_name: String,
}

pub fn get_server_creds(server_name: &str) -> Option<ServerCredentials> {
    if server_name == get_env("TEST_SERVER_NAME") {
        return Some(ServerCredentials {
            connection: ConnectionType::FTP,
            ip: get_env("TEST_SERVER_IP"),
            port: get_env("TEST_SERVER_PORT").parse().unwrap(),
            user: get_env("TEST_SERVER_USER"),
            password: get_env("TEST_SERVER_PASSWORD"),
            world_name: get_env("TEST_SERVER_WORLD_NAME"),
        });
    }

    None
}

struct UsersList {
    users: Vec<twilight_model::id::Id<discord::UserMarker>>,
    roles: Vec<twilight_model::id::Id<discord::RoleMarker>>,
}

struct Server {
    credentials: ServerCredentials,
    uploaders: UsersList,
}

struct BotData {
    servers: std::collections::HashMap<String, Server>,
}
