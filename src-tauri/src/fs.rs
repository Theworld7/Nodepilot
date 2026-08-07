use std::path::{Path, PathBuf};

#[allow(dead_code)]
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
                match std::os::windows::fs::symlink_dir(target, link) {
                    Ok(()) => {}
                    Err(e) => {
                        if e.raw_os_error() == Some(1314) || link.exists() {
                            windows_junction_set(link, target)?;
                        } else {
                            return Err(e);
                        }
                    }
                }
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

// ---------------------------------------------------------------------------
// Windows NTFS Junction helpers (raw FFI — zero dependency)
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod junction {
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    // Win32 constants
    const DELETE: u32 = 0x00010000;
    const GENERIC_WRITE: u32 = 0x40000000;
    const FILE_SHARE_READ: u32 = 1;
    const FILE_SHARE_WRITE: u32 = 2;
    const FILE_SHARE_DELETE: u32 = 4;
    const OPEN_EXISTING: u32 = 3;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x00200000;

    const FSCTL_DELETE_REPARSE_POINT: u32 = 0x0009_00AC;
    const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xA000_0003;

    type HANDLE = isize;

    extern "system" {
        fn CreateFileW(
            lpFileName: *const u16,
            dwDesiredAccess: u32,
            dwShareMode: u32,
            lpSecurityAttributes: *const std::ffi::c_void,
            dwCreationDisposition: u32,
            dwFlagsAndAttributes: u32,
            hTemplateFile: HANDLE,
        ) -> HANDLE;

        fn DeviceIoControl(
            hDevice: HANDLE,
            dwIoControlCode: u32,
            lpInBuffer: *const std::ffi::c_void,
            nInBufferSize: u32,
            lpOutBuffer: *mut std::ffi::c_void,
            nOutBufferSize: u32,
            lpBytesReturned: *mut u32,
            lpOverlapped: *mut std::ffi::c_void,
        ) -> i32;

        fn CloseHandle(hObject: HANDLE) -> i32;

        fn DeleteFileW(lpFileName: *const u16) -> i32;

        fn RemoveDirectoryW(lpPathName: *const u16) -> i32;
    }

    /// Set (or update) an NTFS Junction at `link` pointing to `target`.
    ///
    /// Works regardless of what already exists at `link` — symlink,
    /// Junction, regular directory, or nothing.
    pub fn set(link: &Path, target: &Path) -> io::Result<()> {
        // Resolve target to absolute path.
        let _target_abs = std::fs::canonicalize(target)?;

        let link_wide: Vec<u16> = link
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // ── Phase 1: strip any existing reparse point and remove ─────
        if link.exists() {
            let h = unsafe {
                CreateFileW(
                    link_wide.as_ptr(),
                    DELETE | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                    0,
                )
            };
            if h != -1 {
                // Strip both mount-point and symlink reparse tags.
                let mut _ret: u32 = 0;
                unsafe {
                    let mut del = std::mem::zeroed::<DeleteReparseBuffer>();
                    del.ReparseTag = IO_REPARSE_TAG_MOUNT_POINT;
                    DeviceIoControl(
                        h, FSCTL_DELETE_REPARSE_POINT,
                        &del as *const _ as *const _,
                        std::mem::size_of::<DeleteReparseBuffer>() as u32,
                        std::ptr::null_mut(), 0, &mut _ret, std::ptr::null_mut(),
                    );
                    del.ReparseTag = 0xA000_000C; // IO_REPARSE_TAG_SYMLINK
                    DeviceIoControl(
                        h, FSCTL_DELETE_REPARSE_POINT,
                        &del as *const _ as *const _,
                        std::mem::size_of::<DeleteReparseBuffer>() as u32,
                        std::ptr::null_mut(), 0, &mut _ret, std::ptr::null_mut(),
                    );
                }
                unsafe { CloseHandle(h) };
            }
            // Remove whatever remains (file symlink → file, junction → directory).
            if link.is_dir() {
                unsafe { let _ = RemoveDirectoryW(link_wide.as_ptr()); }
            } else {
                unsafe { let _ = DeleteFileW(link_wide.as_ptr()); }
            }
        }

        if link.exists() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "无法删除旧的版本链接 ({})。\
                     请关闭正在使用 Node.js 的终端后重试。",
                    link.display(),
                ),
            ));
        }

        // ── Phase 2: create fresh junction via mklink /J ──────────────
        let mut cmd = std::process::Command::new("cmd");
        cmd.args(["/c", "mklink", "/J"]).arg(link).arg(target);
        // 隐藏 mklink 弹出的命令窗口
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let result = cmd.output();

        match result {
            Ok(out) if out.status.success() => Ok(()),
            Ok(out) => {
                let msg = String::from_utf8_lossy(&out.stderr);
                log::error!("[junction] mklink /J failed: {}", msg.trim());
                Err(io::Error::new(io::ErrorKind::Other, msg.trim().to_string()))
            }
            Err(e) => {
                log::error!("[junction] mklink spawn failed: {e}");
                Err(e)
            }
        }
    }

    #[repr(C)]
    #[allow(non_snake_case)]
    struct DeleteReparseBuffer {
        ReparseTag: u32,
        ReparseDataLength: u16,
        Reserved: u16,
    }
}

use junction::set as windows_junction_set;

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
