use serde::Deserialize;

/// Represents the type of a Zigbee2MQTT expose/capability.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExposeType {
    Binary,
    Numeric,
    Enum,
    Text,
    Composite,
    Switch,
    Light,
    Climate,
    Cover,
    Fan,
    Lock,
    #[serde(other)]
    Unknown,
}

impl ExposeType {
    /// Returns true if this expose type is a generic/monitorable type.
    pub fn is_generic(&self) -> bool {
        matches!(
            self,
            ExposeType::Binary | ExposeType::Numeric | ExposeType::Enum | ExposeType::Text
        )
    }
}

/// Represents a value that can be either a boolean or a string.
/// Used for binary expose valueOn/valueOff fields.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum BinaryValue {
    Bool(bool),
    String(String),
}

impl BinaryValue {
    /// Check if this value matches the given JSON value.
    pub fn matches(&self, value: &serde_json::Value) -> bool {
        match self {
            BinaryValue::Bool(b) => value.as_bool() == Some(*b),
            BinaryValue::String(s) => value.as_str() == Some(s.as_str()),
        }
    }
}

/// Represents a device capability/expose from Zigbee2MQTT.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Expose {
    #[serde(rename = "type")]
    pub expose_type: ExposeType,
    pub property: Option<String>,
    pub name: Option<String>,
    pub unit: Option<String>,
    pub access: Option<u8>,
    pub category: Option<String>,
    pub description: Option<String>,
    pub features: Option<Vec<Expose>>,

    // Binary-specific fields
    pub value_on: Option<BinaryValue>,
    pub value_off: Option<BinaryValue>,

    // Numeric-specific fields
    pub value_min: Option<f64>,
    pub value_max: Option<f64>,
    pub value_step: Option<f64>,

    // Enum-specific fields
    pub values: Option<Vec<String>>,
}

impl Expose {
    /// Returns true if this expose has publish/read access (bit 0 set).
    pub fn has_publish_access(&self) -> bool {
        self.access.map(|a| (a & 1) != 0).unwrap_or(false)
    }

    /// Returns true if this expose should be monitored for metrics.
    pub fn is_monitorable(&self) -> bool {
        self.expose_type.is_generic() && self.has_publish_access()
    }
}

/// A flattened expose with resolved property path.
#[derive(Debug, Clone)]
pub struct FlattenedExpose {
    pub property: String,
    pub expose_type: ExposeType,
    pub unit: Option<String>,
    pub value_on: Option<BinaryValue>,
    pub value_off: Option<BinaryValue>,
}

/// Flattens exposes recursively, handling composite types.
pub fn flatten_exposes(exposes: &[Expose]) -> Vec<FlattenedExpose> {
    let mut result = Vec::new();
    flatten_exposes_recursive(exposes, "", &mut result);
    result
}

fn flatten_exposes_recursive(exposes: &[Expose], prefix: &str, result: &mut Vec<FlattenedExpose>) {
    for expose in exposes {
        let base_path = if prefix.is_empty() {
            String::new()
        } else {
            format!("{}_", prefix)
        };
        let property = if let Some(p) = &expose.property {
            format!("{}{}", base_path, p)
        } else if let Some(n) = &expose.name {
            format!("{}{}", base_path, n)
        } else {
            format!("{}unknown", base_path)
        };

        if expose.is_monitorable() {
            result.push(FlattenedExpose {
                property: property.clone(),
                expose_type: expose.expose_type.clone(),
                unit: expose.unit.clone(),
                value_on: expose.value_on.clone(),
                value_off: expose.value_off.clone(),
            });
        }

        if let Some(features) = &expose.features {
            // Composite type - recurse into features
            flatten_exposes_recursive(features, &property, result);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expose_type_is_generic() {
        assert!(ExposeType::Binary.is_generic());
        assert!(ExposeType::Numeric.is_generic());
        assert!(ExposeType::Enum.is_generic());
        assert!(ExposeType::Text.is_generic());
        assert!(!ExposeType::Composite.is_generic());
        assert!(!ExposeType::Switch.is_generic());
        assert!(!ExposeType::Light.is_generic());
    }

    #[test]
    fn test_binary_value_matches() {
        let bool_val = BinaryValue::Bool(true);
        assert!(bool_val.matches(&serde_json::Value::Bool(true)));
        assert!(!bool_val.matches(&serde_json::Value::Bool(false)));

        let str_val = BinaryValue::String("ON".to_string());
        assert!(str_val.matches(&serde_json::Value::String("ON".to_string())));
        assert!(!str_val.matches(&serde_json::Value::String("OFF".to_string())));
    }

    #[test]
    fn test_expose_has_publish_access() {
        let expose_with_access = Expose {
            expose_type: ExposeType::Numeric,
            property: Some("temperature".to_string()),
            name: None,
            unit: Some("°C".to_string()),
            access: Some(5), // bit 0 and bit 2 set
            category: None,
            description: None,
            features: None,
            value_on: None,
            value_off: None,
            value_min: None,
            value_max: None,
            value_step: None,
            values: None,
        };
        assert!(expose_with_access.has_publish_access());

        let expose_without_access = Expose {
            expose_type: ExposeType::Numeric,
            property: Some("temperature".to_string()),
            name: None,
            unit: None,
            access: Some(2), // only bit 1 set
            category: None,
            description: None,
            features: None,
            value_on: None,
            value_off: None,
            value_min: None,
            value_max: None,
            value_step: None,
            values: None,
        };
        assert!(!expose_without_access.has_publish_access());
    }

    #[test]
    fn test_flatten_exposes_simple() {
        let exposes = vec![
            Expose {
                expose_type: ExposeType::Numeric,
                property: Some("temperature".to_string()),
                name: None,
                unit: Some("°C".to_string()),
                access: Some(1),
                category: None,
                description: None,
                features: None,
                value_on: None,
                value_off: None,
                value_min: None,
                value_max: None,
                value_step: None,
                values: None,
            },
            Expose {
                expose_type: ExposeType::Binary,
                property: Some("state".to_string()),
                name: None,
                unit: None,
                access: Some(1),
                category: None,
                description: None,
                features: None,
                value_on: Some(BinaryValue::String("ON".to_string())),
                value_off: Some(BinaryValue::String("OFF".to_string())),
                value_min: None,
                value_max: None,
                value_step: None,
                values: None,
            },
        ];

        let flattened = flatten_exposes(&exposes);
        assert_eq!(flattened.len(), 2);
        assert_eq!(flattened[0].property, "temperature");
        assert_eq!(flattened[1].property, "state");
    }

    #[test]
    fn test_flatten_exposes_composite() {
        let exposes = vec![Expose {
            expose_type: ExposeType::Composite,
            property: Some("color_xy".to_string()),
            name: None,
            unit: None,
            access: Some(7),
            category: None,
            description: None,
            features: Some(vec![
                Expose {
                    expose_type: ExposeType::Numeric,
                    property: Some("x".to_string()),
                    name: None,
                    unit: None,
                    access: Some(7),
                    category: None,
                    description: None,
                    features: None,
                    value_on: None,
                    value_off: None,
                    value_min: None,
                    value_max: None,
                    value_step: None,
                    values: None,
                },
                Expose {
                    expose_type: ExposeType::Numeric,
                    property: Some("y".to_string()),
                    name: None,
                    unit: None,
                    access: Some(7),
                    category: None,
                    description: None,
                    features: None,
                    value_on: None,
                    value_off: None,
                    value_min: None,
                    value_max: None,
                    value_step: None,
                    values: None,
                },
            ]),
            value_on: None,
            value_off: None,
            value_min: None,
            value_max: None,
            value_step: None,
            values: None,
        }];

        let flattened = flatten_exposes(&exposes);
        assert_eq!(flattened.len(), 2);
        assert_eq!(flattened[0].property, "color_xy_x");
        assert_eq!(flattened[1].property, "color_xy_y");
    }

    #[test]
    fn test_flatten_exposes_generic_with_features() {
        // Edge case: a generic expose that also has features
        // Both the parent and children should be included
        let exposes = vec![Expose {
            expose_type: ExposeType::Numeric,
            property: Some("brightness".to_string()),
            name: None,
            unit: Some("%".to_string()),
            access: Some(7),
            category: None,
            description: None,
            features: Some(vec![Expose {
                expose_type: ExposeType::Numeric,
                property: Some("transition".to_string()),
                name: None,
                unit: Some("s".to_string()),
                access: Some(7),
                category: None,
                description: None,
                features: None,
                value_on: None,
                value_off: None,
                value_min: None,
                value_max: None,
                value_step: None,
                values: None,
            }]),
            value_on: None,
            value_off: None,
            value_min: None,
            value_max: None,
            value_step: None,
            values: None,
        }];

        let flattened = flatten_exposes(&exposes);
        assert_eq!(flattened.len(), 2);
        assert_eq!(flattened[0].property, "brightness");
        assert_eq!(flattened[0].unit, Some("%".to_string()));
        assert_eq!(flattened[1].property, "brightness_transition");
        assert_eq!(flattened[1].unit, Some("s".to_string()));
    }

    #[test]
    fn test_flatten_exposes_name_fallback() {
        // When property is None, should fall back to name
        let exposes = vec![Expose {
            expose_type: ExposeType::Numeric,
            property: None,
            name: Some("temperature".to_string()),
            unit: Some("°C".to_string()),
            access: Some(1),
            category: None,
            description: None,
            features: None,
            value_on: None,
            value_off: None,
            value_min: None,
            value_max: None,
            value_step: None,
            values: None,
        }];

        let flattened = flatten_exposes(&exposes);
        assert_eq!(flattened.len(), 1);
        assert_eq!(flattened[0].property, "temperature");
    }

    #[test]
    fn test_flatten_exposes_unknown_fallback() {
        // When both property and name are None, should use "unknown"
        let exposes = vec![Expose {
            expose_type: ExposeType::Numeric,
            property: None,
            name: None,
            unit: Some("°C".to_string()),
            access: Some(1),
            category: None,
            description: None,
            features: None,
            value_on: None,
            value_off: None,
            value_min: None,
            value_max: None,
            value_step: None,
            values: None,
        }];

        let flattened = flatten_exposes(&exposes);
        assert_eq!(flattened.len(), 1);
        assert_eq!(flattened[0].property, "unknown");
    }

    #[test]
    fn test_flatten_exposes_nested_name_fallback() {
        // Nested expose with name fallback should include parent prefix
        let exposes = vec![Expose {
            expose_type: ExposeType::Composite,
            property: Some("sensor".to_string()),
            name: None,
            unit: None,
            access: Some(7),
            category: None,
            description: None,
            features: Some(vec![Expose {
                expose_type: ExposeType::Numeric,
                property: None,
                name: Some("reading".to_string()),
                unit: None,
                access: Some(1),
                category: None,
                description: None,
                features: None,
                value_on: None,
                value_off: None,
                value_min: None,
                value_max: None,
                value_step: None,
                values: None,
            }]),
            value_on: None,
            value_off: None,
            value_min: None,
            value_max: None,
            value_step: None,
            values: None,
        }];

        let flattened = flatten_exposes(&exposes);
        assert_eq!(flattened.len(), 1);
        assert_eq!(flattened[0].property, "sensor_reading");
    }
}
