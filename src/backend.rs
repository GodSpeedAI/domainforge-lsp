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

/// State for a single document.
///
/// This struct holds both the source text and the parsed semantic graph,
/// enabling efficient hover and other language features without re-parsing.
#[derive(Debug, Clone)]
struct DocumentState {
    /// The full text content of the document
    text: String,
    /// The LSP document version number
    version: i32,
    /// The parsed semantic graph, if parsing succeeded
    graph: Option<sea_core::Graph>,
}

impl DocumentState {
    /// Create a new DocumentState from text and version.
    ///
    /// Attempts to parse the text into a Graph. If parsing fails,
    /// the graph field will be None.
    fn new(text: String, version: i32) -> Self {
        let graph = parse_to_graph(&text).ok();
        Self {
            text,
            version,
            graph,
        }
    }

    /// Update the document with new text and version.
    ///
    /// Re-parses the text and updates the cached graph.
    fn update(&mut self, text: String, version: i32) {
        self.text = text;
        self.version = version;
        self.graph = parse_to_graph(&self.text).ok();
    }
}

/// The Backend struct holds server state.
///
/// # State
/// - `client`: The LSP client handle for sending notifications
/// - `documents`: In-memory storage of open document contents and parsed graphs
/// - `config`: Server configuration synced from the client
pub struct Backend {
    /// The LSP client handle for sending diagnostics and other notifications
    client: Client,
    /// In-memory storage of open document state (text + parsed graph), keyed by document URI
    documents: RwLock<HashMap<Url, DocumentState>>,
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
    /// Uses the cached graph from DocumentState if available. If parsing failed,
    /// the error was already captured during DocumentState creation.
    async fn validate_document(&self, uri: Url, state: &DocumentState) {
        let diagnostics = if state.graph.is_some() {
            // Parse succeeded - no diagnostics
            log::debug!("Document validated successfully: {}", uri);
            vec![]
        } else {
            // Parse failed - re-parse to get the error for diagnostics
            // (We don't store the error in DocumentState to keep it simple)
            match parse_to_graph(&state.text) {
                Ok(_) => vec![], // Shouldn't happen, but handle gracefully
                Err(parse_error) => {
                    log::debug!("Parse error in {}: {:?}", uri, parse_error);
                    vec![parse_error_to_diagnostic(&parse_error)]
                }
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
        let version = params.text_document.version;

        log::info!("Document opened: {}", uri);

        // Create document state with parsed graph
        let state = DocumentState::new(text, version);

        // Validate and publish diagnostics
        self.validate_document(uri.clone(), &state).await;

        // Store the document state
        {
            let mut documents = self.documents.write().await;
            documents.insert(uri, state);
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;

        // We use full document sync, so there's exactly one change with the full content
        if let Some(change) = params.content_changes.into_iter().next() {
            let text = change.text;

            log::debug!("Document changed: {}", uri);

            // Update the document state
            let state = {
                let mut documents = self.documents.write().await;
                if let Some(doc_state) = documents.get_mut(&uri) {
                    doc_state.update(text, version);
                    doc_state.clone()
                } else {
                    // Document not found, create new state
                    let new_state = DocumentState::new(text, version);
                    documents.insert(uri.clone(), new_state.clone());
                    new_state
                }
            };

            // Re-validate and publish diagnostics
            self.validate_document(uri, &state).await;
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

        // Get the document state from storage
        let state = {
            let documents = self.documents.read().await;
            documents.get(&uri).cloned()
        };

        if let Some(state) = state {
            // Re-validate on save
            self.validate_document(uri, &state).await;
        } else {
            log::warn!("Document not found in storage: {}", uri);
        }
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
                Some(state) => state.text.clone(),
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
