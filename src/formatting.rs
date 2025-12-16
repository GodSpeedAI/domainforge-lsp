//! Format handler for the DomainForge LSP.
//!
//! This module will provide document formatting using sea-core's formatter.
//! Implementation will be added in Phase 2 of the plan.

use tower_lsp::lsp_types::TextEdit;

/// Format a SEA document and return the text edits.
///
/// # Arguments
/// * `source` - The document source code to format
///
/// # Returns
/// A vector of text edits to apply. If the source has parse errors,
/// returns an empty vector (don't format broken code).
///
/// # Note
/// This is a placeholder. Full implementation will be added in Phase 2.
#[allow(dead_code)]
pub fn format_document(_source: &str) -> Vec<TextEdit> {
    // Placeholder - will be implemented in Phase 2
    // 1. Parse the source using sea_core::parser::parse_to_graph
    // 2. If parse fails, return empty (don't format broken code)
    // 3. Format using sea_core::formatter::format
    // 4. Return TextEdit replacing entire document with formatted output
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_placeholder_returns_empty() {
        let result = format_document("Entity \"Test\" {}");
        assert!(result.is_empty());
    }
}
