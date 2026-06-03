use std::path::PathBuf;
use std::time::Instant;

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

/// Emit progress at most once per 150ms to avoid flooding the event bus.
fn throttled_emit(app: &AppHandle, progress: &InstallProgress, last_emit: &mut Instant) {
    let now = Instant::now();
    if now.duration_since(*last_emit).as_millis() < 150 {
        return;
    }
    *last_emit = now;
    let _ = app.emit("install_progress", progress);
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

        let archive_bytes = self.download_archive(version, app).await?;

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

    async fn download_archive(
        &self,
        version: &str,
        app: &AppHandle,
    ) -> Result<Vec<u8>, InstallError> {
        // Try primary platform first
        let url = self.download_url(version);
        let result = self.do_streaming_download(&url, version, app).await;

        match result {
            Ok(bytes) => return Ok(bytes),
            Err(err) => {
                // If 404 and fallback available, try fallback platform
                if let InstallError::Download(ref msg) = err {
                    if msg.starts_with("HTTP 404") || msg.starts_with("http: status 404") || msg.contains("404") {
                        if let Some(fallback) = self.fallback_platform_prefix() {
                            let fallback_url = self.download_url_with_prefix(version, fallback);
                            return self.do_streaming_download(&fallback_url, version, app).await;
                        }
                    }
                }
                return Err(err);
            }
        }
    }

    /// Stream the response body chunk by chunk, emitting progress after each chunk.
    async fn do_streaming_download(
        &self,
        url: &str,
        version: &str,
        app: &AppHandle,
    ) -> Result<Vec<u8>, InstallError> {
        let resp = self
            .http_client
            .get(url)
            .send()
            .await
            .map_err(|e| InstallError::Download(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(InstallError::Download(format!(
                "HTTP {}",
                resp.status()
            )));
        }

        // Determine total size from Content-Length header
        let total = resp
            .content_length()
            .unwrap_or(0);

        let mut collected = Vec::with_capacity(total as usize);
        let mut received: u64 = 0;
        let mut last_emit = Instant::now();

        let mut stream = resp;
        while let Some(chunk) = stream
            .chunk()
            .await
            .map_err(|e| InstallError::Download(e.to_string()))?
        {
            received += chunk.len() as u64;
            collected.extend_from_slice(&chunk);

            if total > 0 {
                let pct = (received as f64 / total as f64) * 100.0;
                let progress = InstallProgress {
                    version: version.to_string(),
                    stage: "downloading".to_string(),
                    percent: pct,
                };
                throttled_emit(app, &progress, &mut last_emit);
            }
        }

        Ok(collected)
    }

    fn extract_tar_gz(
        &self,
        data: &[u8],
        version: &str,
        app: &AppHandle,
    ) -> Result<(), InstallError> {
        use flate2::read::GzDecoder;
        use tar::Archive;

        let version_dir = self.version_dir(version);
        let decoder = GzDecoder::new(data);
        let mut archive = Archive::new(decoder);

        // First pass: count total entries for progress tracking
        let total = {
            let decoder2 = GzDecoder::new(data);
            let mut a2 = Archive::new(decoder2);
            a2.entries()
                .map_err(|e| InstallError::Extract(e.to_string()))?
                .count()
                .max(1)
        };

        let mut last_emit = Instant::now();
        let mut processed: usize = 0;

        for entry in archive
            .entries()
            .map_err(|e| InstallError::Extract(e.to_string()))?
        {
            let mut entry = entry.map_err(|e| InstallError::Extract(e.to_string()))?;

            // Strip the top-level directory (e.g. "node-v20.11.0-darwin-arm64/")
            let path = entry.path().map_err(|e| InstallError::Extract(e.to_string()))?;
            let stripped: PathBuf = path.components().skip(1).collect();

            if stripped.as_os_str().is_empty() {
                processed += 1;
                continue;
            }

            let target = version_dir.join(&stripped);

            if entry
                .header()
                .entry_type()
                .is_dir()
            {
                std::fs::create_dir_all(&target)
                    .map_err(|e| InstallError::Io(e.to_string()))?;
            } else {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| InstallError::Io(e.to_string()))?;
                }
                entry
                    .unpack(&target)
                    .map_err(|e| InstallError::Extract(e.to_string()))?;
            }

            processed += 1;
            let pct = (processed as f64 / total as f64) * 100.0;
            let progress = InstallProgress {
                version: version.to_string(),
                stage: "extracting".to_string(),
                percent: pct,
            };
            throttled_emit(app, &progress, &mut last_emit);
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
        let mut last_emit = Instant::now();

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
                let pct = ((i as f64 + 1.0) / total as f64) * 100.0;
                let progress = InstallProgress {
                    version: version.to_string(),
                    stage: "extracting".to_string(),
                    percent: pct,
                };
                throttled_emit(app, &progress, &mut last_emit);
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
