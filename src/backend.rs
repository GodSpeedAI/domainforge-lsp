//! Backend struct for the DomainForge Language Server.
//!
//! The Backend holds server state and implements the `LanguageServer` trait from tower-lsp.
//! It maintains document content in memory and delegates validation/formatting to sea-core.

use std::collections::HashMap;
use std::num::NonZeroUsize;

use sea_core::parse_to_graph;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use lru::LruCache;

use crate::completion;
use crate::diagnostics::parse_error_to_diagnostic;
use crate::formatting::{extract_format_options, format_document, LspFormatConfig};
use crate::hover::markdown_renderer;
use crate::hover::symbol_resolver::{build_hover_model, HoverBuildInput};
use crate::hover::{DetailLevel, HoverPlusParams, HoverPlusResponse};
use crate::line_index::LineIndex;
use crate::navigation;
use crate::semantic_index::SemanticIndex;

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
    /// Precomputed line index for fast position↔offset conversion
    line_index: LineIndex,
    /// The parsed semantic graph, if parsing succeeded
    graph: Option<sea_core::Graph>,
    /// Semantic index of definitions/references for navigation and hover
    semantic_index: Option<SemanticIndex>,
}

impl DocumentState {
    /// Create a new DocumentState from text and version.
    ///
    /// Attempts to parse the text into a Graph. If parsing fails,
    /// the graph field will be None.
    fn new(text: String, version: i32) -> Self {
        let graph = parse_to_graph(&text).ok();
        let semantic_index = Some(SemanticIndex::build(&text));
        Self {
            line_index: LineIndex::new(&text),
            text,
            version,
            graph,
            semantic_index,
        }
    }

    /// Update the document with new text and version.
    ///
    /// Re-parses the text and updates the cached graph.
    fn update(&mut self, text: String, version: i32) {
        self.text = text;
        self.version = version;
        self.graph = parse_to_graph(&self.text).ok();
        self.semantic_index = Some(SemanticIndex::build(&self.text));
        self.line_index = LineIndex::new(&self.text);
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

    hover_model_cache: Mutex<LruCache<HoverCacheKey, crate::hover::HoverModel>>,
    hover_markdown_cache: Mutex<LruCache<HoverCacheKey, String>>,
}

impl Backend {
    /// Create a new Backend instance with the given client handle.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: RwLock::new(HashMap::new()),
            config: RwLock::new(DomainForgeConfig::default()),
            hover_model_cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(512).expect("non-zero hover model cache size"),
            )),
            hover_markdown_cache: Mutex::new(LruCache::new(
                NonZeroUsize::new(256).expect("non-zero hover markdown cache size"),
            )),
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

    async fn config_hash(&self) -> String {
        let config = self.config.read().await;
        let Ok(bytes) = serde_json::to_vec(&*config) else {
            return "<unhashable-config>".to_string();
        };
        blake3::hash(&bytes).to_hex().to_string()
    }

    pub async fn hover_plus(&self, params: HoverPlusParams) -> Result<Option<HoverPlusResponse>> {
        let uri = params.text_document.uri;
        let detail_level = DetailLevel::parse(params.max_detail_level.as_deref());

        let Some(state) = ({
            let documents = self.documents.read().await;
            documents.get(&uri).cloned()
        }) else {
            return Ok(None);
        };

        let Some(index) = state.semantic_index.as_ref() else {
            return Ok(None);
        };

        let config_hash = self.config_hash().await;
        let model_key = HoverCacheKey::model(&uri, state.version, params.position, detail_level);

        if let Some(model) = self.hover_model_cache.lock().await.get(&model_key).cloned() {
            let markdown = if params.include_markdown {
                let markdown_key =
                    HoverCacheKey::markdown(&uri, state.version, params.position, detail_level);
                Some(self.hover_markdown_for(&markdown_key, &model).await)
            } else {
                None
            };
            return Ok(Some(HoverPlusResponse { model, markdown }));
        }

        let model = build_hover_model(HoverBuildInput {
            uri: &uri,
            document_version: state.version,
            position: params.position,
            config_hash: &config_hash,
            detail_level,
            line_index: &state.line_index,
            index,
            graph: state.graph.as_ref(),
        });

        let Some(mut model) = model else {
            return Ok(None);
        };

        enforce_json_limits(&mut model);

        self.hover_model_cache
            .lock()
            .await
            .put(model_key, model.clone());

        let markdown = if params.include_markdown {
            let markdown_key =
                HoverCacheKey::markdown(&uri, state.version, params.position, detail_level);
            Some(self.hover_markdown_for(&markdown_key, &model).await)
        } else {
            None
        };

        Ok(Some(HoverPlusResponse { model, markdown }))
    }

    async fn hover_markdown_for(
        &self,
        key: &HoverCacheKey,
        model: &crate::hover::HoverModel,
    ) -> String {
        if let Some(markdown) = self.hover_markdown_cache.lock().await.get(key).cloned() {
            return markdown;
        }

        let rendered = markdown_renderer::render_markdown(model);
        if !rendered.truncated_sections.is_empty() {
            log::debug!(
                "Hover markdown truncated sections: {:?}",
                rendered.truncated_sections
            );
        }
        let markdown = rendered.markdown;
        self.hover_markdown_cache
            .lock()
            .await
            .put(key.clone(), markdown.clone());
        markdown
    }
}

fn enforce_json_limits(model: &mut crate::hover::HoverModel) {
    let max = model.limits.max_json_bytes;
    let mut bytes = serde_json::to_vec(model).unwrap_or_default().len();
    if bytes <= max {
        return;
    }

    model.limits.truncated_sections.push("json".to_string());

    // Deterministic, loss-first truncation to fit the payload limit.
    model.related.clear();
    bytes = serde_json::to_vec(model).unwrap_or_default().len();
    if bytes <= max {
        return;
    }

    model.primary.facts.clear();
    bytes = serde_json::to_vec(model).unwrap_or_default().len();
    if bytes <= max {
        return;
    }

    if model.primary.summary.len() > 512 {
        model.primary.summary.truncate(512);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct HoverCacheKey {
    uri: String,
    version: i32,
    line: u32,
    character: u32,
    detail_level: DetailLevel,
    view_kind: ViewKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ViewKind {
    Markdown,
    Json,
}

impl HoverCacheKey {
    fn model(uri: &Url, version: i32, position: Position, detail_level: DetailLevel) -> Self {
        Self {
            uri: uri.to_string(),
            version,
            line: position.line,
            character: position.character,
            detail_level,
            view_kind: ViewKind::Json,
        }
    }

    fn markdown(uri: &Url, version: i32, position: Position, detail_level: DetailLevel) -> Self {
        Self {
            uri: uri.to_string(),
            version,
            line: position.line,
            character: position.character,
            detail_level,
            view_kind: ViewKind::Markdown,
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

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;

        let Some(state) = ({
            let documents = self.documents.read().await;
            documents.get(&uri).cloned()
        }) else {
            return Ok(None);
        };

        let response = completion::completion(
            &state.text,
            &state.line_index,
            position,
            state.graph.as_ref(),
            state.semantic_index.as_ref(),
        );
        Ok(response)
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let Some(state) = ({
            let documents = self.documents.read().await;
            documents.get(&uri).cloned()
        }) else {
            return Ok(None);
        };

        let Some(index) = state.semantic_index.as_ref() else {
            return Ok(None);
        };

        let config_hash = self.config_hash().await;
        let detail_level = DetailLevel::Standard;
        let model_key = HoverCacheKey::model(&uri, state.version, position, detail_level);

        if let Some(model) = self.hover_model_cache.lock().await.get(&model_key).cloned() {
            let markdown_key = HoverCacheKey::markdown(&uri, state.version, position, detail_level);
            let markdown = self.hover_markdown_for(&markdown_key, &model).await;
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: markdown,
                }),
                range: None,
            }));
        }

        let model = build_hover_model(HoverBuildInput {
            uri: &uri,
            document_version: state.version,
            position,
            config_hash: &config_hash,
            detail_level,
            line_index: &state.line_index,
            index,
            graph: state.graph.as_ref(),
        });

        let Some(model) = model else {
            return Ok(None);
        };

        self.hover_model_cache
            .lock()
            .await
            .put(model_key, model.clone());

        let markdown_key = HoverCacheKey::markdown(&uri, state.version, position, detail_level);
        let markdown = self.hover_markdown_for(&markdown_key, &model).await;
        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: markdown,
            }),
            range: None,
        }))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let Some(state) = ({
            let documents = self.documents.read().await;
            documents.get(&uri).cloned()
        }) else {
            return Ok(None);
        };
        let Some(index) = state.semantic_index.as_ref() else {
            return Ok(None);
        };

        let location = navigation::goto_definition(&uri, &state.line_index, position, index);
        Ok(location.map(GotoDefinitionResponse::Scalar))
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let include_declaration = params.context.include_declaration;

        let Some(state) = ({
            let documents = self.documents.read().await;
            documents.get(&uri).cloned()
        }) else {
            return Ok(None);
        };
        let Some(index) = state.semantic_index.as_ref() else {
            return Ok(None);
        };

        let locations = navigation::find_references(
            &uri,
            &state.line_index,
            position,
            index,
            include_declaration,
        );
        Ok(Some(locations))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hover::*;
    use tower_lsp::LspService;

    #[test]
    fn hover_plus_json_is_capped_deterministically() {
        let mut model = HoverModel {
            schema_version: "1.0".to_string(),
            id: "id".to_string(),
            symbol: HoverSymbol {
                name: "X".to_string(),
                kind: "Entity".to_string(),
                qualified_name: "default::X".to_string(),
                uri: "file:///test".to_string(),
                range: HoverRange {
                    start: HoverPosition {
                        line: 0,
                        character: 0,
                    },
                    end: HoverPosition {
                        line: 0,
                        character: 1,
                    },
                },
                resolve_id: "rid".to_string(),
                resolution_confidence: "exact".to_string(),
            },
            context: HoverContext {
                document_version: 1,
                position: HoverPosition {
                    line: 0,
                    character: 0,
                },
                scope_summary: HoverScopeSummary {
                    module: None,
                    enclosing_rule: None,
                    namespaces_in_scope: vec![],
                },
                config_hash: "cfg".to_string(),
            },
            primary: HoverPrimary {
                header: HoverHeader {
                    display_name: "X".to_string(),
                    kind_label: "Entity".to_string(),
                    qualified_path: "default::X".to_string(),
                },
                signature_or_shape: "Entity \"X\"".to_string(),
                summary: "a".repeat(200_000),
                badges: vec![],
                facts: (0..500)
                    .map(|i| (format!("k{i:03}"), "v".repeat(64)))
                    .collect(),
            },
            related: (0..1000)
                .map(|i| HoverRelated {
                    qualified_name: format!("default::R{i:03}"),
                    kind: "Resource".to_string(),
                    relevance_score: 1,
                })
                .collect(),
            limits: HoverLimits {
                max_markdown_bytes: 1024,
                max_json_bytes: 2048,
                truncated_sections: vec![],
            },
        };

        enforce_json_limits(&mut model);
        let bytes = serde_json::to_vec(&model).unwrap().len();
        assert!(bytes <= 2048, "json bytes should be capped, got {}", bytes);
        assert!(
            model
                .limits
                .truncated_sections
                .contains(&"json".to_string()),
            "should mark json truncation"
        );
    }

    #[tokio::test]
    async fn hover_plus_include_markdown_parameter_returns_markdown() {
        let (service, _socket) = LspService::new(Backend::new);
        let backend = service.inner();

        let uri = Url::parse("file:///test.sea").unwrap();
        let source = r#"
Entity "Warehouse"
Entity "Factory"
Resource "Cameras" units
Flow "Cameras" from "Warehouse" to "Factory" quantity 10
"#;
        let line_index = crate::line_index::LineIndex::new(source);
        let offset = source.find("\"Warehouse\"").unwrap() + 2;
        let position = line_index.position_of(offset);

        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "domainforge".to_string(),
                    version: 1,
                    text: source.to_string(),
                },
            })
            .await;

        let resp = backend
            .hover_plus(HoverPlusParams {
                text_document: HoverTextDocumentIdentifier { uri },
                position,
                include_markdown: true,
                include_project_signals: false,
                max_detail_level: Some("standard".to_string()),
            })
            .await
            .unwrap()
            .unwrap();

        assert!(resp.markdown.is_some());
        assert!(resp.model.schema_version == "1.0");
    }
}
