use std::time::Duration;

use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use tokio::sync::mpsc;

use super::config::MqttConfig;
use crate::core::{AppEvent, MqttMessage, MqttVersion, Source};
use crate::events::EventTx;

pub enum MqttCommand {
    Subscribe(String),
    Unsubscribe(String),
    Publish { topic: String, payload: String },
}

pub struct MqttSource {
    config: MqttConfig,
    tx: EventTx,
    cmd_rx: mpsc::UnboundedReceiver<MqttCommand>,
}

impl MqttSource {
    pub fn new(config: MqttConfig, tx: EventTx) -> (Self, mpsc::UnboundedSender<MqttCommand>) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        (Self { config, tx, cmd_rx }, cmd_tx)
    }

    pub async fn run(self) {
        match self.config.version {
            MqttVersion::V311 => self.run_v311().await,
            MqttVersion::V5 => self.run_v5().await,
        }
    }

    async fn run_v311(mut self) {
        let mut opts =
            MqttOptions::new(&self.config.client_id, &self.config.host, self.config.port);
        opts.set_keep_alive(Duration::from_secs(self.config.keep_alive_secs));

        if let Some(username) = &self.config.username {
            opts.set_credentials(username, self.config.password.as_deref().unwrap_or(""));
        }

        let (client, mut eventloop) = AsyncClient::new(opts, 128);

        for topic in &self.config.topics {
            if let Err(e) = client.subscribe(topic, QoS::AtMostOnce).await {
                let _ = self
                    .tx
                    .send(AppEvent::Error(format!("subscribe failed: {e}")));
            }
        }

        loop {
            tokio::select! {
                result = eventloop.poll() => {
                    match result {
                        Ok(Event::Incoming(Packet::ConnAck(_))) => {
                            let _ = self.tx.send(AppEvent::Connected);
                        }
                        Ok(Event::Incoming(Packet::Publish(p))) => {
                            let msg = MqttMessage {
                                topic: p.topic,
                                payload: p.payload.to_vec(),
                                qos: p.qos as u8,
                                retained: p.retain,
                            };
                            let _ = self.tx.send(AppEvent::MqttMessage(msg));
                        }
                        Err(e) => {
                            let _ = self.tx.send(AppEvent::Error(format!("{e}")));
                            let _ = self.tx.send(AppEvent::Disconnected);
                            tokio::select! {
                                _ = tokio::time::sleep(Duration::from_secs(3)) => {}
                                msg = self.cmd_rx.recv() => {
                                    match msg {
                                        None => break,
                                        Some(_) => {}
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(MqttCommand::Subscribe(topic)) => {
                            if let Err(e) = client.subscribe(&topic, QoS::AtMostOnce).await {
                                let _ = self.tx.send(AppEvent::Error(format!("subscribe failed: {e}")));
                            }
                        }
                        Some(MqttCommand::Unsubscribe(topic)) => {
                            let _ = client.unsubscribe(&topic).await;
                        }
                        Some(MqttCommand::Publish { topic, payload }) => {
                            if let Err(e) = client.publish(&topic, QoS::AtMostOnce, false, payload.into_bytes()).await {
                                let _ = self.tx.send(AppEvent::Error(format!("publish failed: {e}")));
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    }

    async fn run_v5(mut self) {
        let mut opts = rumqttc::v5::MqttOptions::new(
            &self.config.client_id,
            &self.config.host,
            self.config.port,
        );
        opts.set_keep_alive(Duration::from_secs(self.config.keep_alive_secs));

        if let Some(username) = &self.config.username {
            opts.set_credentials(username, self.config.password.as_deref().unwrap_or(""));
        }

        let (client, mut eventloop) = rumqttc::v5::AsyncClient::new(opts, 128);

        for topic in &self.config.topics {
            if let Err(e) = client
                .subscribe(topic, rumqttc::v5::mqttbytes::QoS::AtMostOnce)
                .await
            {
                let _ = self
                    .tx
                    .send(AppEvent::Error(format!("subscribe failed: {e}")));
            }
        }

        loop {
            tokio::select! {
                result = eventloop.poll() => {
                    match result {
                        Ok(rumqttc::v5::Event::Incoming(
                            rumqttc::v5::mqttbytes::v5::Packet::ConnAck(_)
                        )) => {
                            let _ = self.tx.send(AppEvent::Connected);
                        }
                        Ok(rumqttc::v5::Event::Incoming(
                            rumqttc::v5::mqttbytes::v5::Packet::Publish(p)
                        )) => {
                            let msg = MqttMessage {
                                topic:    String::from_utf8_lossy(&p.topic).into_owned(),
                                payload:  p.payload.to_vec(),
                                qos:      p.qos as u8,
                                retained: p.retain,
                            };
                            let _ = self.tx.send(AppEvent::MqttMessage(msg));
                        }
                        Err(e) => {
                            let _ = self.tx.send(AppEvent::Error(format!("{e}")));
                            let _ = self.tx.send(AppEvent::Disconnected);
                            tokio::select! {
                                _ = tokio::time::sleep(Duration::from_secs(3)) => {}
                                msg = self.cmd_rx.recv() => {
                                    match msg {
                                        None => break,
                                        Some(_) => {}
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(MqttCommand::Subscribe(topic)) => {
                            if let Err(e) = client.subscribe(&topic, rumqttc::v5::mqttbytes::QoS::AtMostOnce).await {
                                let _ = self.tx.send(AppEvent::Error(format!("subscribe failed: {e}")));
                            }
                        }
                        Some(MqttCommand::Unsubscribe(topic)) => {
                            let _ = client.unsubscribe(&topic).await;
                        }
                        Some(MqttCommand::Publish { topic, payload }) => {
                            if let Err(e) = client.publish(&topic, rumqttc::v5::mqttbytes::QoS::AtMostOnce, false, payload.into_bytes()).await {
                                let _ = self.tx.send(AppEvent::Error(format!("publish failed: {e}")));
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    }
}

impl Source for MqttSource {
    async fn connect(&mut self) {}
    async fn poll(&mut self) {}
}
