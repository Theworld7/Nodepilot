use std::path::PathBuf;

use reqwest::Client;
use serde_json;

use super::types::{NodeVersion, RemoteVersion};

const DEFAULT_SOURCE_URL: &str = "https://nodejs.org/dist/index.json";
const CACHE_FILENAME: &str = "versions.json";

pub struct VersionFetcher {
    http_client: Client,
    source_url: String,
    cache_dir: PathBuf,
}

impl VersionFetcher {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            http_client: Client::builder()
                .user_agent("nodepilot/0.1.0")
                .build()
                .expect("Failed to create HTTP client"),
            source_url: DEFAULT_SOURCE_URL.to_string(),
            cache_dir,
        }
    }

    pub fn set_source_url(&mut self, url: String) {
        self.source_url = url;
    }

    pub fn cache_path(&self) -> PathBuf {
        self.cache_dir.join(CACHE_FILENAME)
    }

    pub async fn fetch_remote(&self) -> Result<Vec<RemoteVersion>, FetchError> {
        let resp = self
            .http_client
            .get(&self.source_url)
            .send()
            .await
            .map_err(|e| FetchError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(FetchError::Http(format!("HTTP {}", resp.status())));
        }

        let versions: Vec<RemoteVersion> = resp
            .json()
            .await
            .map_err(|e| FetchError::Parse(e.to_string()))?;

        Ok(versions)
    }

    pub fn read_cache(&self) -> Result<Vec<NodeVersion>, FetchError> {
        let data =
            std::fs::read_to_string(self.cache_path()).map_err(|e| FetchError::Cache(e.to_string()))?;
        let versions: Vec<NodeVersion> =
            serde_json::from_str(&data).map_err(|e| FetchError::Parse(e.to_string()))?;
        Ok(versions)
    }

    pub fn write_cache(&self, versions: &[NodeVersion]) -> Result<(), FetchError> {
        if let Some(parent) = self.cache_path().parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| FetchError::Cache(e.to_string()))?;
        }
        let data =
            serde_json::to_string_pretty(versions).map_err(|e| FetchError::Cache(e.to_string()))?;
        std::fs::write(self.cache_path(), data)
            .map_err(|e| FetchError::Cache(e.to_string()))?;
        Ok(())
    }

    pub async fn fetch_or_cache(&self) -> Result<Vec<NodeVersion>, FetchError> {
        match self.fetch_remote().await {
            Ok(remote) => {
                let versions: Vec<NodeVersion> = remote.into_iter().map(Into::into).collect();
                if let Err(e) = self.write_cache(&versions) {
                    log::warn!("Failed to write cache: {}", e);
                }
                Ok(versions)
            }
            Err(e) => {
                log::warn!("Remote fetch failed ({}), falling back to cache", e);
                self.read_cache()
            }
        }
    }

    pub async fn refresh(&self) -> Result<Vec<NodeVersion>, FetchError> {
        let remote = self.fetch_remote().await?;
        let versions: Vec<NodeVersion> = remote.into_iter().map(Into::into).collect();
        self.write_cache(&versions)?;
        Ok(versions)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Cache error: {0}")]
    Cache(String),
}
