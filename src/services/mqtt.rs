use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::models::{Device, DeviceInfo};
use crate::services::MetricsManager;

const BRIDGE_DEVICES_TOPIC: &str = "zigbee2mqtt/bridge/devices";
const ZIGBEE2MQTT_PREFIX: &str = "zigbee2mqtt/";

#[derive(Debug, Error)]
pub enum MqttError {
    #[error("MQTT connection failed: {0}")]
    ConnectionFailed(#[from] rumqttc::ConnectionError),
    #[error("MQTT client error: {0}")]
    ClientError(#[from] rumqttc::ClientError),
}

/// Configuration for the MQTT connection.
#[derive(Debug, Clone)]
pub struct MqttConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

/// MQTT service for connecting to Zigbee2MQTT and discovering devices.
pub struct MqttService {
    config: MqttConfig,
    device_registry: Arc<RwLock<HashMap<String, DeviceInfo>>>,
    metrics_manager: Arc<MetricsManager>,
}

impl MqttService {
    pub fn new(
        config: MqttConfig,
        device_registry: Arc<RwLock<HashMap<String, DeviceInfo>>>,
        metrics_manager: Arc<MetricsManager>,
    ) -> Self {
        Self {
            config,
            device_registry,
            metrics_manager,
        }
    }

    /// Run the MQTT service, connecting and handling messages.
    pub async fn run(&self, mut shutdown_rx: tokio::sync::watch::Receiver<bool>) {
        loop {
            if *shutdown_rx.borrow() {
                info!("MQTT service shutting down");
                return;
            }

            match self.connect_and_run(&mut shutdown_rx).await {
                Ok(()) => {
                    info!("MQTT connection closed gracefully");
                    return;
                }
                Err(e) => {
                    error!("MQTT connection error: {}", e);
                    info!("Reconnecting in 2 seconds...");
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(2)) => {}
                        _ = shutdown_rx.changed() => {
                            if *shutdown_rx.borrow() {
                                return;
                            }
                        }
                    }
                }
            }
        }
    }

    async fn connect_and_run(
        &self,
        shutdown_rx: &mut tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), MqttError> {
        let client_id = format!("zmqtt2prom-{}", uuid::Uuid::new_v4());
        let mut mqtt_options = MqttOptions::new(&client_id, &self.config.host, self.config.port);
        mqtt_options.set_keep_alive(Duration::from_secs(30));
        mqtt_options.set_clean_session(true);
        mqtt_options.set_max_packet_size(256 * 1024, 256 * 1024); // 256KB for incoming and outgoing

        if let (Some(username), Some(password)) = (&self.config.username, &self.config.password) {
            mqtt_options.set_credentials(username, password);
        }

        let (client, mut eventloop) = AsyncClient::new(mqtt_options, 100);

        info!(
            "Connecting to MQTT broker at {}:{}",
            self.config.host, self.config.port
        );

        // Subscribe to device discovery topic
        client
            .subscribe(BRIDGE_DEVICES_TOPIC, QoS::AtMostOnce)
            .await?;
        info!("Subscribed to {}", BRIDGE_DEVICES_TOPIC);

        loop {
            tokio::select! {
                event = eventloop.poll() => {
                    match event {
                        Ok(Event::Incoming(Packet::Publish(publish))) => {
                            self.handle_message(&client, &publish.topic, &publish.payload).await;
                        }
                        Ok(Event::Incoming(Packet::ConnAck(_))) => {
                            info!("Connected to MQTT broker");
                        }
                        Ok(_) => {}
                        Err(e) => {
                            return Err(MqttError::ConnectionFailed(e));
                        }
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("Shutdown signal received, disconnecting from MQTT");
                        let _ = client.disconnect().await;
                        return Ok(());
                    }
                }
            }
        }
    }

    async fn handle_message(&self, client: &AsyncClient, topic: &str, payload: &[u8]) {
        if topic == BRIDGE_DEVICES_TOPIC {
            self.handle_device_discovery(client, payload).await;
        } else if let Some(friendly_name) = topic.strip_prefix(ZIGBEE2MQTT_PREFIX) {
            // Skip bridge-related topics
            if friendly_name.starts_with("bridge/") {
                return;
            }
            self.handle_device_message(friendly_name, payload).await;
        }
    }

    async fn handle_device_discovery(&self, client: &AsyncClient, payload: &[u8]) {
        let devices: Vec<Device> = match serde_json::from_slice(payload) {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to parse device discovery message: {}", e);
                return;
            }
        };

        let eligible_devices: Vec<_> = devices.into_iter().filter(|d| d.is_eligible()).collect();

        info!("Discovered {} eligible devices", eligible_devices.len());

        let mut registry = self.device_registry.write().await;
        registry.clear();

        for device in eligible_devices {
            let friendly_name = device.friendly_name.clone();
            let topic = device.mqtt_topic();
            let device_info = DeviceInfo::new(device);

            debug!(
                "Device {}: {} exposes",
                friendly_name,
                device_info.exposes.len()
            );

            registry.insert(friendly_name, device_info);

            // Subscribe to device topic
            if let Err(e) = client.subscribe(&topic, QoS::AtLeastOnce).await {
                warn!("Failed to subscribe to {}: {}", topic, e);
            } else {
                debug!("Subscribed to {}", topic);
            }
        }
    }

    async fn handle_device_message(&self, friendly_name: &str, payload: &[u8]) {
        self.metrics_manager
            .process_payload(friendly_name, payload)
            .await;
    }
}
