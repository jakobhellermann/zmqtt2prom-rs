use std::ops::RangeInclusive;

use clap::Parser;

const PORT_RANGE: RangeInclusive<usize> = 1..=65535;

fn parse_port(s: &str) -> Result<u16, String> {
    let port: usize = s
        .parse()
        .map_err(|_| format!("`{s}` isn't a valid port number"))?;
    if PORT_RANGE.contains(&port) {
        Ok(port as u16)
    } else {
        Err(format!(
            "port not in range {}-{}",
            PORT_RANGE.start(),
            PORT_RANGE.end()
        ))
    }
}

fn parse_log_level(s: &str) -> Result<tracing::Level, String> {
    match s.to_lowercase().as_str() {
        "trace" => Ok(tracing::Level::TRACE),
        "debug" => Ok(tracing::Level::DEBUG),
        "info" => Ok(tracing::Level::INFO),
        "warn" | "warning" => Ok(tracing::Level::WARN),
        "error" => Ok(tracing::Level::ERROR),
        _ => Err(format!(
            "Invalid log level: {}. Valid values: trace, debug, info, warn, error",
            s
        )),
    }
}

/// Bridge between Zigbee2MQTT and Prometheus metrics.
///
/// Connects to an MQTT broker running Zigbee2MQTT, discovers devices,
/// and exposes their metrics via a Prometheus-compatible HTTP endpoint.
#[derive(Parser, Debug)]
#[command(name = "zmqtt2prom")]
#[command(version)]
#[command(about = "Bridge between Zigbee2MQTT and Prometheus metrics")]
pub struct Args {
    /// MQTT broker hostname
    #[arg(long, env = "Z2P_MQTT_HOST", default_value = "localhost")]
    pub mqtt_host: String,

    /// MQTT broker port
    #[arg(long, env = "Z2P_MQTT_PORT", default_value = "1883", value_parser = parse_port)]
    pub mqtt_port: u16,

    /// MQTT username
    #[arg(long, env = "Z2P_MQTT_USERNAME")]
    pub mqtt_username: Option<String>,

    /// MQTT password
    #[arg(long, env = "Z2P_MQTT_PASSWORD")]
    pub mqtt_password: Option<String>,

    /// HTTP server port for metrics endpoint
    #[arg(long, env = "Z2P_HTTP_PORT", default_value = "6565", value_parser = parse_port)]
    pub http_port: u16,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info", value_parser = parse_log_level)]
    pub log_level: tracing::Level,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_port_valid() {
        assert_eq!(parse_port("1883").unwrap(), 1883);
        assert_eq!(parse_port("1").unwrap(), 1);
        assert_eq!(parse_port("65535").unwrap(), 65535);
    }

    #[test]
    fn test_parse_port_invalid() {
        assert!(parse_port("0").is_err());
        assert!(parse_port("65536").is_err());
        assert!(parse_port("abc").is_err());
        assert!(parse_port("-1").is_err());
    }

    #[test]
    fn test_parse_log_level() {
        assert_eq!(parse_log_level("trace").unwrap(), tracing::Level::TRACE);
        assert_eq!(parse_log_level("DEBUG").unwrap(), tracing::Level::DEBUG);
        assert_eq!(parse_log_level("Info").unwrap(), tracing::Level::INFO);
        assert_eq!(parse_log_level("warn").unwrap(), tracing::Level::WARN);
        assert_eq!(parse_log_level("warning").unwrap(), tracing::Level::WARN);
        assert_eq!(parse_log_level("error").unwrap(), tracing::Level::ERROR);
        assert!(parse_log_level("invalid").is_err());
    }

    #[test]
    fn test_default_args() {
        let args = Args::parse_from(["zmqtt2prom"]);
        assert_eq!(args.mqtt_host, "localhost");
        assert_eq!(args.mqtt_port, 1883);
        assert_eq!(args.http_port, 6565);
        assert!(args.mqtt_username.is_none());
        assert!(args.mqtt_password.is_none());
    }

    #[test]
    fn test_custom_args() {
        let args = Args::parse_from([
            "zmqtt2prom",
            "--mqtt-host",
            "mqtt.example.com",
            "--mqtt-port",
            "8883",
            "--mqtt-username",
            "user",
            "--mqtt-password",
            "pass",
            "--http-port",
            "9090",
            "--log-level",
            "debug",
        ]);
        assert_eq!(args.mqtt_host, "mqtt.example.com");
        assert_eq!(args.mqtt_port, 8883);
        assert_eq!(args.mqtt_username.as_deref(), Some("user"));
        assert_eq!(args.mqtt_password.as_deref(), Some("pass"));
        assert_eq!(args.http_port, 9090);
        assert_eq!(args.log_level, tracing::Level::DEBUG);
    }
}
