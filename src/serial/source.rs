use std::io::Write;

use tokio::sync::mpsc;

use super::config::SerialConfig;
use crate::core::AppEvent;
use crate::events::EventTx;

pub enum SerialCommand {
    Send(Vec<u8>),
}

pub struct SerialSource {
    config: SerialConfig,
    tx: EventTx,
    cmd_rx: mpsc::UnboundedReceiver<SerialCommand>,
}

impl SerialSource {
    pub fn new(config: SerialConfig, tx: EventTx) -> (Self, mpsc::UnboundedSender<SerialCommand>) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        (Self { config, tx, cmd_rx }, cmd_tx)
    }

    pub async fn run(mut self) {
        let mut port = match serialport::new(&self.config.port, self.config.baud_rate)
            .data_bits(self.config.data_bits)
            .parity(self.config.parity)
            .stop_bits(self.config.stop_bits)
            .timeout(std::time::Duration::from_millis(200))
            .open()
        {
            Ok(p) => p,
            Err(e) => {
                let _ = self.tx.send(AppEvent::Error(format!("serial open: {e}")));
                let _ = self.tx.send(AppEvent::Disconnected);
                return;
            }
        };

        let mut writer = match port.try_clone() {
            Ok(w) => w,
            Err(e) => {
                let _ = self.tx.send(AppEvent::Error(format!("serial clone: {e}")));
                let _ = self.tx.send(AppEvent::Disconnected);
                return;
            }
        };

        let _ = self.tx.send(AppEvent::Connected);

        let (line_tx, mut line_rx) = mpsc::unbounded_channel::<Result<String, String>>();
        std::thread::spawn(move || {
            let mut raw_buf = [0u8; 1024];
            let mut line_buf = String::new();
            loop {
                match port.read(&mut raw_buf) {
                    Ok(0) => {
                        // Flush any partial data before signalling close.
                        if !line_buf.is_empty() {
                            let _ = line_tx.send(Ok(line_buf.drain(..).collect()));
                        }
                        let _ = line_tx.send(Err("port closed".into()));
                        break;
                    }
                    Ok(n) => {
                        // Accumulate bytes; emit on \r or \n (handles \r\n, \r, \n).
                        for ch in String::from_utf8_lossy(&raw_buf[..n]).chars() {
                            if ch == '\n' || ch == '\r' {
                                if !line_buf.is_empty() {
                                    if line_tx.send(Ok(line_buf.drain(..).collect())).is_err() {
                                        return;
                                    }
                                }
                            } else {
                                line_buf.push(ch);
                            }
                        }
                    }
                    Err(e)
                        if e.kind() == std::io::ErrorKind::TimedOut
                            || e.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                        // On timeout flush partial data so non-newline devices still show output.
                        if !line_buf.is_empty() {
                            if line_tx.send(Ok(line_buf.drain(..).collect())).is_err() {
                                return;
                            }
                        }
                        if line_tx.is_closed() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = line_tx.send(Err(e.to_string()));
                        break;
                    }
                }
            }
        });

        loop {
            tokio::select! {
                result = line_rx.recv() => match result {
                    Some(Ok(line)) => {
                        let _ = self.tx.send(AppEvent::SerialLine(line));
                    }
                    Some(Err(e)) => {
                        let _ = self.tx.send(AppEvent::Error(format!("serial: {e}")));
                        let _ = self.tx.send(AppEvent::Disconnected);
                        break;
                    }
                    None => {
                        let _ = self.tx.send(AppEvent::Disconnected);
                        break;
                    }
                },
                cmd = self.cmd_rx.recv() => match cmd {
                    Some(SerialCommand::Send(data)) => {
                        if let Err(e) = writer.write_all(&data) {
                            let _ = self.tx.send(AppEvent::Error(format!("serial write: {e}")));
                        }
                    }
                    None => break,
                },
            }
        }
    }
}
