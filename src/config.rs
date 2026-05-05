use std::path::PathBuf;

use argon2::{Argon2, PasswordHasher};
use argon2::password_hash::{PasswordHash, SaltString, rand_core::OsRng};
use serde::{Deserialize, Serialize};

use crate::icons;

const APP_DIR: &str = "spud";
const CONFIG_FILE: &str = "config.toml";

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
    Hotkey,
    Focus,
}

impl Default for CaptureMode {
    fn default() -> Self {
        Self::Hotkey
    }
}

impl std::fmt::Display for CaptureMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            CaptureMode::Hotkey => "Toggled by hotkey",
            CaptureMode::Focus => "When window has focus",
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
    pub encrypt: bool,
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
            encrypt: true,
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
    pub encrypt: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: "7878".to_string(),
            sensitivity: "1.00".to_string(),
            natural_scroll: false,
            capture_mode: CaptureMode::Hotkey,
            hotkey: "Ctrl+Alt+Space".to_string(),
            require_auth: true,
            passphrase_hash: String::new(),
            keepalive_interval_ms: 50,
            reconnect_timeout_secs: 30,
            blank_screen: false,
            show_hotkey_on_blank: true,
            encrypt: true,
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
