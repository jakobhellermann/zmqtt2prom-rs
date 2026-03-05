# zmqtt2prom-rs

Rust port of [zmqtt2prom](https://github.com/yellowstonesoftware/zmqtt2prom), mostly because it was annoying to deploy Swift to NixOS.

## Usage

```
Bridge between Zigbee2MQTT and Prometheus metrics

Usage: zmqtt2prom-rs [OPTIONS]

Options:
      --mqtt-host <MQTT_HOST>          MQTT broker hostname [env: Z2P_MQTT_HOST=] [default: localhost]
      --mqtt-port <MQTT_PORT>          MQTT broker port [env: Z2P_MQTT_PORT=] [default: 1883]
      --mqtt-username <MQTT_USERNAME>  MQTT username [env: Z2P_MQTT_USERNAME=]
      --mqtt-password <MQTT_PASSWORD>  MQTT password [env: Z2P_MQTT_PASSWORD=]
      --http-port <HTTP_PORT>          HTTP server port for metrics endpoint [env: Z2P_HTTP_PORT=] [default: 6565]
      --log-level <LOG_LEVEL>          Log level (trace, debug, info, warn, error) [default: info]
  -h, --help                           Print help (see more with '--help')
  -V, --version                        Print version
```

## Prometheus

`zmqtt2prom-rs` creates metrics with the following labels:

- `friendly_name`
- `manufacturer`
- `ieee_address`
- `network_address`
- `model_id`
- `type`
- `property`
- `unit`
