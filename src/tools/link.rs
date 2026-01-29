use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::context::detect_context;
use crate::error::AtlasError;
use crate::locking::ProjectLock;
use crate::storage::{read_atom, write_atom};

use super::reference::{format_atom_reference, parse_atom_reference};

// ============================================================================
// Request/Response types
// ============================================================================

/// Link request parameters.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct LinkRequest {
    /// Source atom: "org/project/K-000001", "project/K-000001", or "K-000001"
    pub source: String,

    /// Target atom: "org/project/K-000001", "project/K-000001", or "K-000001"
    pub target: String,
}

/// Link response.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct LinkResponse {
    /// Full source reference: "org/project/K-000001"
    pub source: String,

    /// Full target reference: "org/project/K-000001"
    pub target: String,

    /// False if link already existed
    pub created: bool,
}

/// Unlink response.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UnlinkResponse {
    /// Full source reference: "org/project/K-000001"
    pub source: String,

    /// Full target reference: "org/project/K-000001"
    pub target: String,

    /// False if link didn't exist
    pub removed: bool,
}

// ============================================================================
// Helpers
// ============================================================================

/// Format link relative to source project for storage.
/// Same project: "K-000001"
/// Cross-project: "other-project/K-000001"
fn format_link_for_storage(source_project: &str, target_project: &str, target_id: &str) -> String {
    if source_project == target_project {
        target_id.to_string()
    } else {
        format!("{}/{}", target_project, target_id)
    }
}

// ============================================================================
// Link tool
// ============================================================================

/// Create a directed link from source atom to target atom.
pub fn link(req: LinkRequest) -> Result<LinkResponse, AtlasError> {
    let ctx = detect_context()?;

    // Parse source and target with full path support
    let source_ref = parse_atom_reference(&req.source, &ctx);
    let target_ref = parse_atom_reference(&req.target, &ctx);

    // Validate: can't link to self
    if source_ref.org == target_ref.org
        && source_ref.project == target_ref.project
        && source_ref.id == target_ref.id
    {
        return Err(AtlasError::Validation(
            "Cannot link an atom to itself".into(),
        ));
    }

    // Validate: both atoms must be in the same org
    if source_ref.org != target_ref.org {
        return Err(AtlasError::Validation(
            "Cannot link atoms across different organizations".into(),
        ));
    }

    // Only lock source project (we only modify source atom)
    let _lock = ProjectLock::acquire(&source_ref.org, &source_ref.project)?;

    // Load source atom for modification
    let mut source_atom = read_atom(&source_ref.org, &source_ref.project, &source_ref.id)?;

    // Validate target exists (read-only check)
    let _ = read_atom(&target_ref.org, &target_ref.project, &target_ref.id)?;

    // Format link relative to source atom's project for storage
    let link_to_target =
        format_link_for_storage(&source_ref.project, &target_ref.project, &target_ref.id);

    // Check if link already exists
    if source_atom.links.contains(&link_to_target) {
        return Ok(LinkResponse {
            source: format_atom_reference(&source_ref.org, &source_ref.project, &source_ref.id),
            target: format_atom_reference(&target_ref.org, &target_ref.project, &target_ref.id),
            created: false,
        });
    }

    // Add link to source atom
    source_atom.links.push(link_to_target);
    source_atom.updated_at = Utc::now().date_naive();
    write_atom(&source_ref.org, &source_ref.project, &source_atom)?;

    Ok(LinkResponse {
        source: format_atom_reference(&source_ref.org, &source_ref.project, &source_ref.id),
        target: format_atom_reference(&target_ref.org, &target_ref.project, &target_ref.id),
        created: true,
    })
}

// ============================================================================
// Unlink tool
// ============================================================================

/// Remove a directed link from source atom to target atom.
pub fn unlink(req: LinkRequest) -> Result<UnlinkResponse, AtlasError> {
    let ctx = detect_context()?;

    // Parse source and target with full path support
    let source_ref = parse_atom_reference(&req.source, &ctx);
    let target_ref = parse_atom_reference(&req.target, &ctx);

    // Validate: both atoms must be in the same org
    if source_ref.org != target_ref.org {
        return Err(AtlasError::Validation(
            "Cannot unlink atoms across different organizations".into(),
        ));
    }

    // Only lock source project (we only modify source atom)
    let _lock = ProjectLock::acquire(&source_ref.org, &source_ref.project)?;

    // Load source atom for modification
    let mut source_atom = read_atom(&source_ref.org, &source_ref.project, &source_ref.id)?;

    // Format link relative to source atom's project
    // Note: No target validation - allows cleaning up dangling links if target was deleted
    let link_to_target =
        format_link_for_storage(&source_ref.project, &target_ref.project, &target_ref.id);

    // Remove link if present
    let removed = if let Some(pos) = source_atom.links.iter().position(|l| l == &link_to_target) {
        source_atom.links.remove(pos);
        source_atom.updated_at = Utc::now().date_naive();
        write_atom(&source_ref.org, &source_ref.project, &source_atom)?;
        true
    } else {
        false
    };

    Ok(UnlinkResponse {
        source: format_atom_reference(&source_ref.org, &source_ref.project, &source_ref.id),
        target: format_atom_reference(&target_ref.org, &target_ref.project, &target_ref.id),
        removed,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_link_for_storage_same_project() {
        let link = format_link_for_storage("project-a", "project-a", "K-000001");
        assert_eq!(link, "K-000001");
    }

    #[test]
    fn test_format_link_for_storage_cross_project() {
        let link = format_link_for_storage("project-a", "project-b", "K-000001");
        assert_eq!(link, "project-b/K-000001");
    }
}
