use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Command {
    Start,
    Stop,
    Pause,
    Resume,
    Status,
    SetLanguage(String),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Response {
    Ok,
    Error(String),
    Status(StatusInfo),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct StatusInfo {
    pub is_running: bool,
    pub is_active: bool,
    pub language: String,
}

#[derive(Error, Debug)]
pub enum IpcError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Connection refused: is ndictd running?")]
    ConnectionRefused,

    #[error("Connection timeout")]
    Timeout,
}
