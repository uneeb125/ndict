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
    Toggle,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_command_serialization_start() {
        let cmd = Command::Start;
        let json = serde_json::to_string(&cmd).unwrap();
        assert_eq!(json, r#""Start""#);
    }

    #[test]
    fn test_command_serialization_set_language() {
        let cmd = Command::SetLanguage("en".to_string());
        let json = serde_json::to_string(&cmd).unwrap();
        assert_eq!(json, r#"{"SetLanguage":"en"}"#);
    }

    #[test]
    fn test_command_round_trip_all_variants() {
        let commands = vec![
            Command::Start,
            Command::Stop,
            Command::Pause,
            Command::Resume,
            Command::Status,
            Command::SetLanguage("test".to_string()),
            Command::Toggle,
        ];
        for cmd in commands {
            let json = serde_json::to_string(&cmd).unwrap();
            let deserialized: Command = serde_json::from_str(&json).unwrap();
            assert_eq!(cmd, deserialized);
        }
    }

    #[test]
    fn test_response_serialization_ok() {
        let resp = Response::Ok;
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, r#""Ok""#);
    }

    #[test]
    fn test_response_serialization_error() {
        let resp = Response::Error("test error".to_string());
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, r#"{"Error":"test error"}"#);
    }

    #[test]
    fn test_response_serialization_status() {
        let info = StatusInfo {
            is_running: true,
            is_active: false,
            language: "en".to_string(),
        };
        let resp = Response::Status(info.clone());
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(
            json,
            r#"{"Status":{"is_running":true,"is_active":false,"language":"en"}}"#
        );
    }

    #[test]
    fn test_response_round_trip_all_variants() {
        let responses = vec![
            Response::Ok,
            Response::Error("error".to_string()),
            Response::Status(StatusInfo {
                is_running: true,
                is_active: false,
                language: "test".to_string(),
            }),
        ];
        for resp in responses {
            let json = serde_json::to_string(&resp).unwrap();
            let deserialized: Response = serde_json::from_str(&json).unwrap();
            assert_eq!(resp, deserialized);
        }
    }

    #[test]
    fn test_status_info_serialization() {
        let info = StatusInfo {
            is_running: true,
            is_active: true,
            language: "en".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("is_running"));
        assert!(json.contains("is_active"));
        assert!(json.contains("language"));
    }

    #[test]
    fn test_status_info_all_states() {
        let combinations = vec![
            (true, true, "en"),
            (true, false, "en"),
            (false, true, "es"),
            (false, false, "fr"),
        ];
        for (running, active, lang) in combinations {
            let info = StatusInfo {
                is_running: running,
                is_active: active,
                language: lang.to_string(),
            };
            let json = serde_json::to_string(&info).unwrap();
            let deserialized: StatusInfo = serde_json::from_str(&json).unwrap();
            assert_eq!(info, deserialized);
        }
    }

    #[test]
    fn test_ipc_error_display_io() {
        let err = IpcError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));
        assert!(err.to_string().contains("IO error"));
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn test_ipc_error_display_serialization() {
        let err = IpcError::Serialization(
            serde_json::from_str::<serde_json::Value>("invalid").unwrap_err(),
        );
        assert!(err.to_string().contains("Serialization error"));
    }

    #[test]
    fn test_ipc_error_display_connection_refused() {
        let err = IpcError::ConnectionRefused;
        assert!(err.to_string().contains("Connection refused"));
    }

    #[test]
    fn test_ipc_error_display_timeout() {
        let err = IpcError::Timeout;
        assert!(err.to_string().contains("Connection timeout"));
    }
}
