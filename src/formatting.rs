//! Format handler for the DomainForge LSP.
//!
//! This module provides document formatting using sea-core's formatter.

use sea_core::formatter::{format, FormatConfig, IndentStyle};
use tower_lsp::lsp_types::{Position, Range, TextEdit};

/// Configuration for formatting, derived from LSP formatting options.
#[derive(Debug, Clone)]
pub struct LspFormatConfig {
    /// Number of spaces per indent level
    pub indent_width: usize,
    /// Use tabs instead of spaces
    pub use_tabs: bool,
}

impl Default for LspFormatConfig {
    fn default() -> Self {
        Self {
            indent_width: 4,
            use_tabs: false,
        }
    }
}

impl From<LspFormatConfig> for FormatConfig {
    fn from(lsp_config: LspFormatConfig) -> Self {
        FormatConfig {
            indent_width: lsp_config.indent_width,
            indent_style: if lsp_config.use_tabs {
                IndentStyle::Tabs
            } else {
                IndentStyle::Spaces
            },
            ..Default::default()
        }
    }
}

/// Format a SEA document and return the text edits.
///
/// # Arguments
/// * `source` - The document source code to format
/// * `config` - Optional formatting configuration (uses defaults if None)
///
/// # Returns
/// A vector of text edits to apply. If the source has parse errors,
/// returns an empty vector (don't format broken code).
pub fn format_document(source: &str, config: Option<LspFormatConfig>) -> Vec<TextEdit> {
    let format_config: FormatConfig = config.unwrap_or_default().into();

    match format(source, format_config) {
        Ok(formatted) => {
            // If the formatted output is identical, no edits needed
            if formatted == source {
                return vec![];
            }

            // Replace entire document with formatted content
            // Calculate the end position based on source content
            let lines: Vec<&str> = source.lines().collect();
            let end_line = if lines.is_empty() { 0 } else { lines.len() - 1 };
            let end_char = lines.last().map(|l| l.len()).unwrap_or(0);

            // Handle case where source ends with newline but lines() doesn't include it
            let (final_line, final_char) = if source.ends_with('\n') {
                (lines.len() as u32, 0)
            } else {
                (end_line as u32, end_char as u32)
            };

            vec![TextEdit {
                range: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: final_line,
                        character: final_char,
                    },
                },
                new_text: formatted,
            }]
        }
        Err(e) => {
            // Log the error but return empty edits - don't format broken code
            log::warn!("Format error: {}", e);
            vec![]
        }
    }
}

/// Extract formatting configuration from LSP formatting options.
///
/// # Arguments
/// * `options` - The FormattingOptions from the LSP request
///
/// # Returns
/// An LspFormatConfig with the extracted settings
pub fn extract_format_options(
    options: &tower_lsp::lsp_types::FormattingOptions,
) -> LspFormatConfig {
    LspFormatConfig {
        indent_width: options.tab_size as usize,
        use_tabs: !options.insert_spaces,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_valid_sea_returns_edit() {
        // Poorly formatted input
        let source = r#"Entity   "Test"    in   domain"#;
        let result = format_document(source, None);

        assert!(!result.is_empty(), "Should return a text edit");
        assert_eq!(result.len(), 1, "Should return exactly one edit");

        let edit = &result[0];
        // The edit should cover the entire document
        assert_eq!(edit.range.start.line, 0);
        assert_eq!(edit.range.start.character, 0);
        // Should have formatted content
        assert!(
            edit.new_text.contains("Entity \"Test\""),
            "Should contain normalized Entity"
        );
    }

    #[test]
    fn test_format_malformed_sea_returns_empty() {
        // Invalid SEA syntax - missing closing quote
        let source = r#"Entity "Broken"#;
        let result = format_document(source, None);

        assert!(
            result.is_empty(),
            "Should return empty edits for malformed code"
        );
    }

    #[test]
    fn test_format_with_tabs() {
        let source = r#"
Relation "Test"
    subject: "A"
    predicate: "rel"
    object: "B"
"#;
        let config = LspFormatConfig {
            indent_width: 4,
            use_tabs: true,
        };
        let result = format_document(source, Some(config));

        assert!(!result.is_empty(), "Should return a text edit");
        let formatted = &result[0].new_text;
        assert!(formatted.contains('\t'), "Should use tabs for indentation");
    }

    #[test]
    fn test_format_with_custom_indent_width() {
        let source = r#"
Relation "Test"
    subject: "A"
    predicate: "rel"
    object: "B"
"#;
        let config = LspFormatConfig {
            indent_width: 2,
            use_tabs: false,
        };
        let result = format_document(source, Some(config));

        assert!(!result.is_empty(), "Should return a text edit");
        let formatted = &result[0].new_text;
        // With indent width 2, we should see 2-space indentation
        assert!(
            formatted.contains("  subject:"),
            "Should use 2-space indentation"
        );
    }

    #[test]
    fn test_format_already_formatted_returns_empty() {
        // Already well-formatted content - sea-core's format output
        let source = "Entity \"Test\" in domain\n";
        let result = format_document(source, None);

        // If the source is already formatted, we might get empty or the same content
        // The important thing is no unnecessary changes
        if !result.is_empty() {
            assert_eq!(
                result[0].new_text, source,
                "Should not change formatted code"
            );
        }
    }

    #[test]
    fn test_extract_format_options() {
        use tower_lsp::lsp_types::FormattingOptions;

        let options = FormattingOptions {
            tab_size: 2,
            insert_spaces: true,
            ..Default::default()
        };

        let config = extract_format_options(&options);
        assert_eq!(config.indent_width, 2);
        assert!(!config.use_tabs);

        let options_tabs = FormattingOptions {
            tab_size: 4,
            insert_spaces: false,
            ..Default::default()
        };

        let config_tabs = extract_format_options(&options_tabs);
        assert_eq!(config_tabs.indent_width, 4);
        assert!(config_tabs.use_tabs);
    }

    #[test]
    fn test_lsp_format_config_to_sea_core_config() {
        let lsp_config = LspFormatConfig {
            indent_width: 2,
            use_tabs: true,
        };

        let sea_config: FormatConfig = lsp_config.into();
        assert_eq!(sea_config.indent_width, 2);
        assert_eq!(sea_config.indent_style, IndentStyle::Tabs);
    }
}
