//! Server capability declarations for the DomainForge LSP.
//!
//! This module returns the `ServerCapabilities` struct that tells the client
//! which LSP features this server supports.

use tower_lsp::lsp_types::*;

/// Returns the server capabilities to be sent during initialization.
///
/// Currently declares:
/// - Text document sync (open/change/close)
/// - Document formatting (Phase 2)
///
/// Future phases will add:
/// - Completion
/// - Hover
/// - Go to definition
/// - Find references
pub fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        // Full document sync - receive entire document on each change
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                save: Some(SaveOptions::default().into()),
                ..Default::default()
            },
        )),
        // Document formatting (Phase 2)
        document_formatting_provider: Some(OneOf::Left(true)),
        // Placeholder for future capabilities
        // completion_provider: Some(CompletionOptions::default()),
        // hover_provider: Some(HoverProviderCapability::Simple(true)),
        // definition_provider: Some(OneOf::Left(true)),
        // references_provider: Some(OneOf::Left(true)),
        ..Default::default()
    }
}
