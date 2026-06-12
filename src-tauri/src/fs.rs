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

#[cfg(test)]
pub mod mock {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;

    pub struct FsMock {
        files: Mutex<HashMap<PathBuf, Vec<u8>>>,
        symlinks: Mutex<HashMap<PathBuf, PathBuf>>,
        dirs: Mutex<Vec<PathBuf>>,
    }

    impl FsMock {
        pub fn new() -> Self {
            Self {
                files: Mutex::new(HashMap::new()),
                symlinks: Mutex::new(HashMap::new()),
                dirs: Mutex::new(vec![]),
            }
        }

        fn ensure_parents(&self, path: &Path, dirs: &mut Vec<PathBuf>) {
            if let Some(parent) = path.parent() {
                if !dirs.contains(&parent.to_path_buf()) {
                    self.ensure_parents(parent, dirs);
                    dirs.push(parent.to_path_buf());
                }
            }
        }
    }

    impl FileSystem for FsMock {
        fn create_dir_all(&self, path: &Path) -> Result<(), std::io::Error> {
            let mut dirs = self.dirs.lock().unwrap();
            if !dirs.contains(&path.to_path_buf()) {
                self.ensure_parents(path, &mut dirs);
                dirs.push(path.to_path_buf());
            }
            Ok(())
        }

        fn write(&self, path: &Path, data: &[u8]) -> Result<(), std::io::Error> {
            self.create_dir_all(path.parent().unwrap_or(path))?;
            self.files
                .lock()
                .unwrap()
                .insert(path.to_path_buf(), data.to_vec());
            Ok(())
        }

        fn read_to_string(&self, path: &Path) -> Result<String, std::io::Error> {
            let files = self.files.lock().unwrap();
            files.get(path).map(|d| {
                String::from_utf8(d.clone()).map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, e)
                })
            }).unwrap_or_else(|| Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("file not found: {}", path.display()),
            )))
        }

        fn remove_dir_all(&self, path: &Path) -> Result<(), std::io::Error> {
            let mut files = self.files.lock().unwrap();
            let mut symlinks = self.symlinks.lock().unwrap();
            let mut dirs = self.dirs.lock().unwrap();

            files.retain(|p, _| !p.starts_with(path));
            symlinks.retain(|p, _| !p.starts_with(path));
            dirs.retain(|d| !d.starts_with(path));
            Ok(())
        }

        fn remove_file(&self, path: &Path) -> Result<(), std::io::Error> {
            self.files.lock().unwrap().remove(path);
            self.symlinks.lock().unwrap().remove(path);
            Ok(())
        }

        fn symlink_metadata(&self, path: &Path) -> Result<std::fs::Metadata, std::io::Error> {
            let symlinks = self.symlinks.lock().unwrap();
            if symlinks.contains_key(path) {
                Ok(std::fs::File::open(path).and_then(|_| std::fs::metadata(path)).unwrap_or_else(|_| {
                    std::fs::metadata(".").unwrap()
                }))
            } else {
                Err(std::io::Error::new(std::io::ErrorKind::NotFound, "not a symlink"))
            }
        }

        fn read_link(&self, path: &Path) -> Result<PathBuf, std::io::Error> {
            let symlinks = self.symlinks.lock().unwrap();
            symlinks.get(path).cloned().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "symlink not found")
            })
        }

        fn symlink(&self, target: &Path, link: &Path) -> Result<(), std::io::Error> {
            self.symlinks
                .lock()
                .unwrap()
                .insert(link.to_path_buf(), target.to_path_buf());
            Ok(())
        }

        fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
            let files = self.files.lock().unwrap();
            let dirs = self.dirs.lock().unwrap();
            let mut entries = Vec::new();

            for p in files.keys() {
                if p.parent() == Some(path) {
                    entries.push(p.clone());
                }
            }
            for d in dirs.iter() {
                if d.parent() == Some(path) {
                    entries.push(d.clone());
                }
            }
            entries.sort();
            entries.dedup();
            Ok(entries)
        }

        fn exists(&self, path: &Path) -> bool {
            let files = self.files.lock().unwrap();
            let symlinks = self.symlinks.lock().unwrap();
            let dirs = self.dirs.lock().unwrap();
            files.contains_key(path) || symlinks.contains_key(path) || dirs.iter().any(|d| d == path)
        }

        fn is_dir(&self, path: &Path) -> bool {
            let dirs = self.dirs.lock().unwrap();
            dirs.contains(&path.to_path_buf())
                || dirs.iter().any(|d| d.starts_with(path))
        }
    }
}
