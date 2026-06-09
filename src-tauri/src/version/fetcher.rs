use std::path::PathBuf;
use std::sync::Arc;

use super::error::VersionManagerError;
use super::types::{NodeVersion, RemoteVersion};
use crate::client::HttpClient;
use crate::fs::FileSystem;

const CACHE_FILENAME: &str = "versions.json";

pub struct VersionFetcher {
    http_client: Arc<dyn HttpClient>,
    fs: Arc<dyn FileSystem>,
    cache_dir: PathBuf,
}

impl VersionFetcher {
    pub fn new(
        cache_dir: PathBuf,
        http_client: Arc<dyn HttpClient>,
        fs: Arc<dyn FileSystem>,
    ) -> Self {
        Self {
            http_client,
            fs,
            cache_dir,
        }
    }

    fn cache_path(&self) -> PathBuf {
        self.cache_dir.join(CACHE_FILENAME)
    }

    pub async fn fetch_remote(&self, source_url: &str) -> Result<Vec<RemoteVersion>, VersionManagerError> {
        let resp = self.http_client.get(source_url).await.map_err(|e| {
            VersionManagerError::Network(format!("failed to fetch version list: {e}"))
        })?;

        let versions: Vec<RemoteVersion> =
            serde_json::from_slice(&resp.data).map_err(|e| {
                VersionManagerError::Parse(format!("failed to parse version list: {e}"))
            })?;

        Ok(versions)
    }

    fn read_cache(&self) -> Result<Vec<NodeVersion>, VersionManagerError> {
        let data = self
            .fs
            .read_to_string(&self.cache_path())
            .map_err(|e| VersionManagerError::Io(e))?;
        let versions: Vec<NodeVersion> = serde_json::from_str(&data).map_err(|e| {
            VersionManagerError::Parse(format!("failed to parse cache: {e}"))
        })?;
        Ok(versions)
    }

    fn write_cache(&self, versions: &[NodeVersion]) -> Result<(), VersionManagerError> {
        let data =
            serde_json::to_string_pretty(versions).map_err(|e| {
                VersionManagerError::Parse(format!("failed to serialize cache: {e}"))
            })?;
        self.fs
            .write(&self.cache_path(), data.as_bytes())
            .map_err(VersionManagerError::Io)
    }

    pub async fn fetch_or_cache(
        &self,
        source_url: &str,
    ) -> Result<Vec<NodeVersion>, VersionManagerError> {
        match self.fetch_remote(source_url).await {
            Ok(remote) => {
                let versions: Vec<NodeVersion> = remote.into_iter().map(Into::into).collect();
                if let Err(e) = self.write_cache(&versions) {
                    log::warn!("Failed to write cache: {e}");
                }
                Ok(versions)
            }
            Err(e) => {
                log::warn!("Remote fetch failed ({e}), falling back to cache");
                self.read_cache()
            }
        }
    }

    pub async fn refresh(
        &self,
        source_url: &str,
    ) -> Result<Vec<NodeVersion>, VersionManagerError> {
        let remote = self.fetch_remote(source_url).await?;
        let versions: Vec<NodeVersion> = remote.into_iter().map(Into::into).collect();
        self.write_cache(&versions)?;
        Ok(versions)
    }
}
