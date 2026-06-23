use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use opcua::{
    client::{ClientBuilder, IdentityToken, Password},
    crypto::SecurityPolicy,
    types::{
        AttributeId, MessageSecurityMode, NodeId, NumericRange, QualifiedName, ReadValueId,
        StatusCode, TimestampsToReturn, Variant,
    },
};
use tokio::sync::mpsc;

use super::config::OpcUaConfig;
use crate::core::AppEvent;
use crate::events::EventTx;

// ── Command ────────────────────────────────────────────────────────────────────

/// Commands that the main loop can send into the running OPC UA task.
pub enum OpcUaCommand {
    /// Add one monitored node.
    AddNode(String),
    /// Remove one monitored node.
    RemoveNode(String),
}

// ── Source ─────────────────────────────────────────────────────────────────────

pub struct OpcUaSource {
    config: OpcUaConfig,
    tx: EventTx,
    cmd_rx: mpsc::UnboundedReceiver<OpcUaCommand>,
}

impl OpcUaSource {
    pub fn new(config: OpcUaConfig, tx: EventTx) -> (Self, mpsc::UnboundedSender<OpcUaCommand>) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        (Self { config, tx, cmd_rx }, cmd_tx)
    }

    pub async fn run(mut self) {
        let poll_ms = self.config.poll_interval_ms;
        let mut node_ids = self.config.node_ids.clone();

        loop {
            // ── Build client & session ──────────────────────────────────────
            let mut client = match ClientBuilder::new()
                .application_name("Pulse-TUI")
                .application_uri("urn:pulse-tui")
                .trust_server_certs(true)
                .session_retry_limit(0)
                .client()
            {
                Ok(c) => c,
                Err(errs) => {
                    let _ = self.tx.send(AppEvent::Error(format!(
                        "OPC UA client build failed: {}",
                        errs.join("; ")
                    )));
                    let _ = self.tx.send(AppEvent::Disconnected);
                    return;
                }
            };

            let endpoint = (
                self.config.endpoint_url.as_str(),
                SecurityPolicy::None.to_str(),
                MessageSecurityMode::None,
            );

            let identity = if self.config.username.is_empty() {
                IdentityToken::Anonymous
            } else {
                IdentityToken::UserName(
                    self.config.username.clone(),
                    Password::new(self.config.password.clone()),
                )
            };

            let (session, event_loop) = match client
                .connect_to_matching_endpoint(endpoint, identity)
                .await
            {
                Ok(pair) => pair,
                Err(e) => {
                    let _ = self
                        .tx
                        .send(AppEvent::Error(format!("OPC UA connect: {}", e)));
                    let _ = self.tx.send(AppEvent::Disconnected);
                    if self.drain_for(Duration::from_secs(3)).await {
                        return;
                    }
                    continue;
                }
            };

            let session = Arc::clone(&session);
            let handle = event_loop.spawn();

            // Wait until the OPC UA handshake completes.
            if !session.wait_for_connection().await {
                let _ = self
                    .tx
                    .send(AppEvent::Error("OPC UA: failed to connect".into()));
                let _ = self.tx.send(AppEvent::Disconnected);
                let _ = session.disconnect().await;
                let _ = handle.await;
                if self.drain_for(Duration::from_secs(3)).await {
                    return;
                }
                continue;
            }

            let _ = self.tx.send(AppEvent::Connected);

            // ── Poll loop ───────────────────────────────────────────────────
            'connected: loop {
                let mut parsed: Vec<(String, NodeId)> = Vec::new();
                for id in &node_ids {
                    match NodeId::from_str(id) {
                        Ok(n) => parsed.push((id.clone(), n)),
                        Err(_) => {
                            let _ = self
                                .tx
                                .send(AppEvent::Error(format!("OPC UA: invalid NodeId '{id}'")));
                        }
                    }
                }

                if parsed.is_empty() {
                    // No nodes to poll yet — wait for an AddNode command instead of
                    // plain-sleeping, otherwise commands queued in cmd_rx are never read
                    // and the loop spins forever ignoring AddNode requests.
                    let deadline = tokio::time::Instant::now() + Duration::from_millis(poll_ms);
                    loop {
                        let remaining =
                            deadline.saturating_duration_since(tokio::time::Instant::now());
                        if remaining.is_zero() {
                            break;
                        }
                        tokio::select! {
                            _ = tokio::time::sleep(remaining) => break,
                            cmd = self.cmd_rx.recv() => match cmd {
                                None => {
                                    let _ = session.disconnect().await;
                                    let _ = handle.await;
                                    return;
                                }
                                Some(OpcUaCommand::AddNode(id)) => {
                                    if !node_ids.contains(&id) {
                                        node_ids.push(id);
                                    }
                                }
                                Some(OpcUaCommand::RemoveNode(id)) => {
                                    node_ids.retain(|n| n != &id);
                                }
                            }
                        }
                    }
                    continue 'connected;
                }

                let mut reads = Vec::with_capacity(parsed.len() * 3);
                for (_, n) in &parsed {
                    reads.push(ReadValueId {
                        node_id: n.clone(),
                        attribute_id: AttributeId::DisplayName as u32,
                        index_range: NumericRange::None,
                        data_encoding: QualifiedName::null(),
                    });
                    reads.push(ReadValueId {
                        node_id: n.clone(),
                        attribute_id: AttributeId::Value as u32,
                        index_range: NumericRange::None,
                        data_encoding: QualifiedName::null(),
                    });
                    reads.push(ReadValueId {
                        node_id: n.clone(),
                        attribute_id: AttributeId::DataType as u32,
                        index_range: NumericRange::None,
                        data_encoding: QualifiedName::null(),
                    });
                }

                match session.read(&reads, TimestampsToReturn::Both, 0.0).await {
                    Ok(results) => {
                        for (i, (node_id_text, _)) in parsed.iter().enumerate() {
                            let base = i * 3;
                            if base + 2 >= results.len() {
                                break;
                            }
                            let dv_name = &results[base];
                            let dv_value = &results[base + 1];
                            let dv_type: &opcua::types::DataValue = &results[base + 2];

                            let display_name = dv_name
                                .value
                                .as_ref()
                                .map(format_display_name)
                                .unwrap_or_else(|| "—".into());
                            let value = dv_value
                                .value
                                .as_ref()
                                .map(format_value)
                                .unwrap_or_else(|| "—".into());
                            let data_type = dv_type
                                .value
                                .as_ref()
                                .map(format_data_type)
                                .unwrap_or_else(|| "—".into());
                            let source_timestamp = dv_value
                                .source_timestamp
                                .as_ref()
                                .map(|t| t.to_rfc3339())
                                .unwrap_or_else(|| "—".into());
                            let server_timestamp = dv_value
                                .server_timestamp
                                .as_ref()
                                .map(|t| t.to_rfc3339())
                                .unwrap_or_else(|| "—".into());

                            let _ = self.tx.send(AppEvent::OpcUaData {
                                node_id: node_id_text.clone(),
                                display_name,
                                value,
                                data_type,
                                source_timestamp,
                                server_timestamp,
                            });
                        }
                    }
                    Err(e)
                        if e == StatusCode::BadNotConnected
                            || e == StatusCode::BadConnectionClosed =>
                    {
                        let _ = self.tx.send(AppEvent::Error(format!("OPC UA read: {e}")));
                        let _ = self.tx.send(AppEvent::Disconnected);
                        break 'connected;
                    }
                    Err(e) => {
                        // Transient errors are surfaced but polling continues.
                        let _ = self.tx.send(AppEvent::Error(format!("OPC UA read: {e}")));
                    }
                }

                // Wait poll interval, accepting commands in the meantime.
                let deadline = tokio::time::Instant::now() + Duration::from_millis(poll_ms);
                loop {
                    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                    if remaining.is_zero() {
                        break;
                    }
                    tokio::select! {
                        _ = tokio::time::sleep(remaining) => break,
                        cmd = self.cmd_rx.recv() => match cmd {
                            None => {
                                let _ = session.disconnect().await;
                                let _ = handle.await;
                                return;
                            }
                            Some(OpcUaCommand::AddNode(id)) => {
                                if !node_ids.contains(&id) {
                                    node_ids.push(id);
                                }
                            }
                            Some(OpcUaCommand::RemoveNode(id)) => {
                                node_ids.retain(|node_id| node_id != &id);
                            }
                        }
                    }
                }
            }

            // Disconnect cleanly before reconnect wait.
            let _ = session.disconnect().await;
            let _ = handle.await;

            if self.drain_for(Duration::from_secs(3)).await {
                return;
            }

            // Sync local node_ids with any additions/removals made during drain_for.
            node_ids = self.config.node_ids.clone();
        }
    }

    /// Wait `dur`; drain command channel. Returns `true` if the channel was closed (shutdown).
    async fn drain_for(&mut self, dur: Duration) -> bool {
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
                    Some(OpcUaCommand::AddNode(id)) => {
                        if !self.config.node_ids.contains(&id) {
                            self.config.node_ids.push(id);
                        }
                    }
                    Some(OpcUaCommand::RemoveNode(id)) => {
                        self.config.node_ids.retain(|node_id| node_id != &id);
                    }
                }
            }
        }
    }
}

fn format_display_name(value: &Variant) -> String {
    match value {
        Variant::LocalizedText(text) => text.text.to_string(),
        Variant::QualifiedName(name) => name.name.to_string(),
        Variant::String(text) => text.to_string(),
        other => other.to_string(),
    }
}

fn format_value(value: &Variant) -> String {
    match value {
        Variant::Empty => "—".into(),
        Variant::Boolean(v) => v.to_string(),
        Variant::SByte(v) => v.to_string(),
        Variant::Byte(v) => v.to_string(),
        Variant::Int16(v) => v.to_string(),
        Variant::UInt16(v) => v.to_string(),
        Variant::Int32(v) => v.to_string(),
        Variant::UInt32(v) => v.to_string(),
        Variant::Int64(v) => v.to_string(),
        Variant::UInt64(v) => v.to_string(),
        Variant::Float(v) => v.to_string(),
        Variant::Double(v) => v.to_string(),
        Variant::String(v) => v.to_string(),
        Variant::DateTime(v) => v.to_string(),
        Variant::Guid(v) => v.to_string(),
        Variant::ByteString(v) => format!("{:?}", v),
        Variant::XmlElement(v) => v.to_string(),
        Variant::QualifiedName(name) => name.name.to_string(),
        Variant::LocalizedText(text) => text.text.to_string(),
        Variant::NodeId(v) => v.to_string(),
        Variant::ExpandedNodeId(v) => v.to_string(),
        Variant::StatusCode(v) => v.to_string(),
        Variant::ExtensionObject(v) => format!("{:?}", v),
        Variant::DataValue(v) => format!("{:?}", v),
        Variant::DiagnosticInfo(v) => format!("{:?}", v),
        Variant::Variant(v) => format_value(v),
        Variant::Array(array) => {
            if array.values.is_empty() {
                "[]".into()
            } else {
                array
                    .values
                    .iter()
                    .map(format_value)
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        }
    }
}

fn format_data_type(value: &Variant) -> String {
    match value {
        Variant::NodeId(node_id) => node_id
            .as_data_type_id()
            .map(|data_type_id| format!("{:?}", data_type_id))
            .unwrap_or_else(|_| node_id.to_string()),
        Variant::ExpandedNodeId(node_id) => node_id
            .node_id
            .as_data_type_id()
            .map(|data_type_id| format!("{:?}", data_type_id))
            .unwrap_or_else(|_| node_id.to_string()),
        Variant::String(text) => text.to_string(),
        other => match other {
            Variant::LocalizedText(text) => text.text.to_string(),
            Variant::QualifiedName(name) => name.name.to_string(),
            _ => other.to_string(),
        },
    }
}
