use std::fs;
use std::path::Path;

#[derive(Debug, Copy, Clone)]
pub struct DiskUsage {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

pub fn dir_size_bytes(path: &Path) -> Option<u64> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if metadata.is_file() {
        return Some(metadata.len());
    }
    if !metadata.is_dir() {
        return Some(0);
    }

    let mut total: u64 = 0;
    let mut stack = vec![path.to_path_buf()];

    while let Some(current) = stack.pop() {
        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let entry_path = entry.path();
            let meta = match fs::symlink_metadata(&entry_path) {
                Ok(meta) => meta,
                Err(_) => continue,
            };

            if meta.file_type().is_symlink() {
                continue;
            }

            if meta.is_file() {
                total = total.saturating_add(meta.len());
            } else if meta.is_dir() {
                stack.push(entry_path);
            }
        }
    }

    Some(total)
}

#[cfg(unix)]
pub fn filesystem_usage(path: &Path) -> Option<DiskUsage> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let result = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if result != 0 {
        return None;
    }

    let block_size = if stat.f_frsize > 0 {
        stat.f_frsize as u64
    } else {
        stat.f_bsize as u64
    };
    let total_bytes = block_size.saturating_mul(stat.f_blocks as u64);
    let available_bytes = block_size.saturating_mul(stat.f_bavail as u64);

    Some(DiskUsage {
        total_bytes,
        available_bytes,
    })
}

#[cfg(not(unix))]
pub fn filesystem_usage(_path: &Path) -> Option<DiskUsage> {
    None
}
