use crossterm::event::KeyEvent;

use super::state::MqttMessage;

#[derive(Debug)]
pub enum AppEvent {
    Tick,
    Key(KeyEvent),
    Paste(String),
    MqttMessage(MqttMessage),
    Connected,
    Disconnected,
    Error(String),
}
