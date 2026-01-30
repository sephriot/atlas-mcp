use rmcp::{
    handler::server::tool::ToolRouter, handler::server::wrapper::Parameters, model::Extensions,
    model::*, tool, tool_router, ErrorData as McpError,
};

use crate::tools::{
    delete_atom, enable_local_storage, get_atom, get_context, link, list_atoms, list_projects,
    search, unlink, upsert, DeleteAtomRequest, EnableLocalStorageRequest, GetAtomRequest,
    LinkRequest, ListAtomsRequest, SearchRequest, UpsertRequest,
};

const INSTRUCTIONS: &str = r#"Atlas MCP - Long-term memory for AI agents.

WORKFLOW:
1. SEARCH first - Before any task, search for relevant patterns/gotchas/decisions
2. READ full atoms - Use get_atom for each relevant search result
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

CONTEXT: Automatically detected from git remote (org/project). Use get_context to verify.

CITATION: Reference atom IDs in your reasoning, e.g., [K-000042]

PARAMETER FORMAT:
- Array fields (tags, sources, links, pitfalls) MUST be JSON arrays, NOT strings
- CORRECT: tags: ["api", "rust"]
- WRONG: tags: "[\"api\", \"rust\"]""#;

/// Atlas MCP Server for centralized knowledge management.
#[derive(Clone)]
pub struct AtlasServer {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl AtlasServer {
    pub fn new() -> Self {
        let mut tool_router = Self::tool_router();

        // Remove enable_local_storage tool in HTTP mode (symlinks don't make sense remotely)
        if std::env::var("ATLAS_HTTP_MODE").is_ok() {
            tool_router.remove_route("enable_local_storage");
        }

        Self { tool_router }
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
    async fn get_atom(
        &self,
        params: Parameters<GetAtomRequest>,
    ) -> Result<CallToolResult, McpError> {
        let atom = get_atom(params.0).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&atom).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "List atoms with optional filtering by type, tags, and confidence. Optionally filter by scope (e.g., 'acme/backend'). Default: lists detected project only."
    )]
    async fn list_atoms(
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
    async fn delete_atom(
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
    async fn enable_local_storage(
        &self,
        params: Parameters<EnableLocalStorageRequest>,
    ) -> Result<CallToolResult, McpError> {
        let result = enable_local_storage(params.0).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(description = "List all projects across all organizations.")]
    async fn list_projects(&self) -> Result<CallToolResult, McpError> {
        let results = list_projects().map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&results).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Get detected project context (org, project, cwd). In HTTP mode, use X-Atlas-Org and X-Atlas-Project headers to override."
    )]
    async fn get_context(&self, extensions: Extensions) -> Result<CallToolResult, McpError> {
        let ctx = get_context(extensions).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&ctx).unwrap_or_default(),
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
