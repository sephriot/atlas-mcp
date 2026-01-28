use std::os::unix::fs::symlink;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::{get_orgs_path, get_project_path};
use crate::context::{detect_context, ProjectContext};
use crate::error::AtlasError;
use crate::locking::ProjectLock;
use crate::models::{Atom, AtomType, Confidence, IndexEntry};
use crate::storage::{
    delete_atom_file, ensure_project_exists, load_index, read_atom as storage_read_atom,
    save_index,
};

// ============================================================================
// get_atom
// ============================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GetAtomRequest {
    /// Atom ID (e.g., K-000001) or project/id for cross-project within org
    pub id: String,

    /// Override org (defaults to detected)
    #[serde(default)]
    pub org: Option<String>,

    /// Override project (defaults to detected)
    #[serde(default)]
    pub project: Option<String>,
}

/// Get a full atom by ID.
pub fn get_atom(req: GetAtomRequest) -> Result<Atom, AtlasError> {
    let ctx = get_context_or_override(&req.org, &req.project)?;

    // Parse ID for cross-project reference
    let (project, id) = parse_id_reference(&req.id, &ctx.project);

    let _lock = ProjectLock::acquire(&ctx.org, &project)?;
    storage_read_atom(&ctx.org, &project, &id)
}

// ============================================================================
// list_atoms
// ============================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ListAtomsRequest {
    /// Filter by atom types
    #[serde(default)]
    pub types: Option<Vec<AtomType>>,

    /// Filter by tags (matches any)
    #[serde(default)]
    pub tags: Option<Vec<String>>,

    /// Filter by confidence level
    #[serde(default)]
    pub confidence: Option<Confidence>,

    /// Maximum number of results (default: 50)
    #[serde(default)]
    pub limit: Option<usize>,

    /// Override org (defaults to detected)
    #[serde(default)]
    pub org: Option<String>,

    /// Override project (defaults to detected)
    #[serde(default)]
    pub project: Option<String>,
}

/// List atom result.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ListAtomResult {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub atom_type: AtomType,
    pub confidence: Confidence,
    pub tags: Vec<String>,
}

impl From<&IndexEntry> for ListAtomResult {
    fn from(entry: &IndexEntry) -> Self {
        Self {
            id: entry.id.clone(),
            title: entry.title.clone(),
            atom_type: entry.atom_type,
            confidence: entry.confidence,
            tags: entry.tags.clone(),
        }
    }
}

/// List atoms with optional filtering.
pub fn list_atoms(req: ListAtomsRequest) -> Result<Vec<ListAtomResult>, AtlasError> {
    let ctx = get_context_or_override(&req.org, &req.project)?;
    let _lock = ProjectLock::acquire(&ctx.org, &ctx.project)?;
    let index = load_index(&ctx.org, &ctx.project)?;

    let limit = req.limit.unwrap_or(50);

    let results: Vec<ListAtomResult> = index
        .entries
        .iter()
        .filter(|entry| {
            // Apply type filter
            if let Some(ref types) = req.types {
                if !types.contains(&entry.atom_type) {
                    return false;
                }
            }

            // Apply confidence filter
            if let Some(ref conf) = req.confidence {
                if &entry.confidence != conf {
                    return false;
                }
            }

            // Apply tags filter (match any)
            if let Some(ref filter_tags) = req.tags {
                let entry_tags_lower: Vec<String> =
                    entry.tags.iter().map(|t| t.to_lowercase()).collect();
                let has_match = filter_tags
                    .iter()
                    .any(|t| entry_tags_lower.contains(&t.to_lowercase()));
                if !has_match {
                    return false;
                }
            }

            true
        })
        .take(limit)
        .map(ListAtomResult::from)
        .collect();

    Ok(results)
}

// ============================================================================
// delete_atom
// ============================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct DeleteAtomRequest {
    /// Atom ID to delete
    pub id: String,

    /// Override org (defaults to detected)
    #[serde(default)]
    pub org: Option<String>,

    /// Override project (defaults to detected)
    #[serde(default)]
    pub project: Option<String>,
}

/// Delete result.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DeleteResult {
    pub deleted: bool,
}

/// Delete an atom.
pub fn delete_atom(req: DeleteAtomRequest) -> Result<DeleteResult, AtlasError> {
    let ctx = get_context_or_override(&req.org, &req.project)?;
    let _lock = ProjectLock::acquire(&ctx.org, &ctx.project)?;

    let mut index = load_index(&ctx.org, &ctx.project)?;

    // Remove from index
    let removed = index.remove_entry(&req.id);

    if removed.is_some() {
        // Delete file
        delete_atom_file(&ctx.org, &ctx.project, &req.id)?;
        // Save updated index
        save_index(&ctx.org, &ctx.project, &index)?;
    }

    Ok(DeleteResult {
        deleted: removed.is_some(),
    })
}

// ============================================================================
// init_project
// ============================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct InitProjectRequest {
    /// Organization name
    pub org: String,

    /// Project name
    pub project: String,

    /// Create .atlas/ directory in current repo with reverse symlink from ~/.atlas.
    /// This makes atoms version-controllable via git.
    #[serde(default)]
    pub create_symlink: Option<bool>,
}

/// Init result.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct InitResult {
    /// Path to the project storage (either ~/.atlas/... or {cwd}/.atlas)
    pub path: String,
    /// Whether a symlink was created from ~/.atlas to repo
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub symlink_created: bool,
}

/// Initialize a new project.
///
/// Without create_symlink: creates project in ~/.atlas/orgs/{org}/{project}
/// With create_symlink: creates .atlas/ in current dir with reverse symlink from ~/.atlas
pub fn init_project(req: InitProjectRequest) -> Result<InitResult, AtlasError> {
    let cwd = std::env::current_dir()?;
    let mut symlink_created = false;

    let project_path = if req.create_symlink.unwrap_or(false) {
        // Repo storage mode: create .atlas in cwd and symlink from ~/.atlas
        let repo_atlas_path = cwd.join(".atlas");
        let repo_atoms_path = repo_atlas_path.join("atoms");
        let central_project_path = get_project_path(&req.org, &req.project)?;

        // Create .atlas/atoms in repo
        std::fs::create_dir_all(&repo_atoms_path)?;

        // Create index.yaml if it doesn't exist
        let index_path = repo_atlas_path.join("index.yaml");
        if !index_path.exists() {
            let index = crate::models::Index::new();
            let content = serde_yaml::to_string(&index)?;
            std::fs::write(&index_path, content)?;
        }

        // Ensure parent directory exists for central symlink
        if let Some(parent) = central_project_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Remove existing central path if it's a directory (not symlink)
        if central_project_path.exists() && !central_project_path.is_symlink() {
            // Check if it has atoms
            let has_atoms = central_project_path.join("atoms").exists()
                && std::fs::read_dir(central_project_path.join("atoms"))
                    .map(|mut d| d.next().is_some())
                    .unwrap_or(false);

            if has_atoms {
                return Err(AtlasError::Validation(format!(
                    "Central storage at {} already has atoms. Move them to {}/atoms/ first.",
                    central_project_path.display(),
                    repo_atlas_path.display()
                )));
            }
            std::fs::remove_dir_all(&central_project_path)?;
        }

        // Create symlink: ~/.atlas/orgs/{org}/{project} -> {cwd}/.atlas
        if !central_project_path.exists() {
            symlink(&repo_atlas_path, &central_project_path)?;
            symlink_created = true;
        }

        repo_atlas_path
    } else {
        // Standard mode: create in ~/.atlas
        let _lock = ProjectLock::acquire(&req.org, &req.project)?;
        ensure_project_exists(&req.org, &req.project)?;
        get_project_path(&req.org, &req.project)?
    };

    Ok(InitResult {
        path: project_path.to_string_lossy().to_string(),
        symlink_created,
    })
}

// ============================================================================
// list_projects
// ============================================================================

/// Project info.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ProjectInfo {
    pub org: String,
    pub project: String,
    pub atom_count: usize,
}

/// List all projects.
pub fn list_projects() -> Result<Vec<ProjectInfo>, AtlasError> {
    let orgs_path = get_orgs_path()?;

    if !orgs_path.exists() {
        return Ok(Vec::new());
    }

    let mut projects = Vec::new();

    for org_entry in std::fs::read_dir(&orgs_path)? {
        let org_entry = org_entry?;
        let org_path = org_entry.path();

        if !org_path.is_dir() {
            continue;
        }

        let org_name = org_entry.file_name().to_string_lossy().to_string();

        for project_entry in std::fs::read_dir(&org_path)? {
            let project_entry = project_entry?;
            let project_path = project_entry.path();

            if !project_path.is_dir() {
                continue;
            }

            let project_name = project_entry.file_name().to_string_lossy().to_string();

            // Count atoms
            let index = load_index(&org_name, &project_name).unwrap_or_default();

            projects.push(ProjectInfo {
                org: org_name.clone(),
                project: project_name,
                atom_count: index.entries.len(),
            });
        }
    }

    Ok(projects)
}

// ============================================================================
// get_context
// ============================================================================

/// Context info.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ContextInfo {
    pub org: String,
    pub project: String,
    pub cwd: String,
}

/// Get detected project context.
pub fn get_context() -> Result<ContextInfo, AtlasError> {
    let ctx = detect_context()?;
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    Ok(ContextInfo {
        org: ctx.org,
        project: ctx.project,
        cwd,
    })
}

// ============================================================================
// Helpers
// ============================================================================

/// Parse ID reference: "K-000001" or "project/K-000001"
fn parse_id_reference(id: &str, default_project: &str) -> (String, String) {
    if id.contains('/') {
        let parts: Vec<&str> = id.splitn(2, '/').collect();
        if parts.len() == 2 {
            return (parts[0].to_string(), parts[1].to_string());
        }
    }
    (default_project.to_string(), id.to_string())
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
