use serde::Serialize;

use crate::version::error::VersionManagerError;

#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    Network(String),
    Parse(String),
    AlreadyInstalled(String),
    NotFound(String),
    Active(String),
    Io(String),
    Config(String),
    Setup(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Network(msg) => write!(f, "network error: {msg}"),
            AppError::Parse(msg) => write!(f, "parse error: {msg}"),
            AppError::AlreadyInstalled(v) => write!(f, "version already installed: {v}"),
            AppError::NotFound(v) => write!(f, "version not found: {v}"),
            AppError::Active(v) => write!(f, "cannot delete active version: {v}"),
            AppError::Io(msg) => write!(f, "I/O error: {msg}"),
            AppError::Config(msg) => write!(f, "config error: {msg}"),
            AppError::Setup(msg) => write!(f, "setup error: {msg}"),
        }
    }
}

impl From<VersionManagerError> for AppError {
    fn from(e: VersionManagerError) -> Self {
        match e {
            VersionManagerError::Network(msg) => AppError::Network(msg),
            VersionManagerError::Parse(msg) => AppError::Parse(msg),
            VersionManagerError::AlreadyInstalled(v) => AppError::AlreadyInstalled(v),
            VersionManagerError::NotFound(v) => AppError::NotFound(v),
            VersionManagerError::Active(v) => AppError::Active(v),
            VersionManagerError::Io(e) => AppError::Io(e.to_string()),
        }
    }
}
