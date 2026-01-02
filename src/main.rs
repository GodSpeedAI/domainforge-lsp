//! DomainForge Language Server
//!
//! This is a thin wrapper around `sea-core` that provides Language Server Protocol support
//! for the SEA DSL. It handles JSON-RPC communication and delegates all actual work to sea-core.

use tower_lsp::{LspService, Server};

use domainforge_lsp::backend::Backend;

#[tokio::main]
async fn main() {
    env_logger::init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(Backend::new)
        .custom_method("textDocument/hoverPlus", Backend::hover_plus)
        .custom_method("sea/astJson", Backend::get_ast_json)
        .finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}
