use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{Router, extract::State, http::StatusCode, routing::get};
use clap::Parser;
use tokio::sync::{RwLock, watch};
use tracing::info;
use tracing_subscriber::EnvFilter;

mod cli;
mod models;
mod services;

use cli::Args;
use services::{MetricsManager, MqttConfig, MqttService};

/// Application state shared between HTTP handlers.
struct AppState {
    metrics_manager: Arc<MetricsManager>,
}

/// GET /metrics - Return Prometheus metrics.
async fn metrics_handler(State(state): State<Arc<AppState>>) -> Result<String, StatusCode> {
    state
        .metrics_manager
        .render()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// GET /health - Health check endpoint.
async fn health_handler() -> &'static str {
    "OK"
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Initialize logging - RUST_LOG takes precedence over --log-level
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(args.log_level.as_str()));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    info!("Starting zmqtt2prom");

    // Create shared device registry
    let device_registry = Arc::new(RwLock::new(HashMap::new()));

    // Create metrics manager
    let metrics_manager = Arc::new(MetricsManager::new(device_registry.clone()));

    // Create MQTT config
    let mqtt_config = MqttConfig {
        host: args.mqtt_host,
        port: args.mqtt_port,
        username: args.mqtt_username,
        password: args.mqtt_password,
    };

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Create MQTT service
    let mqtt_service = MqttService::new(mqtt_config, device_registry, metrics_manager.clone());

    // Spawn MQTT service task
    let mqtt_shutdown_rx = shutdown_rx.clone();
    let mqtt_handle = tokio::spawn(async move {
        mqtt_service.run(mqtt_shutdown_rx).await;
    });

    // Create HTTP server
    let app_state = Arc::new(AppState { metrics_manager });

    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], args.http_port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    info!("HTTP server listening on {}", addr);

    // Spawn HTTP server with graceful shutdown
    let http_handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let mut rx = shutdown_rx;
                loop {
                    if *rx.borrow() {
                        break;
                    }
                    if rx.changed().await.is_err() {
                        break;
                    }
                }
            })
            .await
            .unwrap();
    });

    // Wait for shutdown signal
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received SIGINT, shutting down...");
        }
        _ = async {
            #[cfg(unix)]
            {
                let mut sigterm = tokio::signal::unix::signal(
                    tokio::signal::unix::SignalKind::terminate()
                ).unwrap();
                sigterm.recv().await;
            }
            #[cfg(not(unix))]
            {
                std::future::pending::<()>().await;
            }
        } => {
            info!("Received SIGTERM, shutting down...");
        }
    }

    // Signal shutdown to all tasks
    let _ = shutdown_tx.send(true);

    // Wait for tasks to complete
    let _ = tokio::join!(mqtt_handle, http_handle);

    info!("Shutdown complete");
}
