use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::get_project_path;
use crate::context::{detect_context, ProjectContext};
use crate::error::AtlasError;
use crate::locking::ProjectLock;
use crate::models::{Atom, AtomType, Confidence, IndexEntry};
use crate::storage::{ensure_project_exists, load_index, read_atom, save_index, write_atom};

/// Upsert request parameters.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UpsertRequest {
    /// Atom ID for updates, auto-generated for new atoms
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

    /// Override org (defaults to detected)
    #[serde(default)]
    pub org: Option<String>,

    /// Override project (defaults to detected)
    #[serde(default)]
    pub project: Option<String>,
}

/// Upsert response.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UpsertResult {
    pub id: String,
    pub created: bool,
}

/// Create or update an atom.
pub fn upsert(req: UpsertRequest) -> Result<UpsertResult, AtlasError> {
    let ctx = get_context_or_override(&req.org, &req.project)?;

    // Validate links before acquiring lock
    if let Some(ref links) = req.links {
        validate_links(&ctx, links)?;
    }

    let _lock = ProjectLock::acquire(&ctx.org, &ctx.project)?;

    // Ensure project exists
    ensure_project_exists(&ctx.org, &ctx.project)?;

    let mut index = load_index(&ctx.org, &ctx.project)?;

    let (atom, created) = if let Some(ref id) = req.id {
        // Update existing
        let mut atom = read_atom(&ctx.org, &ctx.project, id)?;
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
    write_atom(&ctx.org, &ctx.project, &atom)?;

    // Update index
    index.upsert_entry(IndexEntry::from_atom(&atom));
    save_index(&ctx.org, &ctx.project, &index)?;

    Ok(UpsertResult {
        id: atom.id,
        created,
    })
}

/// Validate links - reject cross-org links, allow cross-project within same org.
fn validate_links(ctx: &ProjectContext, links: &[String]) -> Result<(), AtlasError> {
    for link in links {
        // Parse link: either "K-XXXXXX" or "project/K-XXXXXX"
        if let Some((project, _id)) = parse_link(link) {
            // Cross-project link
            if project != ctx.project {
                // Verify target project exists in same org
                let project_path = get_project_path(&ctx.org, &project)?;
                if !project_path.exists() {
                    return Err(AtlasError::Validation(format!(
                        "Link target project '{}' does not exist in org '{}'",
                        project, ctx.org
                    )));
                }
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
