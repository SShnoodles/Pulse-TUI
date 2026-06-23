use crossterm::event::KeyEvent;

use super::state::MqttMessage;

#[derive(Debug)]
pub enum AppEvent {
    Tick,
    Key(KeyEvent),
    Paste(String),
    MqttMessage(MqttMessage),
    /// Raw register values polled from a Modbus source.
    ModbusData {
        start: u16,
        values: Vec<u16>,
    },
    /// OPC UA node sample polled from the server.
    OpcUaData {
        node_id: String,
        display_name: String,
        value: String,
        data_type: String,
        source_timestamp: String,
        server_timestamp: String,
    },
    /// A line of text received from a serial port.
    SerialLine(String),
    Connected,
    Disconnected,
    Error(String),
}
