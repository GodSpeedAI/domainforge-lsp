//! Diagnostic mapping utilities for the DomainForge LSP.
//!
//! This module provides functions to convert sea-core validation errors
//! into LSP diagnostics that can be displayed in the editor.

use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range};

/// Convert a sea-core source range to an LSP range.
///
/// **IMPORTANT**: sea-core uses 1-based line/column indexing,
/// while LSP uses 0-based indexing. This function MUST subtract 1
/// from both line and column values.
///
/// # Arguments
/// * `start_line` - 1-based start line from sea-core
/// * `start_col` - 1-based start column from sea-core
/// * `end_line` - 1-based end line from sea-core
/// * `end_col` - 1-based end column from sea-core
#[allow(dead_code)]
pub fn sea_range_to_lsp_range(
    start_line: usize,
    start_col: usize,
    end_line: usize,
    end_col: usize,
) -> Range {
    Range {
        start: Position {
            line: start_line.saturating_sub(1) as u32,
            character: start_col.saturating_sub(1) as u32,
        },
        end: Position {
            line: end_line.saturating_sub(1) as u32,
            character: end_col.saturating_sub(1) as u32,
        },
    }
}

/// Create an error diagnostic at the given range.
///
/// # Arguments
/// * `range` - The LSP range where the error occurs
/// * `message` - The error message to display
/// * `code` - The error code (e.g., "E001" for undefined entity)
#[allow(dead_code)]
pub fn error_diagnostic(range: Range, message: String, code: String) -> Diagnostic {
    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(code)),
        source: Some("domainforge".to_string()),
        message,
        ..Default::default()
    }
}

/// Create a warning diagnostic at the given range.
#[allow(dead_code)]
pub fn warning_diagnostic(range: Range, message: String, code: String) -> Diagnostic {
    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(NumberOrString::String(code)),
        source: Some("domainforge".to_string()),
        message,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_conversion_1_based_to_0_based() {
        // sea-core reports line 1, column 1 (first character in file)
        // LSP expects line 0, character 0
        let range = sea_range_to_lsp_range(1, 1, 1, 10);
        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 0);
        assert_eq!(range.end.line, 0);
        assert_eq!(range.end.character, 9);
    }

    #[test]
    fn test_range_conversion_multiline() {
        // sea-core reports lines 5-7
        // LSP expects lines 4-6
        let range = sea_range_to_lsp_range(5, 1, 7, 20);
        assert_eq!(range.start.line, 4);
        assert_eq!(range.end.line, 6);
    }

    #[test]
    fn test_error_diagnostic_creation() {
        let range = sea_range_to_lsp_range(1, 1, 1, 10);
        let diag = error_diagnostic(range, "Undefined entity".to_string(), "E001".to_string());

        assert_eq!(diag.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diag.code, Some(NumberOrString::String("E001".to_string())));
        assert_eq!(diag.source, Some("domainforge".to_string()));
    }
}
