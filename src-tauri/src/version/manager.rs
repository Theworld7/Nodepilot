use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::activator::VersionActivator;
use super::deleter::VersionDeleter;
use super::error::VersionManagerError;
use super::event::{EventSink, VersionEvent};
use super::fetcher::VersionFetcher;
use super::installer::VersionInstaller;
use super::types::NodeVersion;
use crate::client::HttpClient;
use crate::fs::FileSystem;

pub enum VersionCommand {
    Get,
    Refresh,
    Install { version: String },
    Activate { version: String },
    Delete { version: String },
}

pub struct ExecuteOutput {
    pub versions: Vec<NodeVersion>,
    pub events: Vec<VersionEvent>,
}

struct VersionUrls {
    index_url: String,
    dist_url: String,
}

impl VersionUrls {
    fn new(index_url: String) -> Self {
        let dist_url = derive_dist_url(&index_url);
        Self {
            index_url,
            dist_url,
        }
    }

    fn set_index_url(&mut self, url: String) {
        self.dist_url = derive_dist_url(&url);
        self.index_url = url;
    }
}

fn derive_dist_url(index_url: &str) -> String {
    if index_url.ends_with('/') || index_url.contains(".json") {
        let trimmed = index_url
            .trim_end_matches("index.json")
            .trim_end_matches('/');
        format!("{}/", trimmed)
    } else {
        format!("{}/", index_url)
    }
}

pub struct VersionManager {
    fetcher: VersionFetcher,
    installer: VersionInstaller,
    activator: VersionActivator,
    deleter: VersionDeleter,
    urls: Mutex<VersionUrls>,
}

impl VersionManager {
    pub fn new(
        nodepilot_dir: PathBuf,
        versions_dir: PathBuf,
        cache_dir: PathBuf,
        http_client: Arc<dyn HttpClient>,
        fs: Arc<dyn FileSystem>,
        source_url: String,
    ) -> Self {
        Self {
            fetcher: VersionFetcher::new(cache_dir, http_client.clone(), fs.clone()),
            installer: VersionInstaller::new(
                versions_dir.clone(),
                http_client.clone(),
                fs.clone(),
            ),
            activator: VersionActivator::new(
                nodepilot_dir.clone(),
                versions_dir.clone(),
                fs.clone(),
            ),
            deleter: VersionDeleter::new(versions_dir, nodepilot_dir, fs),
            urls: Mutex::new(VersionUrls::new(source_url)),
        }
    }

    pub fn source_url(&self) -> String {
        self.urls.lock().unwrap().index_url.clone()
    }

    pub fn set_source_url(&self, url: String) {
        self.urls.lock().unwrap().set_index_url(url);
    }

    pub fn get_current_version(&self) -> Option<String> {
        self.activator.get_current_version()
    }

    fn enrich(&self, versions: &mut [NodeVersion]) {
        let installed = self
            .activator
            .get_installed_versions()
            .unwrap_or_default();
        let current = self.activator.get_current_version();

        for v in versions.iter_mut() {
            v.installed = Some(installed.contains(&v.version));
            v.active = Some(current.as_deref() == Some(&v.version));
        }
    }

    pub async fn execute(
        &self,
        cmd: VersionCommand,
        sink: &mut dyn EventSink,
    ) -> Result<ExecuteOutput, VersionManagerError> {
        let (index_url, dist_url) = {
            let urls = self.urls.lock().unwrap();
            (urls.index_url.clone(), urls.dist_url.clone())
        };

        match cmd {
            VersionCommand::Get => {
                let mut versions = self
                    .fetcher
                    .fetch_or_cache(&index_url)
                    .await?;
                self.enrich(&mut versions);
                Ok(ExecuteOutput {
                    versions,
                    events: vec![],
                })
            }

            VersionCommand::Refresh => {
                let mut versions = self
                    .fetcher
                    .refresh(&index_url)
                    .await?;
                self.enrich(&mut versions);
                let event = VersionEvent::VersionsUpdated(versions.clone());
                Ok(ExecuteOutput {
                    versions,
                    events: vec![event],
                })
            }

            VersionCommand::Install { version } => {
                self.installer
                    .install(&version, &dist_url, sink)
                    .await?;

                let mut versions = self
                    .fetcher
                    .fetch_or_cache(&index_url)
                    .await?;
                self.enrich(&mut versions);
                let event = VersionEvent::VersionsUpdated(versions.clone());
                Ok(ExecuteOutput {
                    versions,
                    events: vec![event],
                })
            }

            VersionCommand::Activate { version } => {
                self.activator.activate(&version)?;

                let mut versions = self
                    .fetcher
                    .fetch_or_cache(&index_url)
                    .await?;
                self.enrich(&mut versions);

                let versions_clone = versions.clone();
                Ok(ExecuteOutput {
                    versions,
                    events: vec![
                        VersionEvent::VersionActivated {
                            version: version.clone(),
                        },
                        VersionEvent::VersionsUpdated(versions_clone),
                    ],
                })
            }

            VersionCommand::Delete { version } => {
                self.deleter.delete(&version)?;

                let mut versions = self
                    .fetcher
                    .fetch_or_cache(&index_url)
                    .await?;
                self.enrich(&mut versions);
                let event = VersionEvent::VersionsUpdated(versions.clone());
                Ok(ExecuteOutput {
                    versions,
                    events: vec![event],
                })
            }
        }
    }
}
