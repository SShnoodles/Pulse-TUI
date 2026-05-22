use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::core::MqttVersion;

// ── Per-source config sections ──────────────────────────────────────────────

/// Persisted settings for the MQTT source  →  [mqtt] in ~/.pulse-tui.toml
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct MqttConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    /// "v311" | "v5"
    pub version: String,
    pub topics: Vec<String>,
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            host: "localhost".into(),
            port: 1883,
            username: String::new(),
            version: "v311".into(),
            topics: Vec::new(),
        }
    }
}

// Future sources: ModbusConfig, WebSocketConfig, SerialConfig …

// ── Top-level config ─────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SavedConfig {
    pub mqtt: MqttConfig,
    // pub modbus: ModbusConfig,
    // pub serial: SerialConfig,
}

impl SavedConfig {
    pub fn mqtt_version(&self) -> MqttVersion {
        if self.mqtt.version == "v5" {
            MqttVersion::V5
        } else {
            MqttVersion::V311
        }
    }
}

// ── I/O ──────────────────────────────────────────────────────────────────────

fn config_path() -> Option<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    Some(PathBuf::from(home).join(".pulse-tui.toml"))
}

pub fn load() -> SavedConfig {
    let Some(path) = config_path() else {
        return SavedConfig::default();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return SavedConfig::default();
    };
    toml::from_str(&text).unwrap_or_default()
}

pub fn save(cfg: &SavedConfig) {
    let Some(path) = config_path() else { return };
    if let Ok(text) = toml::to_string(cfg) {
        let _ = std::fs::write(path, text);
    }
}

/// Load, update only the MQTT topics, and save back.
/// Used at runtime so subscribe/unsubscribe never overwrites broker credentials.
pub fn update_topics(topics: &[String]) {
    let mut cfg = load();
    cfg.mqtt.topics = topics.to_vec();
    save(&cfg);
}
