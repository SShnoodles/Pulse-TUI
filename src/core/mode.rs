#[derive(Debug, Default, PartialEq)]
pub enum AppMode {
    #[default]
    SourceSelect,
    Connect,
    Connecting,
    ModbusConnect,
    ModbusConnecting,
    SerialConnect,
    SerialConnecting,
    Monitor,
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
            MqttVersion::V5 => "MQTT 5.0",
        }
    }
    pub fn toggle(&mut self) {
        *self = match self {
            MqttVersion::V311 => MqttVersion::V5,
            MqttVersion::V5 => MqttVersion::V311,
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
            values: [
                host.to_string(),
                port.to_string(),
                String::new(),
                String::new(),
            ],
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
        if self.active < 4 {
            self.values[self.active].push(c);
        }
    }

    pub fn backspace(&mut self) {
        if self.active < 4 {
            self.values[self.active].pop();
        }
    }

    pub fn paste(&mut self, s: &str) {
        if self.active < 4 {
            self.values[self.active].push_str(s);
        }
    }

    pub fn host(&self) -> &str {
        &self.values[0]
    }
    pub fn port(&self) -> u16 {
        self.values[1].parse().unwrap_or(1883)
    }
    pub fn username(&self) -> &str {
        &self.values[2]
    }
    pub fn password(&self) -> &str {
        &self.values[3]
    }
}

// ── SourceKind ───────────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum SourceKind {
    #[default]
    Mqtt,
    ModbusTcp,
    Serial,
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
            values: ["localhost".into(), "502".into(), "1".into(), "1000".into()],
            active: 0,
            status: ConnectStatus::Idle,
        }
    }

    pub fn next(&mut self) {
        self.active = (self.active + 1) % 4;
    }
    pub fn prev(&mut self) {
        self.active = self.active.checked_sub(1).unwrap_or(3);
    }
    pub fn push(&mut self, c: char) {
        self.values[self.active].push(c);
    }
    pub fn backspace(&mut self) {
        self.values[self.active].pop();
    }
    pub fn paste(&mut self, s: &str) {
        self.values[self.active].push_str(s);
    }

    pub fn host(&self) -> &str {
        &self.values[0]
    }
    pub fn port(&self) -> u16 {
        self.values[1].parse().unwrap_or(502)
    }
    pub fn unit_id(&self) -> u8 {
        self.values[2].parse().unwrap_or(1)
    }
    pub fn poll_ms(&self) -> u64 {
        self.values[3].parse().unwrap_or(1000)
    }
}

// ── SerialForm ────────────────────────────────────────────────────────────────

const BAUD_RATES: &[u32] = &[
    1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600,
];
const DATA_BITS_VALS: &[u8] = &[5, 6, 7, 8];
const PARITY_LABELS: &[&str] = &["None", "Odd", "Even"];
const STOP_BITS_VALS: &[u8] = &[1, 2];

/// State for the Serial port connection form.
#[derive(Debug)]
pub struct SerialForm {
    pub available_ports: Vec<String>,
    pub port_idx: usize,   // index into available_ports
    pub baud_idx: usize,   // index into BAUD_RATES
    pub data_idx: usize,   // index into DATA_BITS_VALS
    pub parity_idx: usize, // index into PARITY_LABELS
    pub stop_idx: usize,   // index into STOP_BITS_VALS
    /// 0=port, 1=baud, 2=data_bits, 3=parity, 4=stop_bits
    pub active: usize,
    pub status: ConnectStatus,
}

impl SerialForm {
    pub const FIELD_COUNT: usize = 5;

    pub fn new() -> Self {
        let mut form = Self {
            available_ports: Vec::new(),
            port_idx: 0,
            baud_idx: 7,   // 115200
            data_idx: 3,   // 8
            parity_idx: 0, // None
            stop_idx: 0,   // 1
            active: 0,
            status: ConnectStatus::Idle,
        };
        form.refresh_ports();
        form
    }

    /// Re-scan available serial ports.
    pub fn refresh_ports(&mut self) {
        self.available_ports = serialport::available_ports()
            .unwrap_or_default()
            .into_iter()
            .map(|p| p.port_name)
            .collect();
        self.port_idx = self
            .port_idx
            .min(self.available_ports.len().saturating_sub(1));
    }

    /// Try to select the port by name (used when restoring saved config).
    pub fn select_port(&mut self, name: &str) {
        if let Some(i) = self.available_ports.iter().position(|p| p == name) {
            self.port_idx = i;
        }
    }

    pub fn next(&mut self) {
        self.active = (self.active + 1) % Self::FIELD_COUNT;
    }
    pub fn prev(&mut self) {
        self.active = self.active.checked_sub(1).unwrap_or(Self::FIELD_COUNT - 1);
    }

    pub fn paste(&mut self, _s: &str) {}

    pub fn left(&mut self) {
        match self.active {
            0 => {
                if !self.available_ports.is_empty() {
                    self.port_idx = self
                        .port_idx
                        .checked_sub(1)
                        .unwrap_or(self.available_ports.len() - 1);
                }
            }
            1 => self.baud_idx = self.baud_idx.checked_sub(1).unwrap_or(BAUD_RATES.len() - 1),
            2 => {
                self.data_idx = self
                    .data_idx
                    .checked_sub(1)
                    .unwrap_or(DATA_BITS_VALS.len() - 1)
            }
            3 => {
                self.parity_idx = self
                    .parity_idx
                    .checked_sub(1)
                    .unwrap_or(PARITY_LABELS.len() - 1)
            }
            4 => {
                self.stop_idx = self
                    .stop_idx
                    .checked_sub(1)
                    .unwrap_or(STOP_BITS_VALS.len() - 1)
            }
            _ => {}
        }
    }
    pub fn right(&mut self) {
        match self.active {
            0 => {
                if !self.available_ports.is_empty() {
                    self.port_idx = (self.port_idx + 1) % self.available_ports.len();
                }
            }
            1 => self.baud_idx = (self.baud_idx + 1) % BAUD_RATES.len(),
            2 => self.data_idx = (self.data_idx + 1) % DATA_BITS_VALS.len(),
            3 => self.parity_idx = (self.parity_idx + 1) % PARITY_LABELS.len(),
            4 => self.stop_idx = (self.stop_idx + 1) % STOP_BITS_VALS.len(),
            _ => {}
        }
    }

    pub fn port(&self) -> &str {
        self.available_ports
            .get(self.port_idx)
            .map(String::as_str)
            .unwrap_or("")
    }
    pub fn baud_rate(&self) -> u32 {
        BAUD_RATES[self.baud_idx]
    }
    pub fn data_bits_val(&self) -> u8 {
        DATA_BITS_VALS[self.data_idx]
    }
    pub fn parity_label(&self) -> &'static str {
        PARITY_LABELS[self.parity_idx]
    }
    pub fn stop_bits_val(&self) -> u8 {
        STOP_BITS_VALS[self.stop_idx]
    }

    pub fn baud_label(&self) -> String {
        self.baud_rate().to_string()
    }
    pub fn data_label(&self) -> String {
        self.data_bits_val().to_string()
    }
    pub fn stop_label(&self) -> String {
        self.stop_bits_val().to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FunctionCode {
    FC01Coil,
    FC02Discrete,
    FC03Holding,
    #[default]
    FC04Input,
}

impl FunctionCode {
    pub const ALL: &'static [Self] = &[
        Self::FC01Coil,
        Self::FC02Discrete,
        Self::FC03Holding,
        Self::FC04Input,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::FC01Coil => "FC01 Coil",
            Self::FC02Discrete => "FC02 Discrete",
            Self::FC03Holding => "FC03 Holding",
            Self::FC04Input => "FC04 Input",
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
        Self::Unsigned,
        Self::Signed,
        Self::Hex,
        Self::Binary,
        Self::Long,
        Self::LongInverse,
        Self::Float,
        Self::FloatInverse,
        Self::Double,
        Self::DoubleInverse,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Unsigned => "Unsigned",
            Self::Signed => "Signed",
            Self::Hex => "Hex",
            Self::Binary => "Binary",
            Self::Long => "Long (AB CD)",
            Self::LongInverse => "Long Inv (CD AB)",
            Self::Float => "Float (AB CD)",
            Self::FloatInverse => "Float Inv (CD AB)",
            Self::Double => "Double (AB..GH)",
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
            fc_idx: 3, // FC04 Input
            start_input: "0".into(),
            qty_input: "10".into(),
            format_idx: 0, // Unsigned
            active: 0,
            editing: false,
        }
    }
}

impl ModbusQueryForm {
    pub fn fc(&self) -> FunctionCode {
        FunctionCode::ALL[self.fc_idx.min(FunctionCode::ALL.len() - 1)]
    }
    pub fn start(&self) -> u16 {
        self.start_input.parse().unwrap_or(0)
    }
    pub fn qty(&self) -> u16 {
        self.qty_input.parse().unwrap_or(1).max(1)
    }
    pub fn format(&self) -> DisplayFormat {
        DisplayFormat::ALL[self.format_idx.min(DisplayFormat::ALL.len() - 1)]
    }

    pub fn next_field(&mut self) {
        self.active = (self.active + 1) % 4;
    }
    pub fn prev_field(&mut self) {
        self.active = self.active.checked_sub(1).unwrap_or(3);
    }

    pub fn left(&mut self) {
        match self.active {
            0 => {
                self.fc_idx = self
                    .fc_idx
                    .checked_sub(1)
                    .unwrap_or(FunctionCode::ALL.len() - 1)
            }
            3 => {
                self.format_idx = self
                    .format_idx
                    .checked_sub(1)
                    .unwrap_or(DisplayFormat::ALL.len() - 1)
            }
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
            1 => {
                self.start_input.pop();
            }
            2 => {
                self.qty_input.pop();
            }
            _ => {}
        }
    }
}
