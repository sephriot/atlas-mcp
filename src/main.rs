mod cli;
mod config;
mod context;
mod error;
mod locking;
mod models;
mod serde_helpers;
mod server;
mod storage;
mod tools;

use clap::Parser;
use rmcp::ServiceExt;

use cli::Commands;
use server::AtlasServer;

use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "atlas")]
#[command(about = "Atlas - Long-term memory for AI agents")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Storage path for knowledge atoms (default: ~/.atlas)
    #[arg(long, short = 's', global = true)]
    storage: Option<PathBuf>,

    /// Organization name (requires --project)
    #[arg(long, global = true, requires = "project")]
    org: Option<String>,

    /// Project name (requires --org)
    #[arg(long, global = true, requires = "org")]
    project: Option<String>,

    /// Project root directory for repo storage mode (where .atlas/ is created)
    #[arg(long, global = true)]
    project_root: Option<PathBuf>,

    /// Output as JSON instead of YAML
    #[arg(long, global = true)]
    json: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Set storage path via env var if provided via CLI
    if let Some(ref path) = cli.storage {
        std::env::set_var("ATLAS_STORAGE", path);
    }

    // Set org/project context via env vars if provided via CLI
    if let (Some(ref org), Some(ref project)) = (&cli.org, &cli.project) {
        std::env::set_var("ATLAS_ORG", org);
        std::env::set_var("ATLAS_PROJECT", project);
    }

    // Set project root for repo storage mode
    if let Some(ref path) = cli.project_root {
        std::env::set_var("ATLAS_PROJECT_ROOT", path);
    }

    match cli.command {
        None | Some(Commands::Serve { http: None }) => {
            // Default: run MCP server on stdio
            let server = AtlasServer::new();
            run_stdio_server(server).await
        }
        Some(Commands::Serve { http: Some(port) }) => {
            // HTTP mode
            std::env::set_var("ATLAS_HTTP_MODE", "1");
            let server = AtlasServer::new();
            run_http_server(server, port).await
        }
        Some(cmd) => {
            // CLI command mode
            cli::run(cmd, cli.json)
        }
    }
}

async fn run_stdio_server(server: AtlasServer) -> anyhow::Result<()> {
    let transport = rmcp::transport::io::stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;
    Ok(())
}

async fn run_http_server(_server: AtlasServer, port: u16) -> anyhow::Result<()> {
    use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
    use rmcp::transport::streamable_http_server::tower::{
        StreamableHttpServerConfig, StreamableHttpService,
    };
    use std::net::SocketAddr;
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    let ct = CancellationToken::new();
    let config = StreamableHttpServerConfig {
        sse_keep_alive: Some(std::time::Duration::from_secs(30)),
        sse_retry: None,
        stateful_mode: true,
        cancellation_token: ct.clone(),
    };

    let session_manager = Arc::new(LocalSessionManager::default());
    let service = StreamableHttpService::new(|| Ok(AtlasServer::new()), session_manager, config);

    let app = axum::Router::new().fallback_service(service);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    eprintln!("Atlas MCP server listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
