use std::path::PathBuf;
use std::sync::Arc;

use super::error::VersionManagerError;
use crate::fs::FileSystem;

pub struct VersionDeleter {
    fs: Arc<dyn FileSystem>,
    versions_dir: PathBuf,
    current_symlink: PathBuf,
}

impl VersionDeleter {
    pub fn new(
        versions_dir: PathBuf,
        nodepilot_dir: PathBuf,
        fs: Arc<dyn FileSystem>,
    ) -> Self {
        Self {
            fs,
            versions_dir,
            current_symlink: nodepilot_dir.join("current"),
        }
    }

    pub fn delete(&self, version: &str) -> Result<(), VersionManagerError> {
        let version_dir = self.versions_dir.join(version);
        if !self.fs.exists(&version_dir) {
            return Err(VersionManagerError::NotFound(version.to_string()));
        }

        if let Ok(target) = self.fs.read_link(&self.current_symlink) {
            if target == version_dir {
                return Err(VersionManagerError::Active(version.to_string()));
            }
        }

        self.fs.remove_dir_all(&version_dir)?;

        Ok(())
    }
}
