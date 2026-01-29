use std::os::unix::fs::symlink;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rmcp::model::Extensions;

use crate::config::{get_orgs_path, get_project_path};
use crate::context::{
    detect_context, detect_context_with_headers, ProjectContext, HEADER_ATLAS_ORG,
    HEADER_ATLAS_PROJECT,
};
use crate::error::AtlasError;
use crate::locking::ProjectLock;
use crate::models::{Atom, AtomType, Confidence, IndexEntry};
use crate::serde_helpers::deserialize_optional_usize;
use crate::storage::{delete_atom_file, load_index, read_atom as storage_read_atom, save_index};

use super::reference::{format_atom_reference, parse_atom_reference, parse_scope};

// ============================================================================
// get_atom
// ============================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GetAtomRequest {
    /// Atom ID: "org/project/K-000001", "project/K-000001", or "K-000001"
    pub id: String,
}

/// Get a full atom by ID.
pub fn get_atom(req: GetAtomRequest) -> Result<Atom, AtlasError> {
    let ctx = detect_context()?;
    let atom_ref = parse_atom_reference(&req.id, &ctx);

    let _lock = ProjectLock::acquire(&atom_ref.org, &atom_ref.project)?;
    storage_read_atom(&atom_ref.org, &atom_ref.project, &atom_ref.id)
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
    #[serde(default, deserialize_with = "deserialize_optional_usize")]
    pub limit: Option<usize>,

    /// Scope filter: "org" or "org/project". Defaults to detected context (single project).
    #[serde(default)]
    pub scope: Option<String>,
}

/// List atom result.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ListAtomResult {
    /// Full atom reference: "org/project/K-000001"
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub atom_type: AtomType,
    pub confidence: Confidence,
    pub tags: Vec<String>,
}

impl ListAtomResult {
    fn from_entry(entry: &IndexEntry, org: &str, project: &str) -> Self {
        Self {
            id: format_atom_reference(org, project, &entry.id),
            title: entry.title.clone(),
            atom_type: entry.atom_type,
            confidence: entry.confidence,
            tags: entry.tags.clone(),
        }
    }
}

/// List atoms with optional filtering.
pub fn list_atoms(req: ListAtomsRequest) -> Result<Vec<ListAtomResult>, AtlasError> {
    let ctx = detect_context()?;
    let (list_org, scope_project) = parse_scope(req.scope.as_deref(), &ctx);

    let limit = req.limit.unwrap_or(50);

    // Determine which projects to list from
    let projects_to_list: Vec<String> = if let Some(ref proj) = scope_project {
        vec![proj.clone()]
    } else {
        // Default: list from current project only (unlike search which searches all)
        vec![ctx.project.clone()]
    };

    let mut results: Vec<ListAtomResult> = Vec::new();

    for project_name in projects_to_list {
        let _lock = ProjectLock::acquire(&list_org, &project_name)?;
        let index = match load_index(&list_org, &project_name) {
            Ok(idx) => idx,
            Err(_) => continue,
        };

        for entry in &index.entries {
            if results.len() >= limit {
                break;
            }

            // Apply type filter
            if let Some(ref types) = req.types {
                if !types.contains(&entry.atom_type) {
                    continue;
                }
            }

            // Apply confidence filter
            if let Some(ref conf) = req.confidence {
                if &entry.confidence != conf {
                    continue;
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
                    continue;
                }
            }

            results.push(ListAtomResult::from_entry(entry, &list_org, &project_name));
        }
    }

    Ok(results)
}

// ============================================================================
// delete_atom
// ============================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct DeleteAtomRequest {
    /// Atom ID: "org/project/K-000001", "project/K-000001", or "K-000001"
    pub id: String,
}

/// Delete result.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DeleteResult {
    pub deleted: bool,
}

/// Delete an atom.
pub fn delete_atom(req: DeleteAtomRequest) -> Result<DeleteResult, AtlasError> {
    let ctx = detect_context()?;
    let atom_ref = parse_atom_reference(&req.id, &ctx);

    let _lock = ProjectLock::acquire(&atom_ref.org, &atom_ref.project)?;

    let mut index = load_index(&atom_ref.org, &atom_ref.project)?;

    // Remove from index
    let removed = index.remove_entry(&atom_ref.id);

    if removed.is_some() {
        // Delete file
        delete_atom_file(&atom_ref.org, &atom_ref.project, &atom_ref.id)?;
        // Save updated index
        save_index(&atom_ref.org, &atom_ref.project, &index)?;
    }

    Ok(DeleteResult {
        deleted: removed.is_some(),
    })
}

// ============================================================================
// init_project
// ============================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct EnableLocalStorageRequest {
    /// Organization name
    pub org: String,

    /// Project name
    pub project: String,
}

/// Enable local storage result.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct EnableLocalStorageResult {
    /// Path to the local .atlas directory
    pub path: String,
    /// Whether a new symlink was created from ~/.atlas to the local directory
    pub symlink_created: bool,
    /// Number of atoms migrated from central storage to local
    #[serde(skip_serializing_if = "is_zero")]
    pub atoms_migrated: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// Enable local storage for a project.
///
/// Creates .atlas/ in project root with reverse symlink from ~/.atlas.
/// This makes atoms version-controllable via git.
///
/// Project root is determined by (in order):
/// 1. ATLAS_PROJECT_ROOT env var (set via --project-root CLI arg)
/// 2. Current working directory
pub fn enable_local_storage(
    req: EnableLocalStorageRequest,
) -> Result<EnableLocalStorageResult, AtlasError> {
    let project_root = get_project_root()?;
    let mut symlink_created = false;
    let mut atoms_migrated = 0usize;

    let repo_atlas_path = project_root.join(".atlas");
    let repo_atoms_path = repo_atlas_path.join("atoms");
    let central_project_path = get_project_path(&req.org, &req.project)?;

    // Create .atlas/atoms in project root
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

    // Migrate existing atoms from central storage to local
    if central_project_path.exists() && !central_project_path.is_symlink() {
        let central_atoms_path = central_project_path.join("atoms");
        let central_index_path = central_project_path.join("index.yaml");

        // Move atoms if they exist
        if central_atoms_path.exists() {
            for entry in std::fs::read_dir(&central_atoms_path)? {
                let entry = entry?;
                let src = entry.path();
                let dest = repo_atoms_path.join(entry.file_name());
                if !dest.exists() {
                    move_file(&src, &dest)?;
                    atoms_migrated += 1;
                }
            }
        }

        // Merge index: prefer central index entries for migrated atoms
        if central_index_path.exists() {
            let central_content = std::fs::read_to_string(&central_index_path)?;
            let central_index: crate::models::Index = serde_yaml::from_str(&central_content)?;

            let repo_index_path = repo_atlas_path.join("index.yaml");
            let mut repo_index = if repo_index_path.exists() {
                let content = std::fs::read_to_string(&repo_index_path)?;
                serde_yaml::from_str(&content)?
            } else {
                crate::models::Index::new()
            };

            // Add central entries that don't exist in local
            for entry in central_index.entries {
                if !repo_index.entries.iter().any(|e| e.id == entry.id) {
                    repo_index.entries.push(entry);
                }
            }

            // Update next_id if central has higher
            if central_index.next_id > repo_index.next_id {
                repo_index.next_id = central_index.next_id;
            }

            let content = serde_yaml::to_string(&repo_index)?;
            std::fs::write(&repo_index_path, content)?;
        }

        // Remove central directory after migration
        std::fs::remove_dir_all(&central_project_path)?;
    }

    // Create symlink: ~/.atlas/orgs/{org}/{project} -> {project_root}/.atlas
    if !central_project_path.exists() {
        symlink(&repo_atlas_path, &central_project_path)?;
        symlink_created = true;
    }

    Ok(EnableLocalStorageResult {
        path: repo_atlas_path.to_string_lossy().to_string(),
        symlink_created,
        atoms_migrated,
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
///
/// In HTTP mode, extracts X-Atlas-Org and X-Atlas-Project headers from request.
pub fn get_context(extensions: Extensions) -> Result<ContextInfo, AtlasError> {
    let ctx = detect_context_from_extensions(&extensions)?;
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    Ok(ContextInfo {
        org: ctx.org,
        project: ctx.project,
        cwd,
    })
}

/// Extract context from Extensions, checking HTTP headers if available.
fn detect_context_from_extensions(extensions: &Extensions) -> Result<ProjectContext, AtlasError> {
    // Try to extract HTTP headers if running in HTTP mode
    if let Some(parts) = extensions.get::<http::request::Parts>() {
        let org = parts
            .headers
            .get(HEADER_ATLAS_ORG)
            .and_then(|v| v.to_str().ok());
        let project = parts
            .headers
            .get(HEADER_ATLAS_PROJECT)
            .and_then(|v| v.to_str().ok());

        return detect_context_with_headers(org, project);
    }

    // No HTTP parts available (stdio mode), use standard detection
    detect_context()
}

// ============================================================================
// Helpers
// ============================================================================

/// Get project root directory for repo storage mode.
///
/// Returns ATLAS_PROJECT_ROOT env var if set, otherwise current directory.
fn get_project_root() -> Result<std::path::PathBuf, AtlasError> {
    if let Ok(path) = std::env::var("ATLAS_PROJECT_ROOT") {
        let path = std::path::PathBuf::from(path);
        if !path.exists() {
            return Err(AtlasError::Validation(format!(
                "Project root does not exist: {}",
                path.display()
            )));
        }
        return Ok(path);
    }
    std::env::current_dir().map_err(|e| AtlasError::Context(format!("Failed to get CWD: {}", e)))
}

/// Move a file, falling back to copy+delete if rename fails across filesystems.
fn move_file(src: &std::path::Path, dest: &std::path::Path) -> std::io::Result<()> {
    match std::fs::rename(src, dest) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(18) => {
            // EXDEV (18): Cross-device link - copy then remove
            std::fs::copy(src, dest)?;
            std::fs::remove_file(src)?;
            Ok(())
        }
        Err(e) => Err(e),
    }
}
