use crate::config::{get_atoms_path, get_index_path, get_project_path};
use crate::error::AtlasError;
use crate::models::Index;

/// Load the index for a project, creating if it doesn't exist.
pub fn load_index(org: &str, project: &str) -> Result<Index, AtlasError> {
    let path = get_index_path(org, project)?;

    if !path.exists() {
        return Ok(Index::new());
    }

    let content = std::fs::read_to_string(&path)?;
    let index: Index = serde_yaml::from_str(&content)?;
    Ok(index)
}

/// Save the index for a project.
pub fn save_index(org: &str, project: &str, index: &Index) -> Result<(), AtlasError> {
    let path = get_index_path(org, project)?;

    // Ensure project directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = serde_yaml::to_string(index)?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// Ensure the project directory structure exists.
pub fn ensure_project_exists(org: &str, project: &str) -> Result<(), AtlasError> {
    let project_path = get_project_path(org, project)?;
    let atoms_path = get_atoms_path(org, project)?;

    std::fs::create_dir_all(&project_path)?;
    std::fs::create_dir_all(&atoms_path)?;

    // Create index if it doesn't exist
    let index_path = get_index_path(org, project)?;
    if !index_path.exists() {
        let index = Index::new();
        save_index(org, project, &index)?;
    }

    Ok(())
}
