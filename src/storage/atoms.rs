use std::path::PathBuf;

use crate::config::get_atoms_path;
use crate::error::AtlasError;
use crate::models::Atom;

/// Get the file path for an atom.
fn get_atom_path(org: &str, project: &str, id: &str) -> Result<PathBuf, AtlasError> {
    Ok(get_atoms_path(org, project)?.join(format!("{}.yaml", id)))
}

/// Read an atom from disk.
pub fn read_atom(org: &str, project: &str, id: &str) -> Result<Atom, AtlasError> {
    let path = get_atom_path(org, project, id)?;

    if !path.exists() {
        return Err(AtlasError::NotFound(format!("Atom {} not found", id)));
    }

    let content = std::fs::read_to_string(&path)?;
    let atom: Atom = serde_yaml::from_str(&content)?;
    Ok(atom)
}

/// Write an atom to disk.
pub fn write_atom(org: &str, project: &str, atom: &Atom) -> Result<(), AtlasError> {
    let path = get_atom_path(org, project, &atom.id)?;

    // Ensure atoms directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = serde_yaml::to_string(atom)?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// Delete an atom file from disk.
pub fn delete_atom_file(org: &str, project: &str, id: &str) -> Result<(), AtlasError> {
    let path = get_atom_path(org, project, id)?;

    if path.exists() {
        std::fs::remove_file(&path)?;
    }

    Ok(())
}
