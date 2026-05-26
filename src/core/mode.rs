#[derive(Debug, Default, PartialEq)]
pub enum AppMode {
    #[default]
    SourceSelect,   // initial protocol picker
    Connect,        // MQTT credential form
    Connecting,     // MQTT: waiting for ConnAck
    ModbusConnect,  // Modbus TCP form
    ModbusConnecting, // Modbus TCP: waiting for TCP connect
    Monitor,        // main TUI (shared by all sources)
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

// ── SourceKind ───────────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum SourceKind {
    #[default]
    Mqtt,
    ModbusTcp,
}

// ── ModbusForm ───────────────────────────────────────────────────────────────

/// State for the Modbus TCP connection form.
#[derive(Debug)]
pub struct ModbusForm {
    pub values: [String; 4], // [host, port, unit_id, poll_ms]
    pub active: usize,
    pub status: ConnectStatus,
}

impl ModbusForm {
    pub const LABELS: [&'static str; 4] = ["Host", "Port", "Unit ID", "Poll ms"];

    pub fn new() -> Self {
        Self {
            values: [
                "localhost".into(),
                "502".into(),
                "1".into(),
                "1000".into(),
            ],
            active: 0,
            status: ConnectStatus::Idle,
        }
    }

    pub fn next(&mut self) { self.active = (self.active + 1) % 4; }
    pub fn prev(&mut self) { self.active = self.active.checked_sub(1).unwrap_or(3); }
    pub fn push(&mut self, c: char) { self.values[self.active].push(c); }
    pub fn backspace(&mut self) { self.values[self.active].pop(); }
    pub fn paste(&mut self, s: &str) { self.values[self.active].push_str(s); }

    pub fn host(&self) -> &str { &self.values[0] }
    pub fn port(&self) -> u16  { self.values[1].parse().unwrap_or(502) }
    pub fn unit_id(&self) -> u8  { self.values[2].parse().unwrap_or(1) }
    pub fn poll_ms(&self) -> u64 { self.values[3].parse().unwrap_or(1000) }
}

// ── FunctionCode ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FunctionCode {
    FC01Coil,
    FC02Discrete,
    #[default]
    FC03Holding,
    FC04Input,
}

impl FunctionCode {
    pub const ALL: &'static [Self] =
        &[Self::FC01Coil, Self::FC02Discrete, Self::FC03Holding, Self::FC04Input];

    pub fn label(self) -> &'static str {
        match self {
            Self::FC01Coil     => "FC01 Coil",
            Self::FC02Discrete => "FC02 Discrete",
            Self::FC03Holding  => "FC03 Holding",
            Self::FC04Input    => "FC04 Input",
        }
    }

    pub fn is_bit(self) -> bool {
        matches!(self, Self::FC01Coil | Self::FC02Discrete)
    }
}

// ── DisplayFormat ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum DisplayFormat {
    #[default]
    Unsigned,
    Signed,
    Hex,
    Binary,
    Long,
    LongInverse,
    Float,
    FloatInverse,
    Double,
    DoubleInverse,
}

impl DisplayFormat {
    pub const ALL: &'static [Self] = &[
        Self::Unsigned, Self::Signed, Self::Hex, Self::Binary,
        Self::Long, Self::LongInverse, Self::Float, Self::FloatInverse,
        Self::Double, Self::DoubleInverse,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Unsigned      => "Unsigned",
            Self::Signed        => "Signed",
            Self::Hex           => "Hex",
            Self::Binary        => "Binary",
            Self::Long          => "Long (AB CD)",
            Self::LongInverse   => "Long Inv (CD AB)",
            Self::Float         => "Float (AB CD)",
            Self::FloatInverse  => "Float Inv (CD AB)",
            Self::Double        => "Double (AB..GH)",
            Self::DoubleInverse => "Double Inv (GH..AB)",
        }
    }

    /// Number of consecutive registers needed to represent one value.
    pub fn register_count(self) -> usize {
        match self {
            Self::Long | Self::LongInverse | Self::Float | Self::FloatInverse => 2,
            Self::Double | Self::DoubleInverse => 4,
            _ => 1,
        }
    }
}

// ── ModbusQueryForm ───────────────────────────────────────────────────────────

/// Live query-settings form shown in the Modbus monitor.
#[derive(Debug)]
pub struct ModbusQueryForm {
    pub fc_idx: usize,       // index into FunctionCode::ALL
    pub start_input: String, // text input for start address
    pub qty_input: String,   // text input for quantity
    pub format_idx: usize,   // index into DisplayFormat::ALL
    /// Which field is active: 0=FC, 1=Start, 2=Qty, 3=Format
    pub active: usize,
    /// Whether the form is in edit mode
    pub editing: bool,
}

impl Default for ModbusQueryForm {
    fn default() -> Self {
        Self {
            fc_idx: 2,               // FC03 Holding
            start_input: "0".into(),
            qty_input: "10".into(),
            format_idx: 0,           // Unsigned
            active: 0,
            editing: false,
        }
    }
}

impl ModbusQueryForm {
    pub fn fc(&self) -> FunctionCode {
        FunctionCode::ALL[self.fc_idx.min(FunctionCode::ALL.len() - 1)]
    }
    pub fn start(&self) -> u16 { self.start_input.parse().unwrap_or(0) }
    pub fn qty(&self) -> u16   { self.qty_input.parse().unwrap_or(1).max(1) }
    pub fn format(&self) -> DisplayFormat {
        DisplayFormat::ALL[self.format_idx.min(DisplayFormat::ALL.len() - 1)]
    }

    pub fn next_field(&mut self) { self.active = (self.active + 1) % 4; }
    pub fn prev_field(&mut self) { self.active = self.active.checked_sub(1).unwrap_or(3); }

    pub fn left(&mut self) {
        match self.active {
            0 => self.fc_idx = self.fc_idx.checked_sub(1).unwrap_or(FunctionCode::ALL.len() - 1),
            3 => self.format_idx = self.format_idx.checked_sub(1).unwrap_or(DisplayFormat::ALL.len() - 1),
            _ => {}
        }
    }
    pub fn right(&mut self) {
        match self.active {
            0 => self.fc_idx = (self.fc_idx + 1) % FunctionCode::ALL.len(),
            3 => self.format_idx = (self.format_idx + 1) % DisplayFormat::ALL.len(),
            _ => {}
        }
    }

    pub fn push(&mut self, c: char) {
        if c.is_ascii_digit() {
            match self.active {
                1 => self.start_input.push(c),
                2 => self.qty_input.push(c),
                _ => {}
            }
        }
    }
    pub fn backspace(&mut self) {
        match self.active {
            1 => { self.start_input.pop(); }
            2 => { self.qty_input.pop(); }
            _ => {}
        }
    }
}
