use std::fs::{File, OpenOptions};
use std::path::PathBuf;

use fs2::FileExt;

use crate::config::get_project_path;
use crate::error::AtlasError;

/// A file-based lock for a project's index.
pub struct ProjectLock {
    _lock_file: File,
}

impl ProjectLock {
    /// Acquire an exclusive lock for the given project.
    pub fn acquire(org: &str, project: &str) -> Result<Self, AtlasError> {
        let lock_path = get_lock_path(org, project)?;

        // Ensure directory exists
        if let Some(parent) = lock_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)?;

        lock_file
            .lock_exclusive()
            .map_err(|e| AtlasError::Storage(format!("Failed to acquire lock: {}", e)))?;

        Ok(Self {
            _lock_file: lock_file,
        })
    }
}

impl Drop for ProjectLock {
    fn drop(&mut self) {
        // Lock is automatically released when file is closed
    }
}

fn get_lock_path(org: &str, project: &str) -> Result<PathBuf, AtlasError> {
    Ok(get_project_path(org, project)?.join(".lock"))
}
