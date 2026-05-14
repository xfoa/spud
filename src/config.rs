use std::collections::HashMap;
use std::path::PathBuf;

use argon2::{Argon2, PasswordHasher};
use argon2::password_hash::{PasswordHash, SaltString, rand_core::OsRng};
use serde::{Deserialize, Serialize};

use crate::icons;

const APP_DIR: &str = "spud";
const CONFIG_FILE: &str = "config.toml";
const KNOWN_SERVERS_FILE: &str = "known_servers.toml";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Client,
    Server,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerIcon {
    #[default]
    Desktop,
    Laptop,
    Server,
}

impl ServerIcon {
    pub const ALL: [ServerIcon; 3] =
        [ServerIcon::Desktop, ServerIcon::Laptop, ServerIcon::Server];

    pub fn glyph(self) -> char {
        match self {
            ServerIcon::Desktop => icons::DESKTOP,
            ServerIcon::Laptop => icons::LAPTOP,
            ServerIcon::Server => icons::SERVER,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureMode {
    Fullscreen,
    Window,
}

impl Default for CaptureMode {
    fn default() -> Self {
        Self::Fullscreen
    }
}

impl std::fmt::Display for CaptureMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            CaptureMode::Fullscreen => "Fullscreen (relative mouse)",
            CaptureMode::Window => "Window only (absolute mouse)",
        };
        f.write_str(s)
    }
}

pub fn hash_passphrase(passphrase: &str) -> String {
    let argon2 = Argon2::default();
    let salt = SaltString::generate(&mut OsRng);
    argon2.hash_password(passphrase.as_bytes(), &salt)
        .map(|h| h.to_string())
        .unwrap_or_default()
}

pub fn extract_salt(hash: &str) -> Option<String> {
    PasswordHash::new(hash).ok()?.salt.map(|s| s.as_str().to_string())
}

pub fn hash_passphrase_with_salt(passphrase: &str, salt: &str) -> Option<String> {
    let argon2 = Argon2::default();
    let salt = SaltString::from_b64(salt).ok()?;
    argon2.hash_password(passphrase.as_bytes(), &salt)
        .map(|h| h.to_string())
        .ok()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub name: String,
    pub icon: ServerIcon,
    pub bind_address: String,
    pub port: String,
    pub discoverable: bool,
    pub require_auth: bool,
    pub passphrase_hash: String,
    pub key_timeout_ms: u16,
    pub encrypt_udp: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            name: default_hostname(),
            icon: ServerIcon::Desktop,
            bind_address: "0.0.0.0".to_string(),
            port: "7878".to_string(),
            discoverable: true,
            require_auth: true,
            passphrase_hash: String::new(),
            key_timeout_ms: 1000,
            encrypt_udp: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientConfig {
    pub host: String,
    pub port: String,
    pub sensitivity: String,
    pub natural_scroll: bool,
    pub capture_mode: CaptureMode,
    pub hotkey: String,
    pub require_auth: bool,
    pub passphrase_hash: String,
    pub keepalive_interval_ms: u16,
    pub reconnect_timeout_secs: u16,
    pub blank_screen: bool,
    pub show_hotkey_on_blank: bool,
    pub encrypt_udp: bool,
    pub mouse_batch_size: u8,
    pub batch_redundancy: u8,
    pub udp_drop_percent: u8,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: "7878".to_string(),
            sensitivity: "1.00".to_string(),
            natural_scroll: false,
            capture_mode: CaptureMode::Fullscreen,
            hotkey: "Ctrl+Alt+Space".to_string(),
            require_auth: true,
            passphrase_hash: String::new(),
            keepalive_interval_ms: 100,
            reconnect_timeout_secs: 30,
            blank_screen: false,
            show_hotkey_on_blank: true,
            encrypt_udp: true,
            mouse_batch_size: 8,
            batch_redundancy: 0,
            udp_drop_percent: 0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub mode: Mode,
    pub client: ClientConfig,
    pub server: ServerConfig,
}

fn default_hostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "spud-server".to_string())
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR)
        .join(CONFIG_FILE)
}

pub fn known_servers_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR)
        .join(KNOWN_SERVERS_FILE)
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(cfg) => cfg,
                Err(e) => {
                    eprintln!("Failed to parse config at {}: {e}", path.display());
                    Self::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => {
                eprintln!("Failed to read config at {}: {e}", path.display());
                Self::default()
            }
        }
    }

    pub fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!("Failed to create config directory {}: {e}", parent.display());
                return;
            }
        }
        match toml::to_string_pretty(self) {
            Ok(contents) => {
                if let Err(e) = std::fs::write(&path, contents) {
                    eprintln!("Failed to write config to {}: {e}", path.display());
                }
            }
            Err(e) => eprintln!("Failed to serialize config: {e}"),
        }
    }
}

pub fn load_known_servers() -> HashMap<String, String> {
    let path = known_servers_path();
    match std::fs::read_to_string(&path) {
        Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
        Err(e) => {
            eprintln!("Failed to read known servers at {}: {e}", path.display());
            HashMap::new()
        }
    }
}

pub fn save_known_servers(known: &HashMap<String, String>) {
    let path = known_servers_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("Failed to create config directory {}: {e}", parent.display());
            return;
        }
    }
    match toml::to_string_pretty(known) {
        Ok(contents) => {
            if let Err(e) = std::fs::write(&path, contents) {
                eprintln!("Failed to write known servers to {}: {e}", path.display());
            }
        }
        Err(e) => eprintln!("Failed to serialize known servers: {e}"),
    }
}

pub fn trust_server(host: &str, port: u16, fingerprint: [u8; 32]) {
    let mut known = load_known_servers();
    let key = format!("{host}:{port}");
    known.insert(key, hex::encode(fingerprint));
    save_known_servers(&known);
}
