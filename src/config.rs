use std::path::{Path, PathBuf};

use crate::error::AtlasError;

/// Get the storage root directory (~/.atlas).
pub fn get_storage_root() -> Result<PathBuf, AtlasError> {
    if let Ok(path) = std::env::var("ATLAS_STORAGE") {
        return Ok(PathBuf::from(path));
    }

    dirs::home_dir()
        .map(|h| h.join(".atlas"))
        .ok_or_else(|| AtlasError::Config("Could not determine home directory".into()))
}

/// Get the orgs directory (~/.atlas/orgs).
pub fn get_orgs_path() -> Result<PathBuf, AtlasError> {
    Ok(get_storage_root()?.join("orgs"))
}

/// Get the path to a specific project's directory.
pub fn get_project_path(org: &str, project: &str) -> Result<PathBuf, AtlasError> {
    Ok(get_orgs_path()?.join(org).join(project))
}

/// Fail fast when the project path is a dangling symlink.
///
/// This indicates Atlas was previously configured for repo-local storage, but the
/// target `.atlas` directory was removed. Callers should surface a repair hint
/// rather than a raw IO error from mkdir/open/write operations.
pub fn ensure_project_path_is_usable(org: &str, project: &str) -> Result<(), AtlasError> {
    let project_path = get_project_path(org, project)?;
    ensure_not_broken_symlink(&project_path, org, project)
}

/// Get the path to a project's atoms directory.
pub fn get_atoms_path(org: &str, project: &str) -> Result<PathBuf, AtlasError> {
    Ok(get_project_path(org, project)?.join("atoms"))
}

/// Get the path to a project's index file.
pub fn get_index_path(org: &str, project: &str) -> Result<PathBuf, AtlasError> {
    Ok(get_project_path(org, project)?.join("index.yaml"))
}

/// List all projects within an org.
pub fn list_org_projects(org: &str) -> Result<Vec<String>, AtlasError> {
    let org_path = get_orgs_path()?.join(org);
    if !org_path.exists() {
        return Ok(Vec::new());
    }
    let mut projects = Vec::new();
    for entry in std::fs::read_dir(&org_path)? {
        let entry = entry?;
        if entry.path().is_dir() {
            projects.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    Ok(projects)
}

fn ensure_not_broken_symlink(path: &Path, org: &str, project: &str) -> Result<(), AtlasError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };

    if metadata.file_type().is_symlink() && !path.exists() {
        let target = std::fs::read_link(path)
            .map(|target| target.display().to_string())
            .unwrap_or_else(|_| "<unknown>".to_string());

        return Err(AtlasError::Storage(format!(
            "Broken local-storage symlink for {org}/{project}: {} -> {} (target missing). Run enable_local_storage to recreate it.",
            path.display(),
            target
        )));
    }

    Ok(())
}
