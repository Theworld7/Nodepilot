use std::path::PathBuf;

pub struct VersionDeleter {
    versions_dir: PathBuf,
    current_symlink: PathBuf,
}

impl VersionDeleter {
    pub fn new(versions_dir: PathBuf, nodepilot_dir: PathBuf) -> Self {
        Self {
            versions_dir,
            current_symlink: nodepilot_dir.join("current"),
        }
    }

    pub fn delete(&self, version: &str) -> Result<(), DeleteError> {
        let version_dir = self.versions_dir.join(version);
        if !version_dir.exists() {
            return Err(DeleteError::NotFound(version.to_string()));
        }

        if let Ok(target) = std::fs::read_link(&self.current_symlink) {
            if target == version_dir {
                return Err(DeleteError::Active(version.to_string()));
            }
        }

        std::fs::remove_dir_all(&version_dir)
            .map_err(|e| DeleteError::Io(e.to_string()))?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DeleteError {
    #[error("Version not found: {0}")]
    NotFound(String),
    #[error("Cannot delete active version: {0}")]
    Active(String),
    #[error("IO error: {0}")]
    Io(String),
}
