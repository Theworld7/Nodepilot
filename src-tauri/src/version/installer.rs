use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use super::error::VersionManagerError;
use super::event::{EventSink, VersionEvent};
use crate::client::HttpClient;
use crate::fs::FileSystem;

pub struct VersionInstaller {
    http_client: Arc<dyn HttpClient>,
    fs: Arc<dyn FileSystem>,
    versions_dir: PathBuf,
}

fn throttled_emit(sink: &dyn EventSink, event: VersionEvent, last_emit: &mut Instant) {
    let now = Instant::now();
    if now.duration_since(*last_emit).as_millis() < 150 {
        return;
    }
    *last_emit = now;
    sink.emit(event);
}

impl VersionInstaller {
    pub fn new(
        versions_dir: PathBuf,
        http_client: Arc<dyn HttpClient>,
        fs: Arc<dyn FileSystem>,
    ) -> Self {
        Self {
            http_client,
            fs,
            versions_dir,
        }
    }

    fn version_dir(&self, version: &str) -> PathBuf {
        self.versions_dir.join(version)
    }

    fn platform_prefix() -> &'static str {
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

    fn fallback_platform_prefix() -> Option<&'static str> {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Some("darwin-x64")
        } else if cfg!(all(target_os = "windows", target_arch = "aarch64")) {
            Some("win-x64")
        } else {
            None
        }
    }

    fn archive_ext() -> &'static str {
        if cfg!(target_os = "windows") {
            "zip"
        } else {
            "tar.gz"
        }
    }

    fn archive_filename_with_platform(version: &str, platform: &str) -> String {
        format!("node-{version}-{platform}.{}", Self::archive_ext())
    }

    fn download_url(source_url: &str, version: &str, platform: &str) -> String {
        format!(
            "{}/{}/{}",
            source_url.trim_end_matches('/'),
            version,
            Self::archive_filename_with_platform(version, platform),
        )
    }

    pub async fn install(
        &self,
        version: &str,
        source_url: &str,
        sink: &dyn EventSink,
    ) -> Result<(), VersionManagerError> {
        let version_dir = self.version_dir(version);
        if self.fs.exists(&version_dir) {
            return Err(VersionManagerError::AlreadyInstalled(version.to_string()));
        }

        sink.emit(VersionEvent::InstallProgress {
            version: version.to_string(),
            stage: "downloading".to_string(),
            percent: 0.0,
        });

        let archive_bytes = self.download_archive(version, source_url, sink).await?;

        self.fs
            .create_dir_all(&version_dir)
            .map_err(VersionManagerError::Io)?;

        sink.emit(VersionEvent::InstallProgress {
            version: version.to_string(),
            stage: "extracting".to_string(),
            percent: 0.0,
        });

        if cfg!(target_os = "windows") {
            self.extract_zip(&archive_bytes, version, sink)?;
        } else {
            self.extract_tar_gz(&archive_bytes, version, sink)?;
        }

        sink.emit(VersionEvent::InstallProgress {
            version: version.to_string(),
            stage: "done".to_string(),
            percent: 100.0,
        });

        Ok(())
    }

    async fn download_archive(
        &self,
        version: &str,
        source_url: &str,
        sink: &dyn EventSink,
    ) -> Result<Vec<u8>, VersionManagerError> {
        let url = Self::download_url(source_url, version, Self::platform_prefix());
        let result = self.do_streaming_download(&url, version, sink).await;

        match result {
            Ok(bytes) => Ok(bytes),
            Err(err) => {
                let is_404 = matches!(&err, VersionManagerError::Network(msg) if msg.contains("404"));
                if is_404 {
                    if let Some(fallback) = Self::fallback_platform_prefix() {
                        let fallback_url =
                            Self::download_url(source_url, version, fallback);
                        return self.do_streaming_download(&fallback_url, version, sink).await;
                    }
                }
                Err(err)
            }
        }
    }

    async fn do_streaming_download(
        &self,
        url: &str,
        version: &str,
        sink: &dyn EventSink,
    ) -> Result<Vec<u8>, VersionManagerError> {
        let version_owned = version.to_string();
        let mut last_emit = Instant::now();

        throttled_emit(
            sink,
            VersionEvent::InstallProgress {
                version: version_owned.clone(),
                stage: "downloading".to_string(),
                percent: 0.0,
            },
            &mut last_emit,
        );

        let report_progress = |received: u64, total: u64| {
            let pct = if total > 0 {
                (received as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            sink.emit(VersionEvent::InstallProgress {
                version: version_owned.clone(),
                stage: "downloading".to_string(),
                percent: pct,
            });
        };

        let response = self
            .http_client
            .get_streamed(url, &report_progress)
            .await
            .map_err(|e| VersionManagerError::Network(format!("download failed: {e}")))?;

        throttled_emit(
            sink,
            VersionEvent::InstallProgress {
                version: version_owned,
                stage: "downloading".to_string(),
                percent: 100.0,
            },
            &mut last_emit,
        );

        Ok(response.data)
    }

    fn extract_tar_gz(
        &self,
        data: &[u8],
        version: &str,
        sink: &dyn EventSink,
    ) -> Result<(), VersionManagerError> {
        use flate2::read::GzDecoder;
        use tar::Archive;

        let version_dir = self.version_dir(version);
        let decoder = GzDecoder::new(data);
        let mut archive = Archive::new(decoder);

        let total = {
            let decoder2 = GzDecoder::new(data);
            let mut a2 = Archive::new(decoder2);
            a2.entries()
                .map_err(|e| VersionManagerError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
                .count()
                .max(1)
        };

        let mut processed: usize = 0;
        let mut last_emit = Instant::now();

        for entry in archive
            .entries()
            .map_err(|e| VersionManagerError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
        {
            let mut entry = entry.map_err(|e| {
                VersionManagerError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
            })?;

            let path = entry.path().map_err(|e| {
                VersionManagerError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
            })?;
            let stripped: PathBuf = path.components().skip(1).collect();

            if stripped.as_os_str().is_empty() {
                processed += 1;
                continue;
            }

            let target = version_dir.join(&stripped);

            if entry.header().entry_type().is_dir() {
                self.fs
                    .create_dir_all(&target)
                    .map_err(VersionManagerError::Io)?;
            } else {
                if let Some(parent) = target.parent() {
                    self.fs
                        .create_dir_all(parent)
                        .map_err(VersionManagerError::Io)?;
                }
                entry.unpack(&target).map_err(|e| {
                    VersionManagerError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
                })?;
            }

            processed += 1;
            let pct = (processed as f64 / total as f64) * 100.0;
            throttled_emit(
                sink,
                VersionEvent::InstallProgress {
                    version: version.to_string(),
                    stage: "extracting".to_string(),
                    percent: pct,
                },
                &mut last_emit,
            );
        }

        Ok(())
    }

    fn extract_zip(
        &self,
        data: &[u8],
        version: &str,
        sink: &dyn EventSink,
    ) -> Result<(), VersionManagerError> {
        use zip::ZipArchive;

        let reader = std::io::Cursor::new(data);
        let mut archive =
            ZipArchive::new(reader).map_err(|e| {
                VersionManagerError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
            })?;

        let version_dir = self.version_dir(version);
        let total = archive.len().max(1);
        let mut last_emit = Instant::now();

        for i in 0..total {
            let mut entry = archive.by_index(i).map_err(|e| {
                VersionManagerError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
            })?;

            let path = entry.name().to_string();
            let stripped: PathBuf = path.split('/').skip(1).collect();

            if stripped.as_os_str().is_empty() {
                continue;
            }

            let target = version_dir.join(&stripped);
            if entry.is_dir() {
                self.fs
                    .create_dir_all(&target)
                    .map_err(VersionManagerError::Io)?;
            } else {
                if let Some(parent) = target.parent() {
                    self.fs
                        .create_dir_all(parent)
                        .map_err(VersionManagerError::Io)?;
                }
                let mut out = std::fs::File::create(&target)
                    .map_err(VersionManagerError::Io)?;
                std::io::copy(&mut entry, &mut out).map_err(|e| {
                    VersionManagerError::Io(
                        std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
                    )
                })?;
            }

            let pct = ((i as f64 + 1.0) / total as f64) * 100.0;
            throttled_emit(
                sink,
                VersionEvent::InstallProgress {
                    version: version.to_string(),
                    stage: "extracting".to_string(),
                    percent: pct,
                },
                &mut last_emit,
            );
        }

        Ok(())
    }
}
