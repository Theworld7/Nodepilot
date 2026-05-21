use std::path::PathBuf;

use reqwest::Client;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstallProgress {
    pub version: String,
    pub stage: String,
    pub percent: f64,
}

pub struct VersionInstaller {
    http_client: Client,
    source_url: String,
    versions_dir: PathBuf,
}

impl VersionInstaller {
    pub fn new(versions_dir: PathBuf) -> Self {
        Self {
            http_client: Client::builder()
                .user_agent("nodepilot/0.1.0")
                .build()
                .expect("Failed to create HTTP client"),
            source_url: "https://nodejs.org/dist".to_string(),
            versions_dir,
        }
    }

    pub fn set_source_url(&mut self, url: String) {
        self.source_url = url;
    }

    fn platform_prefix(&self) -> &'static str {
        if cfg!(target_os = "macos") {
            if cfg!(target_arch = "aarch64") {
                "darwin-arm64"
            } else {
                "darwin-x64"
            }
        } else if cfg!(target_os = "windows") {
            if cfg!(target_arch = "aarch64") {
                "win-arm64"
            } else {
                "win-x64"
            }
        } else {
            "linux-x64"
        }
    }

    /// Fallback platform suffix when the primary one returns 404.
    /// On Apple Silicon, fall back to darwin-x64 (runs via Rosetta 2).
    fn fallback_platform_prefix(&self) -> Option<&'static str> {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Some("darwin-x64")
        } else if cfg!(all(target_os = "windows", target_arch = "aarch64")) {
            Some("win-x64")
        } else {
            None
        }
    }

    fn archive_ext(&self) -> &'static str {
        if cfg!(target_os = "windows") {
            "zip"
        } else {
            "tar.gz"
        }
    }

    fn download_url(&self, version: &str) -> String {
        self.download_url_with_prefix(version, self.platform_prefix())
    }

    fn download_url_with_prefix(&self, version: &str, platform: &str) -> String {
        format!(
            "{}/{}/{}",
            self.source_url.trim_end_matches('/'),
            version,
            self.archive_filename_with_platform(version, platform),
        )
    }

    fn archive_filename_with_platform(&self, version: &str, platform: &str) -> String {
        format!("node-{}-{}.{}", version, platform, self.archive_ext())
    }

    fn version_dir(&self, version: &str) -> PathBuf {
        self.versions_dir.join(version)
    }

    pub async fn install(
        &self,
        version: &str,
        app: &AppHandle,
    ) -> Result<(), InstallError> {
        let version_dir = self.version_dir(version);
        if version_dir.exists() {
            return Err(InstallError::AlreadyInstalled(version.to_string()));
        }

        self.emit_progress(app, version, "downloading", 0.0);

        let archive_bytes = self.download_archive(version).await?;

        std::fs::create_dir_all(&version_dir)
            .map_err(|e| InstallError::Io(e.to_string()))?;

        self.emit_progress(app, version, "extracting", 0.0);

        if cfg!(target_os = "windows") {
            self.extract_zip(&archive_bytes, version, app)?;
        } else {
            self.extract_tar_gz(&archive_bytes, version, app)?;
        }

        self.emit_progress(app, version, "done", 100.0);
        Ok(())
    }

    async fn download_archive(&self, version: &str) -> Result<Vec<u8>, InstallError> {
        // Try primary platform first
        let url = self.download_url(version);
        let resp = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| InstallError::Download(e.to_string()))?;

        if resp.status().is_success() {
            return resp
                .bytes()
                .await
                .map(|b| b.to_vec())
                .map_err(|e| InstallError::Download(e.to_string()));
        }

        // If 404 and fallback available, try fallback platform
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            if let Some(fallback) = self.fallback_platform_prefix() {
                let fallback_url = self.download_url_with_prefix(version, fallback);
                let resp = self
                    .http_client
                    .get(&fallback_url)
                    .send()
                    .await
                    .map_err(|e| InstallError::Download(e.to_string()))?;

                if resp.status().is_success() {
                    return resp
                        .bytes()
                        .await
                        .map(|b| b.to_vec())
                        .map_err(|e| InstallError::Download(e.to_string()));
                }

                return Err(InstallError::Download(format!(
                    "HTTP {} (tried: {}, {})",
                    resp.status(),
                    url,
                    fallback_url
                )));
            }
        }

        Err(InstallError::Download(format!("HTTP {}", resp.status())))
    }

    fn extract_tar_gz(
        &self,
        data: &[u8],
        version: &str,
        _app: &AppHandle,
    ) -> Result<(), InstallError> {
        use std::io::Write;

        let version_dir = self.version_dir(version);

        // Write archive to temp file, then use system tar to extract
        let tmp = std::env::temp_dir().join(format!("nodepilot-{}.tar.gz", version));
        {
            let mut f = std::fs::File::create(&tmp)
                .map_err(|e| InstallError::Io(e.to_string()))?;
            f.write_all(data)
                .map_err(|e| InstallError::Io(e.to_string()))?;
        }

        let output = std::process::Command::new("tar")
            .arg("-xzf")
            .arg(&tmp)
            .arg("-C")
            .arg(&version_dir)
            .arg("--strip-components=1")
            .output()
            .map_err(|e| InstallError::Extract(format!("tar command failed: {}", e)))?;

        // Clean up temp file
        let _ = std::fs::remove_file(&tmp);

        if !output.status.success() {
            return Err(InstallError::Extract(format!(
                "tar exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr),
            )));
        }

        Ok(())
    }

    fn extract_zip(
        &self,
        data: &[u8],
        version: &str,
        app: &AppHandle,
    ) -> Result<(), InstallError> {
        use zip::ZipArchive;

        let reader = std::io::Cursor::new(data);
        let mut archive =
            ZipArchive::new(reader).map_err(|e| InstallError::Extract(e.to_string()))?;

        let version_dir = self.version_dir(version);
        let total = archive.len();

        for i in 0..total {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| InstallError::Extract(e.to_string()))?;

            let path = entry.name().to_string();
            let stripped: PathBuf = path.split('/').skip(1).collect();

            if stripped.as_os_str().is_empty() {
                continue;
            }

            let target = version_dir.join(&stripped);
            if entry.is_dir() {
                std::fs::create_dir_all(&target)
                    .map_err(|e| InstallError::Io(e.to_string()))?;
            } else {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| InstallError::Io(e.to_string()))?;
                }
                let mut out = std::fs::File::create(&target)
                    .map_err(|e| InstallError::Io(e.to_string()))?;
                std::io::copy(&mut entry, &mut out)
                    .map_err(|e| InstallError::Extract(e.to_string()))?;
            }

            if total > 0 {
                self.emit_progress(
                    app,
                    version,
                    "extracting",
                    ((i as f64 + 1.0) / total as f64) * 100.0,
                );
            }
        }

        Ok(())
    }

    fn emit_progress(&self, app: &AppHandle, version: &str, stage: &str, percent: f64) {
        let progress = InstallProgress {
            version: version.to_string(),
            stage: stage.to_string(),
            percent,
        };
        let _ = app.emit("install_progress", progress);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("Version already installed: {0}")]
    AlreadyInstalled(String),
    #[error("Download error: {0}")]
    Download(String),
    #[error("Extraction error: {0}")]
    Extract(String),
    #[error("IO error: {0}")]
    Io(String),
}
