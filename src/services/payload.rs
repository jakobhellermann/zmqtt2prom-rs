use serde_json::{Map, Value};

/// Flattens a nested JSON object into a single-level map with underscore-separated keys.
///
/// For example:
/// ```json
/// {"overload_protection": {"min_current": 0.5}}
/// ```
/// becomes:
/// ```json
/// {"overload_protection_min_current": 0.5}
/// ```
pub fn flatten_payload(value: &Value) -> Map<String, Value> {
    let mut result = Map::new();
    if let Value::Object(obj) = value {
        flatten_recursive(obj, "", &mut result);
    }
    result
}

fn flatten_recursive(obj: &Map<String, Value>, prefix: &str, result: &mut Map<String, Value>) {
    for (key, value) in obj {
        let new_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}_{}", prefix, key)
        };

        match value {
            Value::Object(nested) => {
                flatten_recursive(nested, &new_key, result);
            }
            _ => {
                result.insert(new_key, value.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_flatten_simple() {
        let input = json!({
            "temperature": 25.5,
            "humidity": 60
        });

        let result = flatten_payload(&input);
        assert_eq!(result.get("temperature"), Some(&json!(25.5)));
        assert_eq!(result.get("humidity"), Some(&json!(60)));
    }

    #[test]
    fn test_flatten_nested() {
        let input = json!({
            "overload_protection": {
                "min_current": 0.5,
                "max_current": 16.0
            }
        });

        let result = flatten_payload(&input);
        assert_eq!(
            result.get("overload_protection_min_current"),
            Some(&json!(0.5))
        );
        assert_eq!(
            result.get("overload_protection_max_current"),
            Some(&json!(16.0))
        );
    }

    #[test]
    fn test_flatten_deeply_nested() {
        let input = json!({
            "color": {
                "xy": {
                    "x": 0.3,
                    "y": 0.4
                }
            }
        });

        let result = flatten_payload(&input);
        assert_eq!(result.get("color_xy_x"), Some(&json!(0.3)));
        assert_eq!(result.get("color_xy_y"), Some(&json!(0.4)));
    }

    #[test]
    fn test_flatten_with_null() {
        let input = json!({
            "temperature": 25.5,
            "battery": null
        });

        let result = flatten_payload(&input);
        assert_eq!(result.get("temperature"), Some(&json!(25.5)));
        assert_eq!(result.get("battery"), Some(&Value::Null));
    }

    #[test]
    fn test_flatten_mixed() {
        let input = json!({
            "state": "ON",
            "brightness": 254,
            "color": {
                "x": 0.3,
                "y": 0.4
            }
        });

        let result = flatten_payload(&input);
        assert_eq!(result.get("state"), Some(&json!("ON")));
        assert_eq!(result.get("brightness"), Some(&json!(254)));
        assert_eq!(result.get("color_x"), Some(&json!(0.3)));
        assert_eq!(result.get("color_y"), Some(&json!(0.4)));
    }
}
