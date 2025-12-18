//! DomainForge Model Context Protocol (MCP) Server
//!
//! This binary acts as a bridge between AI agents and the DomainForge LSP server.
//! It implements the MCP protocol over stdio and proxies requests (like hover)
//! to the LSP server/logic.

mod guardrails;
mod lsp_client;
mod tools;
mod transport;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the LSP server binary. If not provided, expects 'domainforge-lsp' in PATH.
    #[arg(long)]
    lsp_path: Option<String>,

    /// Root path of the workspace to analyze.
    #[arg(long)]
    workspace_root: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Args::parse();

    log::info!("Starting DomainForge MCP Server...");

    let lsp_path = args
        .lsp_path
        .unwrap_or_else(|| "domainforge-lsp".to_string());
    let client = lsp_client::LspClient::new(&lsp_path).await?;
    client.initialize(args.workspace_root.clone()).await?;

    log::info!("LSP Client initialized, entering loop...");

    // Initialize Guard
    let root_paths = if let Some(root) = &args.workspace_root {
        vec![std::path::PathBuf::from(root)]
    } else {
        vec![] // No root means stricter default? Or maybe allow nothing?
               // For now, empty list means nothing allowed if we strictly check.
               // But typically CWD might be implied. Let's stick to explicit root.
    };
    let guard = std::sync::Arc::new(crate::guardrails::Guard::new(root_paths));

    // Basic stdio loop
    crate::transport::run_stdio_loop(&client, guard).await?;

    Ok(())
}
