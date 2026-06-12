#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use crate::client::{HttpClientMock, HttpResponse};
    use crate::fs::mock::FsMock;
    use crate::fs::FileSystem;
    use crate::version::event::{EventSink, VersionEvent};
    use crate::version::types::NodeVersion;
    use crate::version::{
        activator::VersionActivator,
        deleter::VersionDeleter,
        fetcher::VersionFetcher,
        installer::VersionInstaller,
        VersionCommand, VersionManager,
    };

    struct TestSink;
    impl EventSink for TestSink {
        fn emit(&self, _event: VersionEvent) {}
    }

    fn make_versions_json() -> Vec<u8> {
        serde_json::to_vec(&vec![
            serde_json::json!({
                "version": "v20.0.0",
                "date": "2024-04-23",
                "files": ["darwin-arm64", "darwin-x64", "linux-x64"],
                "lts": false,
                "npm": "10.5.0",
                "v8": "12.3.0",
                "uv": "1.44.0",
                "zlib": "1.3.0",
                "openssl": "3.0.13",
                "modules": "115",
                "security": false
            }),
            serde_json::json!({
                "version": "v22.0.0",
                "date": "2025-04-30",
                "files": ["darwin-arm64", "darwin-x64", "linux-x64"],
                "lts": "Iron",
                "npm": "11.0.0",
                "v8": "12.8.0",
                "uv": "1.48.0",
                "zlib": "1.3.1",
                "openssl": "3.4.0",
                "modules": "124",
                "security": false
            }),
        ])
        .unwrap()
    }

    // --- Fetcher tests ---

    #[tokio::test]
    async fn test_fetch_remote_parses_versions() {
        let http = Arc::new(HttpClientMock::new());
        let fs = Arc::new(FsMock::new());
        let data = make_versions_json();
        http.expect(
            "https://nodejs.org/dist/index.json",
            Ok(HttpResponse {
                data,
                content_length: None,
            }),
        );

        let fetcher = VersionFetcher::new(PathBuf::from("/cache"), http, fs);
        let versions = fetcher.fetch_remote("https://nodejs.org/dist/index.json").await.unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].version, "v20.0.0");
        assert_eq!(versions[1].version, "v22.0.0");
        assert!(versions[1].lts.is_lts());
    }

    #[tokio::test]
    async fn test_fetch_remote_network_error() {
        let http = Arc::new(HttpClientMock::new());
        let fs = Arc::new(FsMock::new());
        http.expect(
            "https://nodejs.org/dist/index.json",
            Err(crate::client::HttpClientError::Connection("timeout".to_string())),
        );

        let fetcher = VersionFetcher::new(PathBuf::from("/cache"), http, fs);
        let result = fetcher.fetch_remote("https://nodejs.org/dist/index.json").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fetch_or_cache_falls_back_to_cache() {
        let http = Arc::new(HttpClientMock::new());
        let fs = Arc::new(FsMock::new());
        let cache_path = PathBuf::from("/cache");

        http.expect(
            "https://nodejs.org/dist/index.json",
            Err(crate::client::HttpClientError::Connection("timeout".to_string())),
        );

        let cached_data = serde_json::to_vec(&vec![
            NodeVersion {
                version: "v20.0.0".to_string(),
                date: "2024-04-23".to_string(),
                lts: false,
                lts_codename: None,
                files: vec!["darwin-arm64".to_string()],
                installed: None,
                active: None,
            },
        ])
        .unwrap();
        fs.write(&cache_path.join("versions.json"), &cached_data).unwrap();

        let fetcher = VersionFetcher::new(cache_path, http, fs);
        let versions = fetcher.fetch_or_cache("https://nodejs.org/dist/index.json").await.unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version, "v20.0.0");
    }

    #[tokio::test]
    async fn test_refresh_fetches_and_caches() {
        let http = Arc::new(HttpClientMock::new());
        let fs = Arc::new(FsMock::new());
        let cache_path = PathBuf::from("/cache");

        let data = make_versions_json();
        http.expect(
            "https://nodejs.org/dist/index.json",
            Ok(HttpResponse {
                data,
                content_length: None,
            }),
        );

        let fetcher = VersionFetcher::new(cache_path.clone(), http, fs.clone());
        let versions = fetcher.refresh("https://nodejs.org/dist/index.json").await.unwrap();
        assert_eq!(versions.len(), 2);

        let cached = fs.read_to_string(&cache_path.join("versions.json")).unwrap();
        assert!(cached.contains("v20.0.0"));
    }

    // --- Activator tests ---

    #[test]
    fn test_get_installed_versions_empty() {
        let fs = Arc::new(FsMock::new());
        let activator = VersionActivator::new(
            PathBuf::from("/nodepilot"),
            PathBuf::from("/nodepilot/versions"),
            fs,
        );
        let versions = activator.get_installed_versions().unwrap();
        assert!(versions.is_empty());
    }

    #[test]
    fn test_get_installed_versions_with_versions() {
        let fs = Arc::new(FsMock::new());
        fs.create_dir_all(&PathBuf::from("/nodepilot/versions/v20.0.0")).unwrap();
        fs.create_dir_all(&PathBuf::from("/nodepilot/versions/v22.0.0")).unwrap();

        let activator = VersionActivator::new(
            PathBuf::from("/nodepilot"),
            PathBuf::from("/nodepilot/versions"),
            fs,
        );
        let mut versions = activator.get_installed_versions().unwrap();
        versions.sort();
        assert_eq!(versions, vec!["v20.0.0", "v22.0.0"]);
    }

    #[test]
    fn test_activate_and_get_current() {
        let fs = Arc::new(FsMock::new());
        fs.create_dir_all(&PathBuf::from("/nodepilot/versions/v20.0.0")).unwrap();

        let activator = VersionActivator::new(
            PathBuf::from("/nodepilot"),
            PathBuf::from("/nodepilot/versions"),
            fs,
        );
        activator.activate("v20.0.0").unwrap();
        assert_eq!(activator.get_current_version(), Some("v20.0.0".to_string()));
    }

    #[test]
    fn test_activate_not_found() {
        let fs = Arc::new(FsMock::new());
        let activator = VersionActivator::new(
            PathBuf::from("/nodepilot"),
            PathBuf::from("/nodepilot/versions"),
            fs,
        );
        let err = activator.activate("v99.0.0").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    // --- Deleter tests ---

    #[test]
    fn test_delete_version() {
        let fs = Arc::new(FsMock::new());
        fs.create_dir_all(&PathBuf::from("/nodepilot/versions/v20.0.0")).unwrap();

        let deleter = VersionDeleter::new(
            PathBuf::from("/nodepilot/versions"),
            PathBuf::from("/nodepilot"),
            fs.clone(),
        );
        deleter.delete("v20.0.0").unwrap();
        assert!(!fs.exists(&PathBuf::from("/nodepilot/versions/v20.0.0")));
    }

    #[test]
    fn test_delete_active_version_errors() {
        let fs = Arc::new(FsMock::new());
        fs.create_dir_all(&PathBuf::from("/nodepilot/versions/v20.0.0")).unwrap();
        fs.symlink(
            &PathBuf::from("/nodepilot/versions/v20.0.0"),
            &PathBuf::from("/nodepilot/current"),
        )
        .unwrap();

        let deleter = VersionDeleter::new(
            PathBuf::from("/nodepilot/versions"),
            PathBuf::from("/nodepilot"),
            fs,
        );
        let err = deleter.delete("v20.0.0").unwrap_err();
        assert!(err.to_string().contains("active"));
    }

    #[test]
    fn test_delete_not_found() {
        let fs = Arc::new(FsMock::new());
        let deleter = VersionDeleter::new(
            PathBuf::from("/nodepilot/versions"),
            PathBuf::from("/nodepilot"),
            fs,
        );
        let err = deleter.delete("v99.0.0").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    // --- Installer tests ---

    #[tokio::test]
    async fn test_install_already_installed() {
        let http = Arc::new(HttpClientMock::new());
        let fs = Arc::new(FsMock::new());
        fs.create_dir_all(&PathBuf::from("/versions/v20.0.0")).unwrap();

        let installer = VersionInstaller::new(PathBuf::from("/versions"), http, fs);
        let err = installer
            .install("v20.0.0", "https://nodejs.org/dist/v20.0.0", &mut TestSink)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("already installed"));
    }

    #[tokio::test]
    async fn test_install_download_error() {
        let http = Arc::new(HttpClientMock::new());
        let fs = Arc::new(FsMock::new());

        http.expect(
            "https://nodejs.org/dist/v20.0.0/node-v20.0.0-darwin-arm64.tar.gz",
            Err(crate::client::HttpClientError::Connection("timeout".to_string())),
        );

        let installer = VersionInstaller::new(PathBuf::from("/versions"), http, fs);
        let err = installer
            .install("v20.0.0", "https://nodejs.org/dist/v20.0.0", &mut TestSink)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("download failed"));
    }

    // --- VersionManager integration tests ---

    #[tokio::test]
    async fn test_manager_get_versions() {
        let http = Arc::new(HttpClientMock::new());
        let fs = Arc::new(FsMock::new());
        let nodepilot_dir = PathBuf::from("/nodepilot");

        let data = make_versions_json();
        http.expect(
            "https://nodejs.org/dist/index.json",
            Ok(HttpResponse {
                data,
                content_length: None,
            }),
        );

        let manager = VersionManager::new(
            nodepilot_dir.clone(),
            nodepilot_dir.join("versions"),
            nodepilot_dir.join("cache"),
            http,
            fs,
            "https://nodejs.org/dist/index.json".to_string(),
        );

        let output = manager
            .execute(VersionCommand::Get, &mut TestSink)
            .await
            .unwrap();
        assert_eq!(output.versions.len(), 2);
        assert_eq!(output.versions[0].version, "v20.0.0");
    }

    #[tokio::test]
    async fn test_manager_refresh() {
        let http = Arc::new(HttpClientMock::new());
        let fs = Arc::new(FsMock::new());
        let nodepilot_dir = PathBuf::from("/nodepilot");

        let data = make_versions_json();
        http.expect(
            "https://nodejs.org/dist/index.json",
            Ok(HttpResponse {
                data,
                content_length: None,
            }),
        );

        let manager = VersionManager::new(
            nodepilot_dir.clone(),
            nodepilot_dir.join("versions"),
            nodepilot_dir.join("cache"),
            http,
            fs.clone(),
            "https://nodejs.org/dist/index.json".to_string(),
        );

        let output = manager
            .execute(VersionCommand::Refresh, &mut TestSink)
            .await
            .unwrap();
        assert_eq!(output.versions.len(), 2);
        assert_eq!(output.events.len(), 1);

        let cached = fs
            .read_to_string(&nodepilot_dir.join("cache").join("versions.json"))
            .unwrap();
        assert!(cached.contains("v20.0.0"));
    }

    #[tokio::test]
    async fn test_manager_set_source_url() {
        let http = Arc::new(HttpClientMock::new());
        let fs = Arc::new(FsMock::new());
        let nodepilot_dir = PathBuf::from("/nodepilot");

        let manager = VersionManager::new(
            nodepilot_dir.clone(),
            nodepilot_dir.join("versions"),
            nodepilot_dir.join("cache"),
            http,
            fs,
            "https://nodejs.org/dist/index.json".to_string(),
        );

        assert_eq!(
            manager.source_url(),
            "https://nodejs.org/dist/index.json"
        );

        manager.set_source_url("https://mirror.example.com/index.json".to_string());
        assert_eq!(
            manager.source_url(),
            "https://mirror.example.com/index.json"
        );
    }
}
