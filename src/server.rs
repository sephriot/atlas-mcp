use std::sync::Arc;

use parking_lot::RwLock;
use rmcp::{
    handler::server::tool::ToolRouter, handler::server::wrapper::Parameters, model::Extensions,
    model::*, tool, tool_router, ErrorData as McpError,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::get_project_path;
use crate::context::{validate_name, ProjectContext};
use crate::tools::{
    delete_atom_with_activation, enable_local_storage, get_atom_with_activation,
    get_context_with_activation, link_with_activation, list_atoms_with_activation,
    list_projects_with_activation, search_with_activation, unlink_with_activation,
    upsert_with_activation, DeleteAtomRequest, EnableLocalStorageRequest, GetAtomRequest,
    LinkRequest, ListAtomsRequest, SearchRequest, UpsertRequest,
};

const INSTRUCTIONS: &str = r#"Atlas MCP - Long-term memory for AI agents.

WORKFLOW:
1. SEARCH first - Before any task, search for relevant patterns/gotchas/decisions
2. READ full atoms - Use get for each relevant search result
3. APPLY knowledge - Let retrieved atoms constrain your approach
4. RECORD learnings - Use upsert when you discover something reusable
5. LINK related atoms - Use link to connect related knowledge

ATOM TYPES:
- note: Facts, observations, domain knowledge
- gotcha: Warnings, pitfalls, "watch out for X"
- recipe: Patterns, how-to guides, code snippets
- decision: Architectural rationale, why X over Y

ATOM IDs:
- All IDs returned as full paths: "org/project/K-000001"
- Accepts: full path, "project/K-000001", or bare "K-000001" (context fills gaps)
- Cross-project/org operations use full paths

LINKING:
- Use link/unlink to create directed connections between atoms
- Links are directed: A links to B does NOT mean B links to A
- Cross-project links supported within same org

CONTEXT: Automatically detected from git remote (org/project). Use context to verify.
Use activate_project to explicitly set org/project when not in a git repository.

ACTIVATION:
- Use activate_project when context shows fallback detection (global/dirname)
- Activation persists for the session, overriding automatic detection
- Use deactivate_project to revert to automatic detection

CITATION: Reference atom IDs in your reasoning, e.g., [K-000042]

PARAMETER FORMAT:
- upsert requires title, type, confidence, summary. Fields "type" and "confidence" MUST be JSON strings (e.g. "recipe", "high"), not bare words (recipe without quotes is invalid JSON).
- Array fields (tags, sources, links, pitfalls) MUST be JSON arrays, NOT strings
- CORRECT: tags: ["api", "rust"]
- WRONG: tags: "[\"api\", \"rust\"]"
"#;

// ============================================================================
// Request/Response types for activate_project
// ============================================================================

/// Request to activate a project context for this session.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ActivateProjectRequest {
    /// Organization name (alphanumeric, hyphens, underscores)
    pub org: String,
    /// Project name (alphanumeric, hyphens, underscores)
    pub project: String,
}

/// Response from activating a project.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ActivateProjectResponse {
    pub org: String,
    pub project: String,
    /// Whether this is a new project (no existing atoms)
    pub is_new: bool,
    pub message: String,
}

/// Response from deactivating a project.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DeactivateProjectResponse {
    /// The new auto-detected org
    pub org: String,
    /// The new auto-detected project
    pub project: String,
    /// Source of the new context
    pub source: String,
    pub message: String,
}

// ============================================================================
// AtlasServer
// ============================================================================

/// Atlas MCP Server for centralized knowledge management.
#[derive(Clone)]
pub struct AtlasServer {
    tool_router: ToolRouter<Self>,
    /// Session-scoped activated project context
    activated_context: Arc<RwLock<Option<ProjectContext>>>,
}

impl AtlasServer {
    /// Get the currently activated context, if any.
    pub fn get_activated_context(&self) -> Option<ProjectContext> {
        self.activated_context.read().clone()
    }

    /// Set the activated context.
    pub fn set_activated_context(&self, ctx: Option<ProjectContext>) {
        *self.activated_context.write() = ctx;
    }
}

#[tool_router]
impl AtlasServer {
    pub fn new() -> Self {
        let mut tool_router = Self::tool_router();

        // Remove enable_local tool in HTTP mode (symlinks don't make sense remotely)
        if std::env::var("ATLAS_HTTP_MODE").is_ok() {
            tool_router.remove_route("enable_local");
        }

        Self {
            tool_router,
            activated_context: Arc::new(RwLock::new(None)),
        }
    }

    #[tool(
        description = "Search knowledge atoms by title, tags, type, and confidence. Returns full atom IDs (org/project/id). Optionally filter by scope: org name (e.g., 'acme') searches all projects in that org, or 'org/project' (e.g., 'acme/backend') for a specific project. Default: searches entire detected org with current project scoring higher."
    )]
    async fn search(&self, params: Parameters<SearchRequest>) -> Result<CallToolResult, McpError> {
        let activated = self.get_activated_context();
        let results = search_with_activation(params.0, activated.as_ref()).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&results).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = r#"Create or update a knowledge atom. For updates, provide id as full path (org/project/id), project/id, or bare id. Omit id for new atoms. Required: title, type, confidence, summary — type and confidence must be JSON strings (e.g. "recipe", "high"), not bare identifiers. Minimal valid arguments: {"title":"My note","type":"recipe","confidence":"high","summary":"Brief text"}. Optional arrays (tags, sources, links, pitfalls) must be JSON arrays like ["a","b"], not stringified arrays. Correct: tags: ["api","rust"]. Wrong: tags: "[\"api\",\"rust\"]"."#
    )]
    async fn upsert(&self, params: Parameters<UpsertRequest>) -> Result<CallToolResult, McpError> {
        let activated = self.get_activated_context();
        let result = upsert_with_activation(params.0, activated.as_ref()).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Get full atom content by ID. Accepts org/project/id, project/id, or bare id (context fills gaps)."
    )]
    async fn get(&self, params: Parameters<GetAtomRequest>) -> Result<CallToolResult, McpError> {
        let activated = self.get_activated_context();
        let atom = get_atom_with_activation(params.0, activated.as_ref()).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&atom).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "List atoms with optional filtering by type, tags, and confidence. Optionally filter by scope (e.g., 'acme/backend'). Default: lists detected project only."
    )]
    async fn atoms(
        &self,
        params: Parameters<ListAtomsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let activated = self.get_activated_context();
        let results =
            list_atoms_with_activation(params.0, activated.as_ref()).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&results).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Delete an atom by ID. Accepts org/project/id, project/id, or bare id (context fills gaps)."
    )]
    async fn delete(
        &self,
        params: Parameters<DeleteAtomRequest>,
    ) -> Result<CallToolResult, McpError> {
        let activated = self.get_activated_context();
        let result =
            delete_atom_with_activation(params.0, activated.as_ref()).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Create a directed link from source to target atom. Both must be in same org. Accepts org/project/id, project/id, or bare id."
    )]
    async fn link(&self, params: Parameters<LinkRequest>) -> Result<CallToolResult, McpError> {
        let activated = self.get_activated_context();
        let result = link_with_activation(params.0, activated.as_ref()).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Remove a directed link from source to target atom. Target need not exist (allows cleaning dangling links). Accepts org/project/id, project/id, or bare id."
    )]
    async fn unlink(&self, params: Parameters<LinkRequest>) -> Result<CallToolResult, McpError> {
        let activated = self.get_activated_context();
        let result = unlink_with_activation(params.0, activated.as_ref()).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Enable local storage for a project. Creates .atlas/ in project root and symlinks from ~/.atlas, making atoms version-controllable via git."
    )]
    async fn enable_local(
        &self,
        params: Parameters<EnableLocalStorageRequest>,
    ) -> Result<CallToolResult, McpError> {
        let result = enable_local_storage(params.0).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "List all projects across all organizations. Shows 'active: true' for currently activated project."
    )]
    async fn projects(&self) -> Result<CallToolResult, McpError> {
        let activated = self.get_activated_context();
        let results = list_projects_with_activation(activated.as_ref()).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&results).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Get detected project context (org, project, cwd, source). Shows detection source and suggests activate_project when using fallback. In HTTP mode, use X-Atlas-Org and X-Atlas-Project headers to override."
    )]
    async fn context(&self, extensions: Extensions) -> Result<CallToolResult, McpError> {
        let activated = self.get_activated_context();
        let ctx =
            get_context_with_activation(extensions, activated.as_ref()).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&ctx).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Activate a project context for this session. Overrides automatic detection until session ends or deactivate_project is called. Use when working outside a git repository or to switch projects."
    )]
    async fn activate_project(
        &self,
        params: Parameters<ActivateProjectRequest>,
    ) -> Result<CallToolResult, McpError> {
        let req = params.0;

        // Validate org and project names
        validate_name(&req.org).map_err(to_mcp_error)?;
        validate_name(&req.project).map_err(to_mcp_error)?;

        // Check if project exists (has atoms)
        let project_path = get_project_path(&req.org, &req.project).map_err(to_mcp_error)?;
        let is_new = !project_path.exists();

        // Set the activated context
        self.set_activated_context(Some(ProjectContext::new(
            req.org.clone(),
            req.project.clone(),
        )));

        let message = if is_new {
            format!(
                "Activated new project {}/{}. Atoms will be created here.",
                req.org, req.project
            )
        } else {
            format!("Activated existing project {}/{}.", req.org, req.project)
        };

        let response = ActivateProjectResponse {
            org: req.org,
            project: req.project,
            is_new,
            message,
        };

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Clear activated project context, reverting to automatic detection (git remote, local storage, env vars, or fallback)."
    )]
    async fn deactivate_project(&self) -> Result<CallToolResult, McpError> {
        use crate::context::{detect_context_full, ContextSource};

        // Clear the activated context
        self.set_activated_context(None);

        // Get the new auto-detected context
        let detected = detect_context_full(None, None, None).map_err(to_mcp_error)?;

        let source_str = match detected.source {
            ContextSource::HttpHeaders => "http_headers",
            ContextSource::Activated => "activated",
            ContextSource::LocalStorage => "local_storage",
            ContextSource::GitRemote => "git_remote",
            ContextSource::EnvVars => "env_vars",
            ContextSource::Fallback => "fallback",
        };

        let response = DeactivateProjectResponse {
            org: detected.context.org,
            project: detected.context.project,
            source: source_str.to_string(),
            message: format!(
                "Deactivated project context. Now using {} detection.",
                source_str
            ),
        };

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&response).unwrap_or_default(),
        )]))
    }
}

impl Default for AtlasServer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    use rmcp::handler::server::wrapper::Parameters;
    use serde_json::Value;

    use super::*;

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn with_env_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn temp_storage_dir() -> std::path::PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("atlas-test-{}", now));
        fs::create_dir_all(&dir).expect("create temp storage dir");
        dir
    }

    fn extract_json_text(result: &CallToolResult) -> Value {
        let text = result
            .content
            .first()
            .and_then(|content| content.as_text())
            .map(|text| text.text.clone())
            .expect("tool result should include text content");
        serde_json::from_str(&text).expect("tool result should be valid json")
    }

    fn upsert_request(title: &str) -> UpsertRequest {
        UpsertRequest {
            id: None,
            title: title.to_string(),
            atom_type: crate::models::AtomType::Note,
            confidence: crate::models::Confidence::Medium,
            summary: "summary".to_string(),
            details: None,
            pitfalls: None,
            tags: None,
            sources: None,
            links: None,
        }
    }

    #[tokio::test]
    async fn test_activation_applies_to_tools() {
        let _lock = with_env_lock();

        let storage_dir = temp_storage_dir();
        let storage = storage_dir.to_string_lossy().to_string();

        let orig_storage = std::env::var("ATLAS_STORAGE").ok();
        let orig_org = std::env::var("ATLAS_ORG").ok();
        let orig_project = std::env::var("ATLAS_PROJECT").ok();

        std::env::set_var("ATLAS_STORAGE", &storage);
        std::env::remove_var("ATLAS_ORG");
        std::env::remove_var("ATLAS_PROJECT");

        let server = AtlasServer::new();
        let org = "test-org";
        let project = "test-project";
        server.set_activated_context(Some(ProjectContext::new(org.into(), project.into())));

        // Upsert should land in activated org/project.
        let upsert_result = server
            .upsert(Parameters(upsert_request("First")))
            .await
            .expect("upsert should succeed");
        let upsert_json = extract_json_text(&upsert_result);
        let id = upsert_json
            .get("id")
            .and_then(|v| v.as_str())
            .expect("upsert response should include id");
        assert!(
            id.starts_with(&format!("{}/{}", org, project)),
            "upsert id should include activated org/project"
        );

        let bare_id = id.split('/').last().expect("id should contain atom id");

        // get_atom should resolve bare ids against activated context.
        let get_result = server
            .get(Parameters(GetAtomRequest {
                id: bare_id.to_string(),
            }))
            .await
            .expect("get should succeed");
        let get_json = extract_json_text(&get_result);
        assert_eq!(
            get_json.get("title").and_then(|v| v.as_str()),
            Some("First")
        );

        // list_atoms should list from activated project.
        let list_result = server
            .atoms(Parameters(ListAtomsRequest {
                types: None,
                tags: None,
                confidence: None,
                limit: Some(50),
                scope: None,
            }))
            .await
            .expect("list atoms should succeed");
        let list_json = extract_json_text(&list_result);
        assert!(
            list_json
                .as_array()
                .map(|items| !items.is_empty())
                .unwrap_or(false),
            "list atoms should include the created atom"
        );

        // search should use activated context for scoring and ids.
        let search_result = server
            .search(Parameters(SearchRequest {
                query: Some("First".to_string()),
                types: None,
                tags: None,
                confidence: None,
                page: Some(1),
                page_size: Some(20),
                scope: None,
            }))
            .await
            .expect("search should succeed");
        let search_json = extract_json_text(&search_result);
        let search_id = search_json["results"]
            .as_array()
            .and_then(|items| items.first())
            .and_then(|item| item.get("id"))
            .and_then(|value| value.as_str())
            .unwrap_or("");
        assert!(
            search_id.starts_with(&format!("{}/{}", org, project)),
            "search results should include activated org/project"
        );

        // link/unlink with bare ids should stay in activated project.
        let second_upsert = server
            .upsert(Parameters(upsert_request("Second")))
            .await
            .expect("second upsert should succeed");
        let second_json = extract_json_text(&second_upsert);
        let second_id = second_json
            .get("id")
            .and_then(|v| v.as_str())
            .expect("upsert response should include id");
        let second_bare = second_id
            .split('/')
            .last()
            .expect("id should contain atom id");

        let link_result = server
            .link(Parameters(LinkRequest {
                source: bare_id.to_string(),
                target: second_bare.to_string(),
            }))
            .await
            .expect("link should succeed");
        let link_json = extract_json_text(&link_result);
        assert!(
            link_json
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .starts_with(&format!("{}/{}", org, project)),
            "link source should include activated org/project"
        );

        let unlink_result = server
            .unlink(Parameters(LinkRequest {
                source: bare_id.to_string(),
                target: second_bare.to_string(),
            }))
            .await
            .expect("unlink should succeed");
        let unlink_json = extract_json_text(&unlink_result);
        assert!(
            unlink_json
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .starts_with(&format!("{}/{}", org, project)),
            "unlink source should include activated org/project"
        );

        // delete should resolve bare id against activated project.
        let delete_result = server
            .delete(Parameters(DeleteAtomRequest {
                id: bare_id.to_string(),
            }))
            .await
            .expect("delete should succeed");
        let delete_json = extract_json_text(&delete_result);
        assert_eq!(
            delete_json.get("deleted").and_then(|v| v.as_bool()),
            Some(true)
        );

        // Restore env.
        match orig_storage {
            Some(val) => std::env::set_var("ATLAS_STORAGE", val),
            None => std::env::remove_var("ATLAS_STORAGE"),
        }
        match orig_org {
            Some(val) => std::env::set_var("ATLAS_ORG", val),
            None => std::env::remove_var("ATLAS_ORG"),
        }
        match orig_project {
            Some(val) => std::env::set_var("ATLAS_PROJECT", val),
            None => std::env::remove_var("ATLAS_PROJECT"),
        }

        let _ = fs::remove_dir_all(storage_dir);
    }
}

#[rmcp::tool_handler]
impl rmcp::handler::server::ServerHandler for AtlasServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "atlas".into(),
                title: Some("Atlas MCP".into()),
                version: env!("CARGO_PKG_VERSION").into(),
                icons: None,
                website_url: None,
            },
            instructions: Some(INSTRUCTIONS.into()),
        }
    }
}

fn to_mcp_error(e: crate::error::AtlasError) -> McpError {
    McpError::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None)
}
