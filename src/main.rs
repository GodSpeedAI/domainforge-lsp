//! DomainForge Language Server
//!
//! This is a thin wrapper around `sea-core` that provides Language Server Protocol support
//! for the SEA DSL. It handles JSON-RPC communication and delegates all actual work to sea-core.

mod backend;
mod capabilities;
mod diagnostics;
mod formatting;

use tower_lsp::{LspService, Server};

use crate::backend::Backend;

#[tokio::main]
async fn main() {
    env_logger::init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend::new(client));
    Server::new(stdin, stdout, socket).serve(service).await;
}
