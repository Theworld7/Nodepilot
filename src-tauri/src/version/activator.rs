use std::path::PathBuf;
use std::sync::Arc;

use super::error::VersionManagerError;
use crate::fs::FileSystem;

pub struct VersionActivator {
    fs: Arc<dyn FileSystem>,
    nodepilot_dir: PathBuf,
    versions_dir: PathBuf,
}

impl VersionActivator {
    pub fn new(
        nodepilot_dir: PathBuf,
        versions_dir: PathBuf,
        fs: Arc<dyn FileSystem>,
    ) -> Self {
        Self {
            fs,
            nodepilot_dir,
            versions_dir,
        }
    }

    fn current_symlink(&self) -> PathBuf {
        self.nodepilot_dir.join("current")
    }

    fn version_dir(&self, version: &str) -> PathBuf {
        self.versions_dir.join(version)
    }

    pub fn get_current_version(&self) -> Option<String> {
        let current = self.current_symlink();
        if self.fs.exists(&current) {
            if let Ok(target) = self.fs.read_link(&current) {
                if let Some(name) = target.file_name() {
                    return Some(name.to_string_lossy().to_string());
                }
            }
        }
        None
    }

    pub fn get_installed_versions(&self) -> Result<Vec<String>, VersionManagerError> {
        if !self.fs.exists(&self.versions_dir) {
            return Ok(vec![]);
        }
        let mut versions = vec![];
        for entry in self.fs.read_dir(&self.versions_dir)? {
                if self.fs.is_dir(&entry) {
                    if let Some(name) = entry.file_name().and_then(|s| s.to_str()) {
                        versions.push(name.to_string());
                    }
                }
        }
        Ok(versions)
    }

    pub fn activate(&self, version: &str) -> Result<(), VersionManagerError> {
        let version_dir = self.version_dir(version);
        if !self.fs.exists(&version_dir) {
            return Err(VersionManagerError::NotFound(version.to_string()));
        }

        let current = self.current_symlink();

        if let Ok(meta) = self.fs.symlink_metadata(&current) {
            if meta.file_type().is_symlink() {
                self.fs.remove_file(&current)?;
            } else {
                return Err(VersionManagerError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "current is not a symlink",
                )));
            }
        }

        self.fs.symlink(&version_dir, &current)?;

        Ok(())
    }
}
