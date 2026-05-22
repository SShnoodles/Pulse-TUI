use crate::core::MqttVersion;

pub struct MqttConfig {
    pub host: String,
    pub port: u16,
    pub client_id: String,
    pub topics: Vec<String>,
    pub keep_alive_secs: u64,
    pub username: Option<String>,
    pub password: Option<String>,
    pub version: MqttVersion,
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            host: "localhost".into(),
            port: 1883,
            client_id: "pulse-tui".into(),
            topics: vec!["#".into()],
            keep_alive_secs: 5,
            username: None,
            password: None,
            version: MqttVersion::V311,
        }
    }
}
