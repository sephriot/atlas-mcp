use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::get_project_path;
use crate::context::detect_context;
use crate::error::AtlasError;
use crate::locking::ProjectLock;
use crate::models::{Atom, AtomType, Confidence, IndexEntry};
use crate::storage::{ensure_project_exists, load_index, read_atom, save_index, write_atom};

use super::reference::{format_atom_reference, parse_atom_reference};

/// Upsert request parameters.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpsertRequest {
    /// Atom ID for updates: "org/project/K-000001", "project/K-000001", or "K-000001".
    /// Omit for new atoms (auto-generated).
    #[serde(default)]
    pub id: Option<String>,

    /// Short descriptive title
    pub title: String,

    /// Type of knowledge
    #[serde(rename = "type")]
    pub atom_type: AtomType,

    /// Confidence level
    pub confidence: Confidence,

    /// Brief explanation
    pub summary: String,

    /// Extended content (optional)
    #[serde(default)]
    pub details: Option<String>,

    /// Potential pitfalls (optional)
    #[serde(default)]
    pub pitfalls: Option<Vec<String>>,

    /// Keywords for search (optional)
    #[serde(default)]
    pub tags: Option<Vec<String>>,

    /// References - simple strings (paths or URLs)
    #[serde(default)]
    pub sources: Option<Vec<String>>,

    /// Related atoms - id or project/id format
    #[serde(default)]
    pub links: Option<Vec<String>>,
}

/// Upsert response.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UpsertResult {
    /// Full atom reference: "org/project/K-000001"
    pub id: String,
    pub created: bool,
}

/// Create or update an atom.
pub fn upsert(req: UpsertRequest) -> Result<UpsertResult, AtlasError> {
    let ctx = detect_context()?;

    // Determine org/project based on whether this is an update or create
    let (target_org, target_project) = if let Some(ref id) = req.id {
        // Update: parse full path from id
        let atom_ref = parse_atom_reference(id, &ctx);
        (atom_ref.org, atom_ref.project)
    } else {
        // Create: use detected context
        (ctx.org.clone(), ctx.project.clone())
    };

    // Validate links before acquiring lock
    if let Some(ref links) = req.links {
        validate_links(&target_org, links)?;
    }

    let _lock = ProjectLock::acquire(&target_org, &target_project)?;

    // Ensure project exists
    ensure_project_exists(&target_org, &target_project)?;

    let mut index = load_index(&target_org, &target_project)?;

    let (atom, created) = if let Some(ref id_str) = req.id {
        // Update existing - re-parse to get just the ID part
        let atom_ref = parse_atom_reference(id_str, &ctx);
        let mut atom = read_atom(&target_org, &target_project, &atom_ref.id)?;
        atom.title = req.title;
        atom.atom_type = req.atom_type;
        atom.confidence = req.confidence;
        atom.summary = req.summary;
        atom.details = req.details;
        atom.pitfalls = req.pitfalls.unwrap_or_default();
        atom.tags = req.tags.unwrap_or_default();
        atom.sources = req.sources.unwrap_or_default();
        atom.links = req.links.unwrap_or_default();
        atom.updated_at = Utc::now().date_naive();
        (atom, false)
    } else {
        // Create new
        let id = index.generate_id();
        let mut atom = Atom::new(id, req.title, req.atom_type, req.confidence, req.summary);
        atom.details = req.details;
        atom.pitfalls = req.pitfalls.unwrap_or_default();
        atom.tags = req.tags.unwrap_or_default();
        atom.sources = req.sources.unwrap_or_default();
        atom.links = req.links.unwrap_or_default();
        (atom, true)
    };

    // Write atom
    write_atom(&target_org, &target_project, &atom)?;

    // Update index
    index.upsert_entry(IndexEntry::from_atom(&atom));
    save_index(&target_org, &target_project, &index)?;

    Ok(UpsertResult {
        id: format_atom_reference(&target_org, &target_project, &atom.id),
        created,
    })
}

/// Validate links - reject cross-org links, allow cross-project within same org.
fn validate_links(org: &str, links: &[String]) -> Result<(), AtlasError> {
    for link in links {
        // Parse link: either "K-XXXXXX" or "project/K-XXXXXX"
        if let Some((project, _id)) = parse_link(link) {
            // Cross-project link - verify target project exists in same org
            let project_path = get_project_path(org, &project)?;
            if !project_path.exists() {
                return Err(AtlasError::Validation(format!(
                    "Link target project '{}' does not exist in org '{}'",
                    project, org
                )));
            }
        }
        // Simple ID link (same project) - always valid
    }
    Ok(())
}

/// Parse a link into (project, id) or just id.
fn parse_link(link: &str) -> Option<(String, String)> {
    if link.contains('/') {
        let parts: Vec<&str> = link.splitn(2, '/').collect();
        if parts.len() == 2 {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    None
}
