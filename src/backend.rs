//! Backend struct for the DomainForge Language Server.
//!
//! The Backend holds server state and implements the `LanguageServer` trait from tower-lsp.
//! It maintains document content in memory and delegates validation/formatting to sea-core.

use std::collections::HashMap;

use sea_core::parse_to_graph;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::diagnostics::parse_error_to_diagnostic;
use crate::formatting::{extract_format_options, format_document, LspFormatConfig};

/// Server-side configuration for DomainForge.
///
/// This matches the configuration schema defined in the VS Code extension's
/// package.json contributes.configuration section.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainForgeConfig {
    /// Formatting configuration
    #[serde(default)]
    pub formatting: FormattingConfig,
}

/// Formatting-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormattingConfig {
    /// Number of spaces per indent level (default: 4)
    #[serde(default = "default_indent_width")]
    pub indent_width: usize,
    /// Use tabs instead of spaces (default: false)
    #[serde(default)]
    pub use_tabs: bool,
    /// Preserve comments in output (default: true)
    #[serde(default = "default_true")]
    pub preserve_comments: bool,
    /// Sort imports alphabetically (default: true)
    #[serde(default = "default_true")]
    pub sort_imports: bool,
}

fn default_indent_width() -> usize {
    4
}

fn default_true() -> bool {
    true
}

impl Default for FormattingConfig {
    fn default() -> Self {
        Self {
            indent_width: default_indent_width(),
            use_tabs: false,
            preserve_comments: true,
            sort_imports: true,
        }
    }
}

impl From<&FormattingConfig> for LspFormatConfig {
    fn from(config: &FormattingConfig) -> Self {
        LspFormatConfig {
            indent_width: config.indent_width,
            use_tabs: config.use_tabs,
        }
    }
}

/// The Backend struct holds server state.
///
/// # State
/// - `client`: The LSP client handle for sending notifications
/// - `documents`: In-memory storage of open document contents
/// - `config`: Server configuration synced from the client
pub struct Backend {
    /// The LSP client handle for sending diagnostics and other notifications
    client: Client,
    /// In-memory storage of open document contents, keyed by document URI
    documents: RwLock<HashMap<Url, String>>,
    /// Server configuration, updated via workspace/didChangeConfiguration
    config: RwLock<DomainForgeConfig>,
}

impl Backend {
    /// Create a new Backend instance with the given client handle.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: RwLock::new(HashMap::new()),
            config: RwLock::new(DomainForgeConfig::default()),
        }
    }

    /// Validate a document and publish diagnostics.
    ///
    /// Parses the document using sea-core and converts any parse errors
    /// into LSP diagnostics that are published to the client.
    async fn validate_document(&self, uri: Url, text: &str) {
        let diagnostics = match parse_to_graph(text) {
            Ok(_graph) => {
                // Parse succeeded - no diagnostics
                log::debug!("Document validated successfully: {}", uri);
                vec![]
            }
            Err(parse_error) => {
                // Parse failed - convert error to diagnostic
                log::debug!("Parse error in {}: {:?}", uri, parse_error);
                vec![parse_error_to_diagnostic(&parse_error)]
            }
        };

        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    /// Get the current formatting configuration.
    async fn get_format_config(&self) -> LspFormatConfig {
        let config = self.config.read().await;
        LspFormatConfig::from(&config.formatting)
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

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;

        log::info!("Document saved: {}", uri);

        // Get the document content from storage (or from params if provided)
        let text = if let Some(text) = params.text {
            text
        } else {
            // Fetch from stored documents
            let documents = self.documents.read().await;
            match documents.get(&uri) {
                Some(content) => content.clone(),
                None => {
                    log::warn!("Document not found in storage: {}", uri);
                    return;
                }
            }
        };

        // Re-validate on save
        self.validate_document(uri, &text).await;
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        log::info!("Configuration changed");

        // Try to extract the domainforge configuration section
        if let Some(settings) = params.settings.as_object() {
            if let Some(domainforge) = settings.get("domainforge") {
                match serde_json::from_value::<DomainForgeConfig>(domainforge.clone()) {
                    Ok(new_config) => {
                        log::debug!("Updated configuration: {:?}", new_config);
                        let mut config = self.config.write().await;
                        *config = new_config;
                    }
                    Err(e) => {
                        log::warn!("Failed to parse configuration: {}", e);
                    }
                }
            }
        }
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;

        log::info!("Format document: {}", uri);

        // Get the document content
        let text = {
            let documents = self.documents.read().await;
            match documents.get(&uri) {
                Some(content) => content.clone(),
                None => {
                    log::warn!("Document not found for formatting: {}", uri);
                    return Ok(None);
                }
            }
        };

        // Get formatting config - prefer request options, fall back to server config
        let format_config = {
            // Extract from request options
            let config = extract_format_options(&params.options);

            // The LSP options always provide tab_size and insert_spaces from the editor.
            // Server config could override these in the future if needed, but for now
            // we respect the editor settings from the request.
            let _server_config = self.get_format_config().await;

            config
        };

        // Perform formatting
        let edits = format_document(&text, Some(format_config));

        if edits.is_empty() {
            log::debug!("No formatting changes needed for: {}", uri);
            Ok(Some(vec![]))
        } else {
            log::debug!("Returning {} format edit(s) for: {}", edits.len(), uri);
            Ok(Some(edits))
        }
    }
}
