use std::path::PathBuf;

pub struct VersionActivator {
    nodepilot_dir: PathBuf,
    versions_dir: PathBuf,
}

impl VersionActivator {
    pub fn new(nodepilot_dir: PathBuf, versions_dir: PathBuf) -> Self {
        Self {
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
        if current.exists() {
            if let Ok(target) = std::fs::read_link(&current) {
                if let Some(name) = target.file_name() {
                    return Some(name.to_string_lossy().to_string());
                }
            }
        }
        None
    }

    pub fn get_installed_versions(&self) -> Result<Vec<String>, ActivateError> {
        if !self.versions_dir.exists() {
            return Ok(vec![]);
        }
        let mut versions = vec![];
        for entry in std::fs::read_dir(&self.versions_dir)
            .map_err(|e| ActivateError::Io(e.to_string()))?
        {
            let entry = entry.map_err(|e| ActivateError::Io(e.to_string()))?;
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if let Some(name) = entry.file_name().to_str() {
                    versions.push(name.to_string());
                }
            }
        }
        Ok(versions)
    }

    pub fn activate(&self, version: &str) -> Result<(), ActivateError> {
        let version_dir = self.version_dir(version);
        if !version_dir.exists() {
            return Err(ActivateError::NotFound(version.to_string()));
        }

        let current = self.current_symlink();

        // Use symlink_metadata to detect dangling symlinks (exists() returns
        // false for broken links, but the symlink file itself is still there).
        if let Ok(meta) = std::fs::symlink_metadata(&current) {
            if meta.file_type().is_symlink() {
                std::fs::remove_file(&current)
                    .map_err(|e| ActivateError::Io(e.to_string()))?;
            } else {
                return Err(ActivateError::Io("current is not a symlink".to_string()));
            }
        }

        #[cfg(unix)]
        std::os::unix::fs::symlink(&version_dir, &current)
            .map_err(|e| ActivateError::Io(e.to_string()))?;

        #[cfg(windows)]
        {
            if std::os::windows::fs::symlink_dir(&version_dir, &current).is_err() {
                std::os::windows::fs::symlink_dir(&version_dir, &current)
                    .map_err(|e| ActivateError::Io(e.to_string()))?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ActivateError {
    #[error("Version not found: {0}")]
    NotFound(String),
    #[error("IO error: {0}")]
    Io(String),
}
