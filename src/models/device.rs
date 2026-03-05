use serde::Deserialize;

use super::expose::{Expose, FlattenedExpose, flatten_exposes};

/// Device definition containing model info and exposes.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceDefinition {
    pub description: Option<String>,
    pub model: Option<String>,
    pub vendor: Option<String>,
    pub exposes: Option<Vec<Expose>>,
}

/// A Zigbee device discovered from Zigbee2MQTT.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Device {
    pub disabled: Option<bool>,
    pub friendly_name: String,
    pub ieee_address: String,
    pub interview_completed: Option<bool>,
    pub manufacturer: Option<String>,
    pub model_id: Option<String>,
    pub network_address: Option<u32>,
    pub supported: Option<bool>,
    #[serde(rename = "type")]
    pub device_type: Option<String>,
    pub definition: Option<DeviceDefinition>,
}

impl Device {
    /// Returns true if this device is eligible for monitoring.
    /// A device must be supported, not disabled, and have completed the interview.
    pub fn is_eligible(&self) -> bool {
        let supported = self.supported.unwrap_or(false);
        let disabled = self.disabled.unwrap_or(false);
        let interview_completed = self.interview_completed.unwrap_or(false);

        supported && !disabled && interview_completed
    }

    /// Returns the MQTT topic for this device.
    pub fn mqtt_topic(&self) -> String {
        format!("zigbee2mqtt/{}", self.friendly_name)
    }

    /// Returns the flattened exposes for this device.
    pub fn flattened_exposes(&self) -> Vec<FlattenedExpose> {
        self.definition
            .as_ref()
            .and_then(|d| d.exposes.as_ref())
            .map(|exposes| flatten_exposes(exposes))
            .unwrap_or_default()
    }
}

/// Information about a device including its flattened exposes.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub device: Device,
    pub exposes: Vec<FlattenedExpose>,
}

impl DeviceInfo {
    pub fn new(device: Device) -> Self {
        let exposes = device.flattened_exposes();
        Self { device, exposes }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_device(supported: bool, disabled: bool, interview_completed: bool) -> Device {
        Device {
            disabled: Some(disabled),
            friendly_name: "test_device".to_string(),
            ieee_address: "0x00158d0001234567".to_string(),
            interview_completed: Some(interview_completed),
            manufacturer: Some("Test".to_string()),
            model_id: Some("TEST-001".to_string()),
            network_address: Some(12345),
            supported: Some(supported),
            device_type: Some("EndDevice".to_string()),
            definition: None,
        }
    }

    #[test]
    fn test_device_is_eligible() {
        // Eligible: supported, not disabled, interview completed
        let eligible = create_test_device(true, false, true);
        assert!(eligible.is_eligible());

        // Not eligible: not supported
        let not_supported = create_test_device(false, false, true);
        assert!(!not_supported.is_eligible());

        // Not eligible: disabled
        let disabled = create_test_device(true, true, true);
        assert!(!disabled.is_eligible());

        // Not eligible: interview not completed
        let no_interview = create_test_device(true, false, false);
        assert!(!no_interview.is_eligible());
    }

    #[test]
    fn test_mqtt_topic() {
        let device = Device {
            disabled: None,
            friendly_name: "living_room_sensor".to_string(),
            ieee_address: "0x00158d0001234567".to_string(),
            interview_completed: None,
            manufacturer: None,
            model_id: None,
            network_address: None,
            supported: None,
            device_type: None,
            definition: None,
        };

        assert_eq!(device.mqtt_topic(), "zigbee2mqtt/living_room_sensor");
    }

    #[test]
    fn test_device_deserialization() {
        let json = r#"{
            "disabled": false,
            "friendly_name": "test_sensor",
            "ieee_address": "0x00158d0001234567",
            "interview_completed": true,
            "manufacturer": "LUMI",
            "model_id": "lumi.sensor_ht.agl02",
            "network_address": 54321,
            "supported": true,
            "type": "EndDevice",
            "definition": {
                "description": "Temperature and humidity sensor",
                "model": "WSDCGQ12LM",
                "vendor": "Aqara",
                "exposes": [
                    {
                        "type": "numeric",
                        "property": "temperature",
                        "unit": "°C",
                        "access": 1
                    }
                ]
            }
        }"#;

        let device: Device = serde_json::from_str(json).unwrap();
        assert_eq!(device.friendly_name, "test_sensor");
        assert_eq!(device.ieee_address, "0x00158d0001234567");
        assert!(device.is_eligible());

        let exposes = device.flattened_exposes();
        assert_eq!(exposes.len(), 1);
        assert_eq!(exposes[0].property, "temperature");
    }
}
