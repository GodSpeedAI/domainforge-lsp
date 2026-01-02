//! AST JSON export for the DomainForge LSP.
//!
//! This module provides a custom LSP request `sea/astJson` that returns
//! the AST JSON representation of a SEA document. The AST JSON preserves
//! source structure and line numbers, conforming to ast-v3.schema.json.
//!
//! # Usage
//!
//! The client sends a `sea/astJson` request with a document URI and receives
//! the AST JSON as a string (or structured JSON).

use serde::{Deserialize, Serialize};
use tower_lsp::lsp_types::Url;

/// Parameters for the `sea/astJson` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AstJsonParams {
    /// The document URI to get AST for.
    pub uri: Url,
    /// Whether to pretty-print the JSON (default: true).
    #[serde(default = "default_true")]
    pub pretty: bool,
}

fn default_true() -> bool {
    true
}

/// Response for the `sea/astJson` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AstJsonResponse {
    /// The AST JSON string.
    pub ast_json: String,
    /// Document version at time of AST generation.
    pub version: i32,
    /// Whether the document parsed successfully.
    pub success: bool,
    /// Error message if parsing failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Convert SEA source to AST JSON using sea-core.
///
/// This function parses the source and converts it to the schema-compliant
/// AST JSON format with tagged enums like `{"type": "Entity", ...}`.
pub fn source_to_ast_json(source: &str, pretty: bool) -> Result<String, String> {
    use sea_core::parser::{ast_schema, parse};

    let internal_ast = parse(source).map_err(|e| format!("Parse error: {}", e))?;
    let schema_ast: ast_schema::Ast = internal_ast.into();

    if pretty {
        serde_json::to_string_pretty(&schema_ast)
            .map_err(|e| format!("Serialization error: {}", e))
    } else {
        serde_json::to_string(&schema_ast).map_err(|e| format!("Serialization error: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_to_ast_json_entity() {
        let source = r#"Entity "Customer""#;
        let result = source_to_ast_json(source, true);
        assert!(result.is_ok());
        let json = result.unwrap();
        assert!(json.contains("\"type\": \"Entity\""));
        assert!(json.contains("\"Customer\""));
    }

    #[test]
    fn test_source_to_ast_json_with_namespace() {
        let source = r#"
@namespace "ecommerce"
@version "1.0.0"

Entity "Order" in sales
Resource "Money" currency
"#;
        let result = source_to_ast_json(source, false);
        assert!(result.is_ok(), "Parse failed: {:?}", result);
        let json = result.unwrap();
        assert!(json.contains("\"namespace\":\"ecommerce\""));
        assert!(json.contains("\"type\":\"Entity\""));
        assert!(json.contains("\"type\":\"Resource\""));
    }

    #[test]
    fn test_source_to_ast_json_parse_error() {
        let source = "Entity"; // Missing name
        let result = source_to_ast_json(source, true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Parse error"));
    }
}
