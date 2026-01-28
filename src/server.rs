use rmcp::{
    handler::server::tool::ToolRouter, handler::server::wrapper::Parameters, model::*,
    tool, tool_router, ErrorData as McpError,
};

use crate::tools::{
    delete_atom, get_atom, get_context, init_project, list_atoms, list_projects, search, upsert,
    DeleteAtomRequest, GetAtomRequest, InitProjectRequest, ListAtomsRequest, SearchRequest,
    UpsertRequest,
};

const INSTRUCTIONS: &str = r#"Atlas MCP - Long-term memory for AI agents.

WORKFLOW:
1. SEARCH first - Before any task, search for relevant patterns/gotchas/decisions
2. READ full atoms - Use get_atom for each relevant search result
3. APPLY knowledge - Let retrieved atoms constrain your approach
4. RECORD learnings - Use upsert when you discover something reusable

ATOM TYPES:
- note: Facts, observations, domain knowledge
- gotcha: Warnings, pitfalls, "watch out for X"
- recipe: Patterns, how-to guides, code snippets
- decision: Architectural rationale, why X over Y

CONTEXT: Automatically detected from git remote (org/project). Use get_context to verify.

CITATION: Reference atom IDs in your reasoning, e.g., [K-000042]"#;

/// Atlas MCP Server for centralized knowledge management.
#[derive(Clone)]
pub struct AtlasServer {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl AtlasServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Search knowledge atoms by title, tags, type, and confidence. Returns matching atoms with relevance scores.")]
    async fn search(
        &self,
        params: Parameters<SearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        let results = search(params.0).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&results).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Create or update a knowledge atom. Provide id to update existing, omit for new atom.")]
    async fn upsert(
        &self,
        params: Parameters<UpsertRequest>,
    ) -> Result<CallToolResult, McpError> {
        let result = upsert(params.0).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Get full atom content by ID. Supports cross-project within org using project/id format.")]
    async fn get_atom(
        &self,
        params: Parameters<GetAtomRequest>,
    ) -> Result<CallToolResult, McpError> {
        let atom = get_atom(params.0).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&atom).unwrap_or_default(),
        )]))
    }

    #[tool(description = "List atoms with optional filtering by type, tags, and confidence.")]
    async fn list_atoms(
        &self,
        params: Parameters<ListAtomsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let results = list_atoms(params.0).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&results).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Delete an atom by ID.")]
    async fn delete_atom(
        &self,
        params: Parameters<DeleteAtomRequest>,
    ) -> Result<CallToolResult, McpError> {
        let result = delete_atom(params.0).map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Initialize a new project. Creates org/project directory structure and optionally a .knowledge symlink.")]
    async fn init(
        &self,
        params: Parameters<InitProjectRequest>,
    ) -> Result<CallToolResult, McpError> {
        let result = init_project(params.0).map_err(to_mcp_error)?;
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

    #[tool(description = "Get detected project context (org, project, cwd) based on git remote or .knowledge symlink.")]
    async fn get_context(&self) -> Result<CallToolResult, McpError> {
        let ctx = get_context().map_err(to_mcp_error)?;
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
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation {
                name: "atlas-mcp".into(),
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
