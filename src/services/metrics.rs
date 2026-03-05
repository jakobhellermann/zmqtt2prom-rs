use std::collections::HashMap;
use std::sync::Arc;

use prometheus::{Encoder, GaugeVec, Opts, Registry, TextEncoder};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::models::{DeviceInfo, ExposeType, FlattenedExpose};
use crate::services::flatten_payload;

/// Properties that should be skipped when creating metrics.
const SKIPPED_PROPERTIES: &[&str] = &["power_on_behavior"];

/// Labels used for Prometheus metrics.
const METRIC_LABELS: &[&str] = &[
    "friendly_name",
    "manufacturer",
    "ieee_address",
    "model_id",
    "type",
    "property",
    "unit",
];

/// Manages Prometheus metrics for Zigbee devices.
pub struct MetricsManager {
    registry: Registry,
    gauge: GaugeVec,
    device_registry: Arc<RwLock<HashMap<String, DeviceInfo>>>,
}

impl MetricsManager {
    pub fn new(device_registry: Arc<RwLock<HashMap<String, DeviceInfo>>>) -> Self {
        let registry = Registry::new();

        let opts = Opts::new("mqtt2prom_gauge", "Zigbee2MQTT device metrics");
        let gauge = GaugeVec::new(opts, METRIC_LABELS).expect("Failed to create gauge");

        registry
            .register(Box::new(gauge.clone()))
            .expect("Failed to register gauge");

        Self {
            registry,
            gauge,
            device_registry,
        }
    }

    /// Process a device payload and update metrics.
    pub async fn process_payload(&self, friendly_name: &str, payload: &[u8]) {
        let device_registry = self.device_registry.read().await;

        let device_info = match device_registry.get(friendly_name) {
            Some(info) => info,
            None => {
                debug!("No device info for {}, skipping payload", friendly_name);
                return;
            }
        };

        let payload_value: serde_json::Value = match serde_json::from_slice(payload) {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to parse payload for {}: {}", friendly_name, e);
                return;
            }
        };

        let flattened = flatten_payload(&payload_value);

        for expose in &device_info.exposes {
            if SKIPPED_PROPERTIES.contains(&expose.property.as_str()) {
                continue;
            }

            let value = match flattened.get(&expose.property) {
                Some(v) => v,
                None => {
                    debug!(
                        "Property {} not found in payload for {}",
                        expose.property, friendly_name
                    );
                    continue;
                }
            };

            if value.is_null() {
                debug!(
                    "Null value for property {} on {}",
                    expose.property, friendly_name
                );
                continue;
            }

            let metric_value = match self.convert_to_metric_value(value, expose) {
                Some(v) => v,
                None => {
                    warn!(
                        "Could not convert value for {} on {}: {:?}",
                        expose.property, friendly_name, value
                    );
                    continue;
                }
            };

            let labels = [
                friendly_name,
                device_info.device.manufacturer.as_deref().unwrap_or(""),
                &device_info.device.ieee_address,
                device_info.device.model_id.as_deref().unwrap_or(""),
                device_info.device.device_type.as_deref().unwrap_or(""),
                &expose.property,
                expose.unit.as_deref().unwrap_or(""),
            ];

            if let Ok(metric) = self.gauge.get_metric_with_label_values(&labels) {
                metric.set(metric_value);
            }
        }
    }

    /// Convert a JSON value to a metric value based on the expose type.
    fn convert_to_metric_value(
        &self,
        value: &serde_json::Value,
        expose: &FlattenedExpose,
    ) -> Option<f64> {
        match &expose.expose_type {
            ExposeType::Numeric => {
                // Try to get as number directly
                if let Some(n) = value.as_f64() {
                    return Some(n);
                }
                if let Some(n) = value.as_i64() {
                    return Some(n as f64);
                }
                None
            }
            ExposeType::Binary => {
                // Check against valueOn/valueOff
                if let Some(value_on) = &expose.value_on
                    && value_on.matches(value)
                {
                    return Some(1.0);
                }
                if let Some(value_off) = &expose.value_off
                    && value_off.matches(value)
                {
                    return Some(0.0);
                }
                // Fallback: treat as boolean
                if let Some(b) = value.as_bool() {
                    return Some(if b { 1.0 } else { 0.0 });
                }
                None
            }
            ExposeType::Enum | ExposeType::Text => {
                // Enums and text are not directly convertible to metrics
                debug!(
                    "Unsupported expose type for metrics: {:?}",
                    expose.expose_type
                );
                None
            }
            _ => None,
        }
    }

    /// Render metrics in Prometheus text format.
    pub fn render(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::with_capacity(20 * 1024);
        encoder
            .encode(&metric_families, &mut buffer)
            .expect("Failed to encode metrics");
        String::from_utf8(buffer).expect("Metrics are not valid UTF-8")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BinaryValue, Device, DeviceInfo, ExposeType, FlattenedExpose};

    fn create_test_device_registry() -> Arc<RwLock<HashMap<String, DeviceInfo>>> {
        let mut registry = HashMap::new();

        let device = Device {
            disabled: Some(false),
            friendly_name: "test_sensor".to_string(),
            ieee_address: "0x00158d0001234567".to_string(),
            interview_completed: Some(true),
            manufacturer: Some("Test".to_string()),
            model_id: Some("TEST-001".to_string()),
            network_address: Some(12345),
            supported: Some(true),
            device_type: Some("EndDevice".to_string()),
            definition: None,
        };

        let exposes = vec![
            FlattenedExpose {
                property: "temperature".to_string(),
                expose_type: ExposeType::Numeric,
                unit: Some("°C".to_string()),
                value_on: None,
                value_off: None,
            },
            FlattenedExpose {
                property: "state".to_string(),
                expose_type: ExposeType::Binary,
                unit: None,
                value_on: Some(BinaryValue::String("ON".to_string())),
                value_off: Some(BinaryValue::String("OFF".to_string())),
            },
        ];

        let device_info = DeviceInfo { device, exposes };

        registry.insert("test_sensor".to_string(), device_info);

        Arc::new(RwLock::new(registry))
    }

    #[tokio::test]
    async fn test_process_numeric_payload() {
        let device_registry = create_test_device_registry();
        let manager = MetricsManager::new(device_registry);

        let payload = br#"{"temperature": 25.5}"#;
        manager.process_payload("test_sensor", payload).await;

        let metrics = manager.render();
        assert!(metrics.contains("mqtt2prom_gauge"));
        assert!(metrics.contains("temperature"));
        assert!(metrics.contains("25.5"));
    }

    #[tokio::test]
    async fn test_process_binary_payload() {
        let device_registry = create_test_device_registry();
        let manager = MetricsManager::new(device_registry);

        let payload = br#"{"state": "ON"}"#;
        manager.process_payload("test_sensor", payload).await;

        let metrics = manager.render();
        assert!(metrics.contains("mqtt2prom_gauge"));
        assert!(metrics.contains("state"));
    }

    #[tokio::test]
    async fn test_skip_null_values() {
        let device_registry = create_test_device_registry();
        let manager = MetricsManager::new(device_registry);

        let payload = br#"{"temperature": null}"#;
        manager.process_payload("test_sensor", payload).await;

        // Should not create a metric for null value
        let metrics = manager.render();
        // The gauge should be registered but empty
        assert!(!metrics.contains("25.5"));
    }

    #[tokio::test]
    async fn test_nested_payload() {
        let mut registry = HashMap::new();

        let device = Device {
            disabled: Some(false),
            friendly_name: "nested_sensor".to_string(),
            ieee_address: "0x00158d0001234567".to_string(),
            interview_completed: Some(true),
            manufacturer: Some("Test".to_string()),
            model_id: Some("TEST-001".to_string()),
            network_address: Some(12345),
            supported: Some(true),
            device_type: Some("EndDevice".to_string()),
            definition: None,
        };

        let exposes = vec![FlattenedExpose {
            property: "color_x".to_string(),
            expose_type: ExposeType::Numeric,
            unit: None,
            value_on: None,
            value_off: None,
        }];

        registry.insert("nested_sensor".to_string(), DeviceInfo { device, exposes });

        let device_registry = Arc::new(RwLock::new(registry));
        let manager = MetricsManager::new(device_registry);

        let payload = br#"{"color": {"x": 0.3}}"#;
        manager.process_payload("nested_sensor", payload).await;

        let metrics = manager.render();
        assert!(metrics.contains("color_x"));
        assert!(metrics.contains("0.3"));
    }
}
