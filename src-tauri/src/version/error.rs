#[derive(Debug, thiserror::Error)]
pub enum VersionManagerError {
    #[error("network error: {0}")]
    Network(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("version already installed: {0}")]
    AlreadyInstalled(String),
    #[error("version not found: {0}")]
    NotFound(String),
    #[error("cannot delete active version: {0}")]
    Active(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
