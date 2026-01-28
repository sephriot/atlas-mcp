use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::context::{detect_context, ProjectContext};
use crate::error::AtlasError;
use crate::locking::ProjectLock;
use crate::storage::{read_atom, write_atom};

// ============================================================================
// Request/Response types
// ============================================================================

/// Link request parameters.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct LinkRequest {
    /// Source atom: "K-000001" or "project/K-000001"
    pub source: String,

    /// Target atom: "K-000001" or "project/K-000001"
    pub target: String,

    /// Override org (defaults to detected)
    #[serde(default)]
    pub org: Option<String>,

    /// Override project for unprefixed IDs (defaults to detected)
    #[serde(default)]
    pub project: Option<String>,
}

/// Link response.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct LinkResponse {
    /// Normalized source reference: "project/K-000001"
    pub source: String,

    /// Normalized target reference: "project/K-000001"
    pub target: String,

    /// False if link already existed
    pub created: bool,
}

/// Unlink response.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UnlinkResponse {
    /// Normalized source reference: "project/K-000001"
    pub source: String,

    /// Normalized target reference: "project/K-000001"
    pub target: String,

    /// False if link didn't exist
    pub removed: bool,
}

// ============================================================================
// Helpers
// ============================================================================

/// Parse "project/K-000001" or "K-000001" into (project, id).
fn parse_atom_ref(ref_str: &str, default_project: &str) -> (String, String) {
    if let Some((proj, id)) = ref_str.split_once('/') {
        (proj.to_string(), id.to_string())
    } else {
        (default_project.to_string(), ref_str.to_string())
    }
}

/// Format link relative to source project.
/// Same project: "K-000001"
/// Cross-project: "other-project/K-000001"
fn format_link(source_project: &str, target_project: &str, target_id: &str) -> String {
    if source_project == target_project {
        target_id.to_string()
    } else {
        format!("{}/{}", target_project, target_id)
    }
}

fn get_context_or_override(
    org: &Option<String>,
    project: &Option<String>,
) -> Result<ProjectContext, AtlasError> {
    match (org, project) {
        (Some(o), Some(p)) => Ok(ProjectContext::new(o.clone(), p.clone())),
        (Some(_), None) | (None, Some(_)) => Err(AtlasError::Validation(
            "Both org and project must be specified together".into(),
        )),
        (None, None) => detect_context(),
    }
}

// ============================================================================
// Link tool
// ============================================================================

/// Create a bidirectional link between two atoms.
pub fn link(req: LinkRequest) -> Result<LinkResponse, AtlasError> {
    let ctx = get_context_or_override(&req.org, &req.project)?;

    // Parse source and target
    let (source_project, source_id) = parse_atom_ref(&req.source, &ctx.project);
    let (target_project, target_id) = parse_atom_ref(&req.target, &ctx.project);

    // Validate: can't link to self
    if source_project == target_project && source_id == target_id {
        return Err(AtlasError::Validation(
            "Cannot link an atom to itself".into(),
        ));
    }

    // Acquire locks on both projects (in sorted order to prevent deadlock)
    let mut projects = vec![&source_project, &target_project];
    projects.sort();
    projects.dedup();

    let _locks: Vec<_> = projects
        .iter()
        .map(|p| ProjectLock::acquire(&ctx.org, p))
        .collect::<Result<Vec<_>, _>>()?;

    // Load both atoms (validates they exist)
    let mut source_atom = read_atom(&ctx.org, &source_project, &source_id)?;
    let mut target_atom = read_atom(&ctx.org, &target_project, &target_id)?;

    // Format links relative to each atom's project
    let link_to_target = format_link(&source_project, &target_project, &target_id);
    let link_to_source = format_link(&target_project, &source_project, &source_id);

    // Check if link already exists (both directions)
    let source_has_link = source_atom.links.contains(&link_to_target);
    let target_has_link = target_atom.links.contains(&link_to_source);

    if source_has_link && target_has_link {
        // Link already exists in both directions
        return Ok(LinkResponse {
            source: format!("{}/{}", source_project, source_id),
            target: format!("{}/{}", target_project, target_id),
            created: false,
        });
    }

    // Add links where missing
    let mut modified = false;

    if !source_has_link {
        source_atom.links.push(link_to_target);
        source_atom.updated_at = Utc::now().date_naive();
        write_atom(&ctx.org, &source_project, &source_atom)?;
        modified = true;
    }

    if !target_has_link {
        target_atom.links.push(link_to_source);
        target_atom.updated_at = Utc::now().date_naive();
        write_atom(&ctx.org, &target_project, &target_atom)?;
        modified = true;
    }

    Ok(LinkResponse {
        source: format!("{}/{}", source_project, source_id),
        target: format!("{}/{}", target_project, target_id),
        created: modified,
    })
}

// ============================================================================
// Unlink tool
// ============================================================================

/// Remove a bidirectional link between two atoms.
pub fn unlink(req: LinkRequest) -> Result<UnlinkResponse, AtlasError> {
    let ctx = get_context_or_override(&req.org, &req.project)?;

    // Parse source and target
    let (source_project, source_id) = parse_atom_ref(&req.source, &ctx.project);
    let (target_project, target_id) = parse_atom_ref(&req.target, &ctx.project);

    // Acquire locks on both projects (in sorted order to prevent deadlock)
    let mut projects = vec![&source_project, &target_project];
    projects.sort();
    projects.dedup();

    let _locks: Vec<_> = projects
        .iter()
        .map(|p| ProjectLock::acquire(&ctx.org, p))
        .collect::<Result<Vec<_>, _>>()?;

    // Load both atoms (validates they exist)
    let mut source_atom = read_atom(&ctx.org, &source_project, &source_id)?;
    let mut target_atom = read_atom(&ctx.org, &target_project, &target_id)?;

    // Format links relative to each atom's project
    let link_to_target = format_link(&source_project, &target_project, &target_id);
    let link_to_source = format_link(&target_project, &source_project, &source_id);

    // Remove links where present
    let mut removed = false;

    if let Some(pos) = source_atom.links.iter().position(|l| l == &link_to_target) {
        source_atom.links.remove(pos);
        source_atom.updated_at = Utc::now().date_naive();
        write_atom(&ctx.org, &source_project, &source_atom)?;
        removed = true;
    }

    if let Some(pos) = target_atom.links.iter().position(|l| l == &link_to_source) {
        target_atom.links.remove(pos);
        target_atom.updated_at = Utc::now().date_naive();
        write_atom(&ctx.org, &target_project, &target_atom)?;
        removed = true;
    }

    Ok(UnlinkResponse {
        source: format!("{}/{}", source_project, source_id),
        target: format!("{}/{}", target_project, target_id),
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
    fn test_parse_atom_ref_with_project() {
        let (project, id) = parse_atom_ref("other-project/K-000001", "default");
        assert_eq!(project, "other-project");
        assert_eq!(id, "K-000001");
    }

    #[test]
    fn test_parse_atom_ref_without_project() {
        let (project, id) = parse_atom_ref("K-000001", "default");
        assert_eq!(project, "default");
        assert_eq!(id, "K-000001");
    }

    #[test]
    fn test_format_link_same_project() {
        let link = format_link("project-a", "project-a", "K-000001");
        assert_eq!(link, "K-000001");
    }

    #[test]
    fn test_format_link_cross_project() {
        let link = format_link("project-a", "project-b", "K-000001");
        assert_eq!(link, "project-b/K-000001");
    }
}
