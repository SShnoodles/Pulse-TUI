#[derive(Debug, Default, PartialEq)]
pub enum AppMode {
    #[default]
    Connect,    // credential form (idle or showing error)
    Connecting, // waiting for ConnAck
    Monitor,    // main TUI
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum MqttVersion {
    #[default]
    V311,
    V5,
}

impl MqttVersion {
    pub fn label(self) -> &'static str {
        match self {
            MqttVersion::V311 => "MQTT 3.1.1",
            MqttVersion::V5   => "MQTT 5.0",
        }
    }
    pub fn toggle(&mut self) {
        *self = match self {
            MqttVersion::V311 => MqttVersion::V5,
            MqttVersion::V5   => MqttVersion::V311,
        };
    }
}

#[derive(Debug, Default, PartialEq)]
pub enum ConnectStatus {
    #[default]
    Idle,
    Connecting,
    Error(String),
}

/// State for the connection credential form.
#[derive(Debug)]
pub struct ConnectForm {
    pub values: [String; 4], // [host, port, username, password]
    pub active: usize,       // 0-3 = text fields, 4 = version selector
    pub status: ConnectStatus,
    pub mqtt_version: MqttVersion,
}

impl ConnectForm {
    pub const LABELS: [&'static str; 4] = ["Broker", "Port", "Username", "Password"];

    pub fn new(host: &str, port: u16) -> Self {
        Self {
            values: [host.to_string(), port.to_string(), String::new(), String::new()],
            active: 0,
            status: ConnectStatus::Idle,
            mqtt_version: MqttVersion::V311,
        }
    }

    pub fn next(&mut self) {
        self.active = (self.active + 1) % 5;
    }

    pub fn prev(&mut self) {
        self.active = self.active.checked_sub(1).unwrap_or(4);
    }

    pub fn push(&mut self, c: char) {
        if self.active < 4 { self.values[self.active].push(c); }
    }

    pub fn backspace(&mut self) {
        if self.active < 4 { self.values[self.active].pop(); }
    }

    pub fn paste(&mut self, s: &str) {
        if self.active < 4 { self.values[self.active].push_str(s); }
    }

    pub fn host(&self) -> &str { &self.values[0] }
    pub fn port(&self) -> u16  { self.values[1].parse().unwrap_or(1883) }
    pub fn username(&self) -> &str { &self.values[2] }
    pub fn password(&self) -> &str { &self.values[3] }
}
