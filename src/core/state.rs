use chrono::{DateTime, Local};

use super::mode::{DisplayFormat, FunctionCode, ModbusQueryForm, SourceKind};

const TPS_HISTORY_LEN: usize = 60;
const MODBUS_TREND_HISTORY_LEN: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SerialDisplayFormat {
    #[default]
    Ascii,
    Hex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialDirection {
    Rx,
    Tx,
}

#[derive(Debug, Clone)]
pub struct SerialEntry {
    pub timestamp: String, // "hh:mm:ss"
    pub direction: SerialDirection,
    pub raw: Vec<u8>, // raw bytes (never converted through UTF-8)
}

impl SerialEntry {
    fn now(direction: SerialDirection, raw: Vec<u8>) -> Self {
        let ts = chrono::Local::now().format("%H:%M:%S").to_string();
        Self {
            timestamp: ts,
            direction,
            raw,
        }
    }

    pub fn rx(text: String) -> Self {
        Self::now(SerialDirection::Rx, text.into_bytes())
    }

    pub fn tx(bytes: Vec<u8>) -> Self {
        Self::now(SerialDirection::Tx, bytes)
    }

    /// Format for display. In Hex mode the payload bytes are hex-encoded.
    pub fn render(&self, fmt: SerialDisplayFormat) -> String {
        let payload = match fmt {
            SerialDisplayFormat::Ascii => String::from_utf8_lossy(&self.raw).into_owned(),
            SerialDisplayFormat::Hex => self
                .raw
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(" "),
        };
        match self.direction {
            SerialDirection::Rx => format!("{} RX <- {}", self.timestamp, payload),
            SerialDirection::Tx => format!("{} TX -> {}", self.timestamp, payload),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModbusRow {
    pub address: u16,
    pub value: u16,
}

#[derive(Debug, Clone)]
pub struct ModbusTrendPoint {
    pub address: u16,
    pub values: Vec<ModbusTrendSample>,
}

#[derive(Debug, Clone)]
pub struct ModbusTrendSample {
    pub time_mmss: String,
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct OpcUaRow {
    pub node_id: String,
    pub display_name: String,
    pub value: String,
    pub data_type: String,
    pub source_timestamp: String,
    pub server_timestamp: String,
}

impl ModbusTrendPoint {
    pub fn new(address: u16) -> Self {
        Self {
            address,
            values: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TopicItem {
    pub name: String,
    pub msg_count: u64,
    pub tps: u64,         // messages/sec (updated each second)
    pub tps_counter: u64, // accumulator for current second
    pub tps_history: Vec<u64>,
}

impl TopicItem {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            msg_count: 0,
            tps: 0,
            tps_counter: 0,
            tps_history: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    pub topic: String,
    pub payload: String,
    pub timestamp: String,
    pub qos: u8,
    pub retained: bool,
}

#[derive(Debug, Clone)]
pub struct MqttMessage {
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: u8,
    pub retained: bool,
}

#[derive(Debug)]
pub struct AppState {
    pub connected: bool,
    pub paused: bool,
    pub search_mode: bool,
    pub search_query: String,
    pub subscribe_mode: bool,
    pub subscribe_input: String,
    pub publish_mode: bool,
    pub publish_input: String,
    pub broker: String,
    /// None = show all topics; Some(i) = filter by topics[i]
    pub selected_topic_idx: Option<usize>,
    pub topics: Vec<TopicItem>,
    pub messages: Vec<Message>,
    /// None = auto-follow newest; Some(i) = cursor at index i in filtered list
    pub msg_cursor: Option<usize>,
    /// Yank (copy selection) mode — only active when paused
    pub yank_mode: bool,
    /// Selection anchor (byte offset in current msg payload)
    pub yank_start: usize,
    /// Selection cursor (byte offset; selection = [min..max])
    pub yank_cursor: usize,
    /// Show "back to connect?" confirmation dialog
    pub confirm_back: bool,
    /// Last error message for display in the error bar
    pub last_error: Option<String>,
    /// Protocol version label shown in status bar
    pub mqtt_version: &'static str,
    /// 100 ms tick accumulator; TPS is flushed every 10 ticks
    tick_count: u8,
    /// Topics we have subscribed to (for save/restore and delete)
    pub subscribed_topics: Vec<String>,
    /// Auto-select first topic when it appears (set when connecting with saved topics)
    pub auto_select_first: bool,
    /// Which protocol source is active
    pub source_kind: SourceKind,
    /// Polled register rows from Modbus source
    pub modbus_rows: Vec<ModbusRow>,
    /// Query settings form for Modbus monitor
    pub modbus_query: ModbusQueryForm,
    /// Scroll offset for the Modbus data table
    pub modbus_table_offset: usize,
    /// Tracked Modbus points rendered in the trend chart
    pub modbus_trend_points: Vec<ModbusTrendPoint>,
    /// Selected tracked point index
    pub modbus_trend_selected: usize,
    /// Add-point input mode for the trend chart
    pub modbus_add_point_mode: bool,
    /// Address input buffer for add-point mode
    pub modbus_add_point_input: String,
    /// Lines received from / sent to the serial port (capped at 2000)
    pub serial_lines: Vec<SerialEntry>,
    /// Scroll offset for the serial monitor view
    pub serial_line_offset: usize,
    /// Write (send) mode active in serial monitor
    pub serial_write_mode: bool,
    /// Input buffer for serial write mode
    pub serial_write_input: String,
    /// Display format for received serial data
    pub serial_display_format: SerialDisplayFormat,
    /// Pause auto-scroll in serial monitor (new lines still buffered)
    pub serial_paused: bool,
    /// Latest values for monitored OPC UA nodes.
    pub opcua_rows: Vec<OpcUaRow>,
    /// Node IDs being monitored by OPC UA source.
    pub opcua_node_ids: Vec<String>,
    /// Scroll offset for OPC UA monitor
    pub opcua_offset: usize,
    /// Add-node input mode for OPC UA monitor.
    pub opcua_add_node_mode: bool,
    /// Input buffer for add-node mode.
    pub opcua_add_node_input: String,
    /// Delete-node input mode for OPC UA monitor.
    pub opcua_delete_node_mode: bool,
    /// Input buffer for delete-node mode.
    pub opcua_delete_node_input: String,
    /// Local timestamp of the last successful OPC UA poll.
    pub opcua_last_refresh: Option<DateTime<Local>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            connected: false,
            paused: false,
            search_mode: false,
            search_query: String::new(),
            subscribe_mode: false,
            subscribe_input: String::new(),
            publish_mode: false,
            publish_input: String::new(),
            broker: String::new(),
            selected_topic_idx: None,
            topics: Vec::new(),
            messages: Vec::new(),
            msg_cursor: None,
            yank_mode: false,
            yank_start: 0,
            yank_cursor: 0,
            confirm_back: false,
            last_error: None,
            mqtt_version: "MQTT 3.1.1",
            tick_count: 0,
            subscribed_topics: Vec::new(),
            auto_select_first: false,
            source_kind: SourceKind::Mqtt,
            modbus_rows: Vec::new(),
            modbus_query: ModbusQueryForm::default(),
            modbus_table_offset: 0,
            modbus_trend_points: Vec::new(),
            modbus_trend_selected: 0,
            modbus_add_point_mode: false,
            modbus_add_point_input: String::new(),
            serial_lines: Vec::<SerialEntry>::new(),
            serial_line_offset: 0,
            serial_write_mode: false,
            serial_write_input: String::new(),
            serial_display_format: SerialDisplayFormat::Ascii,
            serial_paused: false,
            opcua_rows: Vec::new(),
            opcua_node_ids: Vec::new(),
            opcua_offset: 0,
            opcua_add_node_mode: false,
            opcua_add_node_input: String::new(),
            opcua_delete_node_mode: false,
            opcua_delete_node_input: String::new(),
            opcua_last_refresh: None,
        }
    }
}

impl AppState {
    pub fn enter_search(&mut self) {
        self.search_mode = true;
    }

    pub fn exit_search(&mut self, clear: bool) {
        self.search_mode = false;
        if clear {
            self.search_query.clear();
        }
    }

    pub fn push_search(&mut self, c: char) {
        self.search_query.push(c);
    }

    pub fn backspace_search(&mut self) {
        self.search_query.pop();
    }

    // ── Topic navigation ────────────────────────────────────────────────────

    pub fn select_topic_next(&mut self) {
        if self.topics.is_empty() {
            return;
        }
        self.selected_topic_idx = Some(match self.selected_topic_idx {
            None => 0,
            Some(i) => (i + 1).min(self.topics.len() - 1),
        });
        self.msg_cursor = None;
    }

    pub fn select_topic_prev(&mut self) {
        self.selected_topic_idx = match self.selected_topic_idx {
            None | Some(0) => None,
            Some(i) => Some(i - 1),
        };
        self.msg_cursor = None;
    }

    pub fn clear_topic_filter(&mut self) {
        self.selected_topic_idx = None;
        self.msg_cursor = None;
    }

    pub fn selected_topic_name(&self) -> Option<&str> {
        self.selected_topic_idx
            .and_then(|i| self.topics.get(i))
            .map(|t| t.name.as_str())
    }

    // ── Message navigation ───────────────────────────────────────────────────

    pub fn select_msg_prev(&mut self) {
        let count = self.filtered_messages().count();
        if count == 0 {
            return;
        }
        self.msg_cursor = Some(match self.msg_cursor {
            None => count.saturating_sub(1),
            Some(i) => i.saturating_sub(1),
        });
    }

    pub fn select_msg_next(&mut self) {
        let count = self.filtered_messages().count();
        if count == 0 {
            return;
        }
        match self.msg_cursor {
            None => {}
            Some(i) => {
                if i + 1 >= count {
                    self.msg_cursor = None; // reached newest → auto-follow
                } else {
                    self.msg_cursor = Some(i + 1);
                }
            }
        }
    }

    pub fn selected_message(&self) -> Option<&Message> {
        let idx = self.msg_cursor?;
        self.filtered_messages().nth(idx)
    }

    // ── Yank mode ────────────────────────────────────────────────────────────

    pub fn enter_yank_mode(&mut self) {
        if self.msg_cursor.is_none() {
            let count = self.filtered_messages().count();
            if count > 0 {
                self.msg_cursor = Some(count.saturating_sub(1));
            } else {
                return;
            }
        }
        self.yank_mode = true;
        self.yank_start = 0;
        self.yank_cursor = 0;
    }

    pub fn exit_yank_mode(&mut self) {
        self.yank_mode = false;
        self.yank_start = 0;
        self.yank_cursor = 0;
    }

    pub fn yank_left(&mut self) {
        if self.yank_cursor == 0 {
            return;
        }
        if let Some(msg) = self.selected_message() {
            if let Some((i, _)) = msg.payload[..self.yank_cursor].char_indices().next_back() {
                self.yank_cursor = i;
            }
        }
    }

    pub fn yank_right(&mut self) {
        if let Some(msg) = self.selected_message() {
            if self.yank_cursor >= msg.payload.len() {
                return;
            }
            let c = msg.payload[self.yank_cursor..].chars().next().unwrap();
            self.yank_cursor += c.len_utf8();
        }
    }

    pub fn yank_text(&self) -> Option<String> {
        let msg = self.selected_message()?;
        let start = self.yank_start.min(self.yank_cursor).min(msg.payload.len());
        let end = self.yank_start.max(self.yank_cursor).min(msg.payload.len());
        if start == end {
            Some(msg.payload.clone())
        } else {
            Some(msg.payload[start..end].to_string())
        }
    }

    // ── Message filtering ────────────────────────────────────────────────────

    pub fn filtered_messages(&self) -> impl Iterator<Item = &Message> {
        let q = self.search_query.to_lowercase();
        let topic = self.selected_topic_name().unwrap_or("").to_string();
        self.messages.iter().filter(move |m| {
            if !topic.is_empty() && m.topic != topic {
                return false;
            }
            if !q.is_empty()
                && !m.topic.to_lowercase().contains(&q)
                && !m.payload.to_lowercase().contains(&q)
            {
                return false;
            }
            true
        })
    }

    pub fn on_tick(&mut self) {
        self.tick_count += 1;
        if self.tick_count >= 10 {
            self.tick_count = 0;
            for t in &mut self.topics {
                t.tps = t.tps_counter;
                t.tps_counter = 0;
                t.tps_history.push(t.tps);
                if t.tps_history.len() > TPS_HISTORY_LEN {
                    t.tps_history.remove(0);
                }
            }
        }
    }

    pub fn add_message(&mut self, msg: MqttMessage) {
        if self.paused {
            return;
        }

        // Drop messages for non-selected topics when a topic is focused
        if let Some(idx) = self.selected_topic_idx {
            if let Some(selected) = self.topics.get(idx) {
                if selected.name != msg.topic {
                    return;
                }
            }
        }

        let payload = String::from_utf8_lossy(&msg.payload).into_owned();

        if let Some(item) = self.topics.iter_mut().find(|t| t.name == msg.topic) {
            item.msg_count += 1;
            item.tps_counter += 1;
        } else {
            let mut item = TopicItem::new(&msg.topic);
            item.msg_count = 1;
            item.tps_counter = 1;
            self.topics.push(item);
            // Auto-select first topic if requested
            if self.auto_select_first && self.selected_topic_idx.is_none() {
                self.selected_topic_idx = Some(0);
                self.auto_select_first = false;
            }
        }

        self.messages.push(Message {
            topic: msg.topic,
            payload,
            timestamp: now_hms(),
            qos: msg.qos,
            retained: msg.retained,
        });
    }

    pub fn add_modbus_trend_point(&mut self, address: u16) {
        if let Some(i) = self
            .modbus_trend_points
            .iter()
            .position(|p| p.address == address)
        {
            self.modbus_trend_selected = i;
            return;
        }

        self.modbus_trend_points
            .push(ModbusTrendPoint::new(address));
        self.modbus_trend_selected = self.modbus_trend_points.len().saturating_sub(1);
    }

    pub fn selected_modbus_trend_point(&self) -> Option<&ModbusTrendPoint> {
        self.modbus_trend_points.get(self.modbus_trend_selected)
    }

    pub fn update_modbus_trend_points(&mut self) {
        let fmt = self.modbus_query.format();
        let fc = self.modbus_query.fc();

        for p in &mut self.modbus_trend_points {
            if let Some(v) = numeric_display_for_address(&self.modbus_rows, p.address, fmt, fc) {
                p.values.push(ModbusTrendSample {
                    time_mmss: chrono::Local::now().format("%M:%S").to_string(),
                    value: v,
                });
                if p.values.len() > MODBUS_TREND_HISTORY_LEN {
                    p.values.remove(0);
                }
            }
        }

        if self.modbus_trend_selected >= self.modbus_trend_points.len() {
            self.modbus_trend_selected = self.modbus_trend_points.len().saturating_sub(1);
        }
    }

    /// Delete the currently selected topic: remove from topics list, subscribed list, messages.
    /// Returns the topic name for the caller to send Unsubscribe.
    pub fn delete_selected_topic(&mut self) -> Option<String> {
        let idx = self.selected_topic_idx?;
        let topic = self.topics.get(idx)?.name.clone();
        self.topics.remove(idx);
        self.messages.retain(|m| m.topic != topic);
        self.subscribed_topics.retain(|t| t != &topic);
        // Adjust cursor
        self.selected_topic_idx = if self.topics.is_empty() {
            None
        } else {
            Some(idx.min(self.topics.len() - 1))
        };
        self.msg_cursor = None;
        Some(topic)
    }
}

fn now_hms() -> String {
    let now = chrono::Local::now();
    now.format("%H:%M:%S").to_string()
}

fn numeric_display_for_address(
    rows: &[ModbusRow],
    address: u16,
    fmt: DisplayFormat,
    fc: FunctionCode,
) -> Option<f64> {
    let idx = rows.iter().position(|r| r.address == address)?;

    if fc.is_bit() {
        return Some(if rows[idx].value != 0 { 1.0 } else { 0.0 });
    }

    let reg_count = fmt.register_count();
    if reg_count > 1 && idx % reg_count != 0 {
        return None;
    }

    match fmt {
        DisplayFormat::Unsigned => Some(rows[idx].value as f64),
        DisplayFormat::Signed => Some((rows[idx].value as i16) as f64),
        DisplayFormat::Hex | DisplayFormat::Binary => Some(rows[idx].value as f64),
        DisplayFormat::Long => {
            if idx + 1 < rows.len() {
                let hi = rows[idx].value as u32;
                let lo = rows[idx + 1].value as u32;
                Some(((hi << 16 | lo) as i32) as f64)
            } else {
                None
            }
        }
        DisplayFormat::LongInverse => {
            if idx + 1 < rows.len() {
                let lo = rows[idx].value as u32;
                let hi = rows[idx + 1].value as u32;
                Some(((hi << 16 | lo) as i32) as f64)
            } else {
                None
            }
        }
        DisplayFormat::Float => {
            if idx + 1 < rows.len() {
                let bytes = u32::from(rows[idx].value) << 16 | u32::from(rows[idx + 1].value);
                Some(f32::from_bits(bytes) as f64)
            } else {
                None
            }
        }
        DisplayFormat::FloatInverse => {
            if idx + 1 < rows.len() {
                let bytes = u32::from(rows[idx + 1].value) << 16 | u32::from(rows[idx].value);
                Some(f32::from_bits(bytes) as f64)
            } else {
                None
            }
        }
        DisplayFormat::Double => {
            if idx + 3 < rows.len() {
                let b: u64 = (u64::from(rows[idx].value) << 48)
                    | (u64::from(rows[idx + 1].value) << 32)
                    | (u64::from(rows[idx + 2].value) << 16)
                    | u64::from(rows[idx + 3].value);
                Some(f64::from_bits(b))
            } else {
                None
            }
        }
        DisplayFormat::DoubleInverse => {
            if idx + 3 < rows.len() {
                let b: u64 = (u64::from(rows[idx + 3].value) << 48)
                    | (u64::from(rows[idx + 2].value) << 32)
                    | (u64::from(rows[idx + 1].value) << 16)
                    | u64::from(rows[idx].value);
                Some(f64::from_bits(b))
            } else {
                None
            }
        }
    }
}
