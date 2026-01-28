mod config;
mod context;
mod error;
mod locking;
mod models;
mod server;
mod storage;
mod tools;

use clap::Parser;
use rmcp::ServiceExt;

use server::AtlasServer;

#[derive(Parser, Debug)]
#[command(name = "atlas-mcp")]
#[command(about = "Atlas MCP - Centralized knowledge management server")]
#[command(version)]
struct Args {
    /// Run HTTP/SSE server on specified port (localhost only)
    #[arg(long)]
    http: Option<u16>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let server = AtlasServer::new();

    if let Some(port) = args.http {
        run_http_server(server, port).await
    } else {
        run_stdio_server(server).await
    }
}

async fn run_stdio_server(server: AtlasServer) -> anyhow::Result<()> {
    let transport = rmcp::transport::io::stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;
    Ok(())
}

async fn run_http_server(_server: AtlasServer, port: u16) -> anyhow::Result<()> {
    use rmcp::transport::streamable_http_server::tower::{
        StreamableHttpServerConfig, StreamableHttpService,
    };
    use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
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
    let service = StreamableHttpService::new(
        || Ok(AtlasServer::new()),
        session_manager,
        config,
    );

    let app = axum::Router::new().nest_service("/", service);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    eprintln!("Atlas MCP server listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
