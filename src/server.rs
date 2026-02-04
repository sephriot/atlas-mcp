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
    delete_atom, enable_local_storage, get_atom, get_context_with_activation, link, list_atoms,
    list_projects_with_activation, search, unlink, upsert, DeleteAtomRequest,
    EnableLocalStorageRequest, GetAtomRequest, LinkRequest, ListAtomsRequest, SearchRequest,
    UpsertRequest,
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
- Array fields (tags, sources, links, pitfalls) MUST be JSON arrays, NOT strings
- CORRECT: tags: ["api", "rust"]
- WRONG: tags: "[\"api\", \"rust\"]""#;

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
        let results = search(params.0).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&results).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Create or update a knowledge atom. For updates, provide id as full path (org/project/id), project/id, or bare id. Omit id for new atoms. IMPORTANT: Array fields (tags, sources, links, pitfalls) must be JSON arrays, not stringified arrays. Correct: tags: [\"api\", \"rust\"]. Wrong: tags: \"[\\\"api\\\", \\\"rust\\\"]\""
    )]
    async fn upsert(&self, params: Parameters<UpsertRequest>) -> Result<CallToolResult, McpError> {
        let result = upsert(params.0).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Get full atom content by ID. Accepts org/project/id, project/id, or bare id (context fills gaps)."
    )]
    async fn get(&self, params: Parameters<GetAtomRequest>) -> Result<CallToolResult, McpError> {
        let atom = get_atom(params.0).map_err(to_mcp_error)?;
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
        let results = list_atoms(params.0).map_err(to_mcp_error)?;
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
        let result = delete_atom(params.0).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Create a directed link from source to target atom. Both must be in same org. Accepts org/project/id, project/id, or bare id."
    )]
    async fn link(&self, params: Parameters<LinkRequest>) -> Result<CallToolResult, McpError> {
        let result = link(params.0).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Remove a directed link from source to target atom. Target need not exist (allows cleaning dangling links). Accepts org/project/id, project/id, or bare id."
    )]
    async fn unlink(&self, params: Parameters<LinkRequest>) -> Result<CallToolResult, McpError> {
        let result = unlink(params.0).map_err(to_mcp_error)?;
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
