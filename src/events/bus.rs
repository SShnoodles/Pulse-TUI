use tokio::sync::mpsc;

use crate::core::AppEvent;

/// Sender handle — clone into every producer task
pub type EventTx = mpsc::UnboundedSender<AppEvent>;

/// Receiver handle — owned by the main loop
pub type EventRx = mpsc::UnboundedReceiver<AppEvent>;

pub fn new_event_channel() -> (EventTx, EventRx) {
    mpsc::unbounded_channel()
}
