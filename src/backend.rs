//! Backend struct for the DomainForge Language Server.
//!
//! The Backend holds server state and implements the `LanguageServer` trait from tower-lsp.
//! It maintains document content in memory and delegates validation/formatting to sea-core.

use std::collections::HashMap;

use tokio::sync::RwLock;
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

    /// Validate a document and publish diagnostics.
    /// This will be enhanced in Phase 1 to call sea-core validation.
    async fn validate_document(&self, uri: Url, _text: &str) {
        // Placeholder: In Phase 1, this will:
        // 1. Parse with sea_core::parser::parse_to_graph
        // 2. Convert errors to diagnostics using crate::diagnostics
        // 3. Publish diagnostics

        // For now, just clear any existing diagnostics
        self.client.publish_diagnostics(uri, vec![], None).await;
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

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;

        log::info!("Document opened: {}", uri);

        // Store the document content
        {
            let mut documents = self.documents.write().await;
            documents.insert(uri.clone(), text.clone());
        }

        // Validate and publish diagnostics
        self.validate_document(uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;

        // We use full document sync, so there's exactly one change with the full content
        if let Some(change) = params.content_changes.into_iter().next() {
            let text = change.text;

            log::debug!("Document changed: {}", uri);

            // Update the stored document content
            {
                let mut documents = self.documents.write().await;
                documents.insert(uri.clone(), text.clone());
            }

            // Re-validate and publish diagnostics
            self.validate_document(uri, &text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;

        log::info!("Document closed: {}", uri);

        // Remove the document from storage
        {
            let mut documents = self.documents.write().await;
            documents.remove(&uri);
        }

        // Clear diagnostics for the closed document
        self.client.publish_diagnostics(uri, vec![], None).await;
    }
}
