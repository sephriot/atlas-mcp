use std::os::unix::fs::symlink;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rmcp::model::Extensions;

use crate::config::{get_orgs_path, get_project_path};
use crate::context::{
    detect_context_full, ContextSource, DetectedContext, ProjectContext, HEADER_ATLAS_ORG,
    HEADER_ATLAS_PROJECT,
};
use crate::error::AtlasError;
use crate::locking::ProjectLock;
use crate::models::{Atom, AtomType, Confidence, IndexEntry};
use crate::serde_helpers::deserialize_optional_usize;
use crate::storage::{delete_atom_file, load_index, read_atom as storage_read_atom, save_index};

use super::reference::{format_atom_reference, parse_atom_reference, parse_scope};

// ============================================================================
// Context Hint Support
// ============================================================================

/// Hint about context detection for the user.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ContextHint {
    pub message: String,
    pub source: String,
}

/// Generic response wrapper that includes an optional context hint.
/// Can be used to wrap any tool response with a context hint.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct WithContext<T: Serialize> {
    #[serde(flatten)]
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_hint: Option<ContextHint>,
}

#[allow(dead_code)]
impl<T: Serialize> WithContext<T> {
    pub fn new(data: T, hint: Option<ContextHint>) -> Self {
        Self {
            data,
            context_hint: hint,
        }
    }
}

/// Create a context hint if the source is fallback.
pub fn make_context_hint(source: &ContextSource) -> Option<ContextHint> {
    match source {
        ContextSource::Fallback => Some(ContextHint {
            message:
                "Context detected via fallback. Use activate_project to set explicit org/project."
                    .to_string(),
            source: "fallback".to_string(),
        }),
        _ => None,
    }
}

/// Helper function to detect context with activation support.
#[allow(dead_code)]
pub fn detect_with_activation(
    activated: Option<&ProjectContext>,
) -> Result<DetectedContext, AtlasError> {
    detect_context_full(None, None, activated)
}

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
    get_atom_with_activation(req, None)
}

/// Get a full atom by ID with optional activated context.
pub fn get_atom_with_activation(
    req: GetAtomRequest,
    activated: Option<&ProjectContext>,
) -> Result<Atom, AtlasError> {
    let ctx = detect_context_full(None, None, activated)?.context;
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
    #[schemars(extend("examples" = [["api", "rust"]]))]
    pub tags: Option<Vec<String>>,

    /// Filter by confidence level
    #[serde(default)]
    pub confidence: Option<Confidence>,

    /// Maximum number of results (default: 50)
    #[serde(default, deserialize_with = "deserialize_optional_usize")]
    pub limit: Option<usize>,

    /// Scope filter: org name or org/project path. Examples: "acme", "acme/backend"
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
    list_atoms_with_activation(req, None)
}

/// List atoms with optional filtering and optional activated context.
pub fn list_atoms_with_activation(
    req: ListAtomsRequest,
    activated: Option<&ProjectContext>,
) -> Result<Vec<ListAtomResult>, AtlasError> {
    let ctx = detect_context_full(None, None, activated)?.context;
    let (list_org, scope_project) = parse_scope(req.scope.as_deref(), &ctx)?;

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
    delete_atom_with_activation(req, None)
}

/// Delete an atom with optional activated context.
pub fn delete_atom_with_activation(
    req: DeleteAtomRequest,
    activated: Option<&ProjectContext>,
) -> Result<DeleteResult, AtlasError> {
    let ctx = detect_context_full(None, None, activated)?.context;
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

    // If local-storage symlink exists but points to a missing target, remove it so
    // this call can repair the project back into a consistent repo-local state.
    let central_path_is_broken_symlink = std::fs::symlink_metadata(&central_project_path)
        .map(|metadata| metadata.file_type().is_symlink() && !central_project_path.exists())
        .unwrap_or(false);
    if central_path_is_broken_symlink {
        std::fs::remove_file(&central_project_path)?;
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
    /// True if this is the currently active project
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub active: bool,
}

/// List all projects.
///
/// Takes optional activated context to mark the active project.
pub fn list_projects_with_activation(
    activated: Option<&ProjectContext>,
) -> Result<Vec<ProjectInfo>, AtlasError> {
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

            // Check if this is the active project
            let active = activated
                .map(|a| a.org == org_name && a.project == project_name)
                .unwrap_or(false);

            projects.push(ProjectInfo {
                org: org_name.clone(),
                project: project_name,
                atom_count: index.entries.len(),
                active,
            });
        }
    }

    Ok(projects)
}

/// List all projects (without activation info).
pub fn list_projects() -> Result<Vec<ProjectInfo>, AtlasError> {
    list_projects_with_activation(None)
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
    /// Source of context detection
    pub source: ContextSource,
    /// Hint for the user if using fallback detection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<ContextHint>,
}

/// Get detected project context with source info.
///
/// In HTTP mode, extracts X-Atlas-Org and X-Atlas-Project headers from request.
/// Takes optional activated context for session-level override.
pub fn get_context_with_activation(
    extensions: Extensions,
    activated: Option<&ProjectContext>,
) -> Result<ContextInfo, AtlasError> {
    let detected = detect_context_from_extensions_full(&extensions, activated)?;
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let hint = make_context_hint(&detected.source);

    Ok(ContextInfo {
        org: detected.context.org,
        project: detected.context.project,
        cwd,
        source: detected.source,
        hint,
    })
}

/// Get detected project context (without activation).
#[allow(dead_code)]
pub fn get_context(extensions: Extensions) -> Result<ContextInfo, AtlasError> {
    get_context_with_activation(extensions, None)
}

/// Extract context from Extensions with full detection and source tracking.
fn detect_context_from_extensions_full(
    extensions: &Extensions,
    activated: Option<&ProjectContext>,
) -> Result<DetectedContext, AtlasError> {
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

        return detect_context_full(org, project, activated);
    }

    // No HTTP parts available (stdio mode), use detection with activation
    detect_context_full(None, None, activated)
}

/// Extract context from Extensions, checking HTTP headers if available (legacy).
#[allow(dead_code)]
fn detect_context_from_extensions(extensions: &Extensions) -> Result<ProjectContext, AtlasError> {
    Ok(detect_context_from_extensions_full(extensions, None)?.context)
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

#[cfg(test)]
mod tests {
    use super::{enable_local_storage, EnableLocalStorageRequest};
    use crate::error::AtlasError;
    use crate::locking::ProjectLock;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvVarGuard {
        key: &'static str,
        original: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &Path) -> Self {
            let original = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("atlas-{prefix}-{unique}"));
            std::fs::create_dir_all(&path).expect("temp dir should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn test_enable_local_storage_repairs_broken_symlink() {
        let _guard = env_lock().lock().expect("env lock should succeed");
        let temp = TempDir::new("repair-broken-symlink");
        let storage_root = temp.path().join("storage");
        let project_root = temp.path().join("repo");
        let broken_target = temp.path().join("missing-repo").join(".atlas");
        let repaired_target = project_root.join(".atlas");
        let central_project_path = storage_root.join("orgs").join("acme").join("atlas");

        std::fs::create_dir_all(&project_root).expect("project root should exist");
        std::fs::create_dir_all(central_project_path.parent().expect("parent exists"))
            .expect("central parent should exist");
        std::os::unix::fs::symlink(&broken_target, &central_project_path)
            .expect("broken symlink should be created");

        let _storage_guard = EnvVarGuard::set("ATLAS_STORAGE", &storage_root);
        let _project_root_guard = EnvVarGuard::set("ATLAS_PROJECT_ROOT", &project_root);

        let result = enable_local_storage(EnableLocalStorageRequest {
            org: "acme".to_string(),
            project: "atlas".to_string(),
        })
        .expect("enable_local_storage should repair broken symlink");

        assert!(result.symlink_created);
        assert_eq!(result.atoms_migrated, 0);
        assert_eq!(
            std::fs::read_link(&central_project_path).unwrap(),
            repaired_target
        );
        assert!(repaired_target.join("atoms").is_dir());
        assert!(repaired_target.join("index.yaml").is_file());
    }

    #[test]
    fn test_project_lock_reports_broken_symlink() {
        let _guard = env_lock().lock().expect("env lock should succeed");
        let temp = TempDir::new("broken-symlink-error");
        let storage_root = temp.path().join("storage");
        let central_project_path = storage_root.join("orgs").join("acme").join("atlas");
        let broken_target = temp.path().join("missing-repo").join(".atlas");

        std::fs::create_dir_all(central_project_path.parent().expect("parent exists"))
            .expect("central parent should exist");
        std::os::unix::fs::symlink(&broken_target, &central_project_path)
            .expect("broken symlink should be created");

        let _storage_guard = EnvVarGuard::set("ATLAS_STORAGE", &storage_root);

        let error = match ProjectLock::acquire("acme", "atlas") {
            Ok(_) => panic!("broken symlink should be rejected"),
            Err(error) => error,
        };

        match error {
            AtlasError::Storage(message) => {
                assert!(message.contains("Broken local-storage symlink"));
                assert!(message.contains("enable_local_storage"));
                assert!(message.contains("acme/atlas"));
            }
            other => panic!("expected storage error, got {other:?}"),
        }
    }
}
