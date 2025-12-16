pub mod markdown_renderer;
pub mod symbol_resolver;

use serde::{Deserialize, Serialize};
use tower_lsp::lsp_types::{Position, Url};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailLevel {
    Core,
    Standard,
    Deep,
}

impl DetailLevel {
    pub fn parse(s: Option<&str>) -> Self {
        match s.unwrap_or("standard") {
            "core" => Self::Core,
            "deep" => Self::Deep,
            _ => Self::Standard,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverModel {
    pub schema_version: String,
    pub id: String,
    pub symbol: HoverSymbol,
    pub context: HoverContext,
    pub primary: HoverPrimary,
    pub related: Vec<HoverRelated>,
    pub limits: HoverLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverSymbol {
    pub name: String,
    pub kind: String,
    pub qualified_name: String,
    pub uri: String,
    pub range: HoverRange,
    pub resolve_id: String,
    pub resolution_confidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverContext {
    pub document_version: i32,
    pub position: HoverPosition,
    pub scope_summary: HoverScopeSummary,
    pub config_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverScopeSummary {
    pub module: Option<String>,
    pub enclosing_rule: Option<String>,
    pub namespaces_in_scope: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverPrimary {
    pub header: HoverHeader,
    pub signature_or_shape: String,
    pub summary: String,
    pub badges: Vec<String>,
    pub facts: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverHeader {
    pub display_name: String,
    pub kind_label: String,
    pub qualified_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverRelated {
    pub qualified_name: String,
    pub kind: String,
    pub relevance_score: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverLimits {
    pub max_markdown_bytes: usize,
    pub max_json_bytes: usize,
    pub truncated_sections: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverPlusParams {
    pub text_document: HoverTextDocumentIdentifier,
    pub position: Position,
    #[serde(default)]
    pub include_markdown: bool,
    #[serde(default)]
    pub include_project_signals: bool,
    pub max_detail_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverTextDocumentIdentifier {
    pub uri: Url,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverPlusResponse {
    pub model: HoverModel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverRange {
    pub start: HoverPosition,
    pub end: HoverPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverPosition {
    pub line: u32,
    pub character: u32,
}

impl From<Position> for HoverPosition {
    fn from(pos: Position) -> Self {
        Self {
            line: pos.line,
            character: pos.character,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_plus_response_serializes_with_required_fields() {
        let model = HoverModel {
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
                summary: "summary".to_string(),
                badges: vec![],
                facts: vec![],
            },
            related: vec![],
            limits: HoverLimits {
                max_markdown_bytes: 1024,
                max_json_bytes: 1024,
                truncated_sections: vec![],
            },
        };

        let resp = HoverPlusResponse {
            model,
            markdown: Some("md".to_string()),
        };

        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"symbol\""));
        assert!(json.contains("\"primary\""));
        assert!(json.contains("\"limits\""));
        assert!(json.contains("\"markdown\""));
    }
}
