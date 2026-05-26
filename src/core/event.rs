use crossterm::event::KeyEvent;

use super::state::MqttMessage;

#[derive(Debug)]
pub enum AppEvent {
    Tick,
    Key(KeyEvent),
    Paste(String),
    MqttMessage(MqttMessage),
    /// Raw register values polled from a Modbus source.
    ModbusData { start: u16, values: Vec<u16> },
    Connected,
    Disconnected,
    Error(String),
}
