mod audio;
mod output;
mod server;
mod state;
mod transcription;
mod vad;

use anyhow::Result;
use server::DaemonServer;
use state::DaemonState;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::INFO)
        .with_target(false)
        .with_env_filter(EnvFilter::from_default_env().add_directive(LevelFilter::INFO.into()))
        .init();

    info!("ndict daemon (ndictd) starting...");

    let daemon_state = DaemonState::new();
    let state = Arc::new(Mutex::new(daemon_state));

    let server = DaemonServer::new("/tmp/ndictd.sock".into(), state);
    server.run().await?;

    Ok(())
}
