mod audio;
mod config;
mod output;
mod rate_limit;
mod server;
mod state;
mod transcription;
mod vad;

use anyhow::Result;
use server::DaemonServer;
use state::DaemonState;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, level_filters::LevelFilter, warn};
use tracing_subscriber::EnvFilter;

/// Get the Unix socket path for the daemon.
/// Uses XDG runtime directory if available, falls back to /tmp/ndictd.sock
fn get_socket_path() -> PathBuf {
    if let Some(runtime_dir) = dirs::runtime_dir() {
        let path = runtime_dir.join("ndictd.sock");
        info!("Using XDG runtime directory: {}", path.display());
        path
    } else {
        warn!("XDG runtime directory not found, using fallback: /tmp/ndictd.sock");
        PathBuf::from("/tmp/ndictd.sock")
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::INFO)
        .with_target(false)
        .with_env_filter(EnvFilter::from_default_env().add_directive(LevelFilter::INFO.into()))
        .init();

    info!("ndict daemon (ndictd) starting...");

    let config = config::load_config()?;
    let daemon_state = DaemonState::new(config);
    let state = Arc::new(Mutex::new(daemon_state));

    let socket_path = get_socket_path();
    let server = DaemonServer::new(socket_path, state);
    server.run().await?;

    Ok(())
}
