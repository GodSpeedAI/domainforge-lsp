//! Backend struct for the DomainForge Language Server.
//!
//! The Backend holds server state and implements the `LanguageServer` trait from tower-lsp.
//! It maintains document content in memory and delegates validation/formatting to sea-core.

use std::collections::HashMap;
use std::sync::RwLock;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

/// The Backend struct holds server state.
///
/// # State
/// - `client`: The LSP client handle for sending notifications
/// - `documents`: In-memory storage of open document contents
pub struct Backend {
    /// The LSP client handle for sending diagnostics and other notifications
    client: Client,
    /// In-memory storage of open document contents, keyed by document URI
    documents: RwLock<HashMap<Url, String>>,
}

impl Backend {
    /// Create a new Backend instance with the given client handle.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: RwLock::new(HashMap::new()),
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "domainforge-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            capabilities: crate::capabilities::server_capabilities(),
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        log::info!("DomainForge LSP initialized");
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}
