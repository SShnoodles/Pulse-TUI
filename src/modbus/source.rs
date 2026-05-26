use std::net::ToSocketAddrs;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio_modbus::client::{tcp, Context, Reader};
use tokio_modbus::Slave;

use super::config::ModbusConfig;
use crate::core::{AppEvent, FunctionCode};
use crate::events::EventTx;

// ── Command ───────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum ModbusCommand {
    SetQuery { fc: FunctionCode, start: u16, quantity: u16 },
}

// ── Source ────────────────────────────────────────────────────────────────────

pub struct ModbusSource {
    config: ModbusConfig,
    tx: EventTx,
    cmd_rx: mpsc::UnboundedReceiver<ModbusCommand>,
}

impl ModbusSource {
    pub fn new(
        config: ModbusConfig,
        tx: EventTx,
    ) -> (Self, mpsc::UnboundedSender<ModbusCommand>) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        (Self { config, tx, cmd_rx }, cmd_tx)
    }

    pub async fn run(mut self) {
        let addr_str = format!("{}:{}", self.config.host, self.config.port);

        // Resolve address once (supports both IP strings and hostnames)
        let socket_addr = match addr_str.to_socket_addrs().ok().and_then(|mut i| i.next()) {
            Some(a) => a,
            None => {
                let _ = self.tx.send(AppEvent::Error(format!("cannot resolve {addr_str}")));
                let _ = self.tx.send(AppEvent::Disconnected);
                return;
            }
        };

        let unit = Slave(self.config.unit_id);
        let mut current_query: Option<(FunctionCode, u16, u16)> = None;

        loop {
            // ── Attempt TCP connection ─────────────────────────────────────
            let conn =
                tokio::time::timeout(Duration::from_secs(5), tcp::connect_slave(socket_addr, unit))
                    .await;

            let mut ctx = match conn {
                Ok(Ok(ctx)) => {
                    let _ = self.tx.send(AppEvent::Connected);
                    ctx
                }
                Ok(Err(e)) => {
                    let _ = self.tx.send(AppEvent::Error(format!("connect: {e}")));
                    let _ = self.tx.send(AppEvent::Disconnected);
                    if self.drain_for(Duration::from_secs(3), &mut current_query).await {
                        return;
                    }
                    continue;
                }
                Err(_) => {
                    let _ = self.tx.send(AppEvent::Error("connection timed out".into()));
                    let _ = self.tx.send(AppEvent::Disconnected);
                    if self.drain_for(Duration::from_secs(3), &mut current_query).await {
                        return;
                    }
                    continue;
                }
            };

            // ── Poll loop ──────────────────────────────────────────────────
            'connected: loop {
                if let Some((fc, start, qty)) = current_query {
                    match poll_registers(&mut ctx, fc, start, qty).await {
                        Ok(values) => {
                            let _ = self.tx.send(AppEvent::ModbusData { start, values });
                        }
                        Err(e) => {
                            let _ = self.tx.send(AppEvent::Error(format!("poll: {e}")));
                            let _ = self.tx.send(AppEvent::Disconnected);
                            break 'connected;
                        }
                    }
                }

                // Wait poll interval, accepting commands in the meantime
                let deadline = tokio::time::Instant::now()
                    + Duration::from_millis(self.config.poll_interval_ms);
                loop {
                    let remaining =
                        deadline.saturating_duration_since(tokio::time::Instant::now());
                    if remaining.is_zero() {
                        break;
                    }
                    tokio::select! {
                        _ = tokio::time::sleep(remaining) => break,
                        cmd = self.cmd_rx.recv() => match cmd {
                            None => return,
                            Some(ModbusCommand::SetQuery { fc, start, quantity }) => {
                                current_query = Some((fc, start, quantity));
                            }
                        }
                    }
                }
            }

            // ── Reconnect wait ─────────────────────────────────────────────
            if self.drain_for(Duration::from_secs(3), &mut current_query).await {
                return;
            }
        }
    }

    /// Wait `dur`, processing commands; returns `true` if channel dropped (shutdown).
    async fn drain_for(
        &mut self,
        dur: Duration,
        query: &mut Option<(FunctionCode, u16, u16)>,
    ) -> bool {
        let deadline = tokio::time::Instant::now() + dur;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return false;
            }
            tokio::select! {
                _ = tokio::time::sleep(remaining) => return false,
                cmd = self.cmd_rx.recv() => match cmd {
                    None => return true,
                    Some(ModbusCommand::SetQuery { fc, start, quantity }) => {
                        *query = Some((fc, start, quantity));
                    }
                }
            }
        }
    }
}

// ── Poll helpers ──────────────────────────────────────────────────────────────

async fn poll_registers(
    ctx: &mut Context,
    fc: FunctionCode,
    start: u16,
    qty: u16,
) -> anyhow::Result<Vec<u16>> {
    match fc {
        FunctionCode::FC03Holding => Ok(ctx.read_holding_registers(start, qty).await??),
        FunctionCode::FC04Input => Ok(ctx.read_input_registers(start, qty).await??),
        FunctionCode::FC01Coil => {
            let bools = ctx.read_coils(start, qty).await??;
            Ok(bools.into_iter().map(|b| if b { 1u16 } else { 0u16 }).collect())
        }
        FunctionCode::FC02Discrete => {
            let bools = ctx.read_discrete_inputs(start, qty).await??;
            Ok(bools.into_iter().map(|b| if b { 1u16 } else { 0u16 }).collect())
        }
    }
}

