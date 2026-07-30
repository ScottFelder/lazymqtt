use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub topic: String,
    #[serde(default)]
    pub qos: u8,
}

impl Subscription {
    pub fn new(topic: impl Into<String>) -> Self {
        Self {
            topic: topic.into(),
            qos: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    #[serde(default = "new_id")]
    pub id: String,
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub tls: bool,
    #[serde(default = "default_tls_verify")]
    pub tls_verify: bool,
    #[serde(default)]
    pub subscriptions: Vec<Subscription>,
}

fn new_id() -> String {
    Uuid::new_v4().to_string()
}
fn default_port() -> u16 {
    1883
}
fn default_tls_verify() -> bool {
    true
}

impl Connection {
    pub fn new(name: impl Into<String>, host: impl Into<String>, port: u16) -> Self {
        Self {
            id: new_id(),
            name: name.into(),
            host: host.into(),
            port,
            client_id: format!("lazymqtt-{}", &new_id()[..8]),
            username: String::new(),
            password: String::new(),
            tls: false,
            tls_verify: true,
            subscriptions: vec![Subscription::new("#")],
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub connections: Vec<Connection>,
}

fn config_path() -> Result<PathBuf> {
    let dir = crate::paths::config_dir();
    fs::create_dir_all(&dir).ok();
    Ok(dir.join("connections.json"))
}

impl Config {
    pub fn load() -> Self {
        match config_path().and_then(|p| Ok(fs::read_to_string(p)?)) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Config::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }
}
