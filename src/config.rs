use std::path::PathBuf;

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

/// Get the path to a project's atoms directory.
pub fn get_atoms_path(org: &str, project: &str) -> Result<PathBuf, AtlasError> {
    Ok(get_project_path(org, project)?.join("atoms"))
}

/// Get the path to a project's index file.
pub fn get_index_path(org: &str, project: &str) -> Result<PathBuf, AtlasError> {
    Ok(get_project_path(org, project)?.join("index.yaml"))
}
