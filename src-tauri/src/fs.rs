use std::path::{Path, PathBuf};

pub trait FileSystem: Send + Sync {
    fn create_dir_all(&self, path: &Path) -> Result<(), std::io::Error>;
    fn write(&self, path: &Path, data: &[u8]) -> Result<(), std::io::Error>;
    fn read_to_string(&self, path: &Path) -> Result<String, std::io::Error>;
    fn remove_dir_all(&self, path: &Path) -> Result<(), std::io::Error>;
    fn remove_file(&self, path: &Path) -> Result<(), std::io::Error>;
    fn symlink_metadata(&self, path: &Path) -> Result<std::fs::Metadata, std::io::Error>;
    fn read_link(&self, path: &Path) -> Result<PathBuf, std::io::Error>;
    fn symlink(&self, target: &Path, link: &Path) -> Result<(), std::io::Error>;
    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, std::io::Error>;
    fn exists(&self, path: &Path) -> bool;
    fn is_dir(&self, path: &Path) -> bool;
}

pub struct FsProd;

impl FileSystem for FsProd {
    fn create_dir_all(&self, path: &Path) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(path)
    }

    fn write(&self, path: &Path, data: &[u8]) -> Result<(), std::io::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, data)
    }

    fn read_to_string(&self, path: &Path) -> Result<String, std::io::Error> {
        std::fs::read_to_string(path)
    }

    fn remove_dir_all(&self, path: &Path) -> Result<(), std::io::Error> {
        std::fs::remove_dir_all(path)
    }

    fn remove_file(&self, path: &Path) -> Result<(), std::io::Error> {
        std::fs::remove_file(path)
    }

    fn symlink_metadata(&self, path: &Path) -> Result<std::fs::Metadata, std::io::Error> {
        std::fs::symlink_metadata(path)
    }

    fn read_link(&self, path: &Path) -> Result<PathBuf, std::io::Error> {
        std::fs::read_link(path)
    }

    fn symlink(&self, target: &Path, link: &Path) -> Result<(), std::io::Error> {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link)?;
        }
        #[cfg(windows)]
        {
            if target.is_dir() {
                std::os::windows::fs::symlink_dir(target, link)?;
            } else {
                std::os::windows::fs::symlink_file(target, link)?;
            }
        }
        Ok(())
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path)? {
            entries.push(entry?.path());
        }
        Ok(entries)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }
}
