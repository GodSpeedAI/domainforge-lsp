use serde_json::Value;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range,
};

use sea_core::semantic_pack::{
    DiagnosticSeverity as SeaDiagnosticSeverity, SemanticDiagnostic, SemanticTruth,
};

use crate::line_index::LineIndex;

pub fn semantic_diagnostic_to_lsp(
    diag: &SemanticDiagnostic,
    line_index: &LineIndex,
) -> Diagnostic {
    let start = line_index.position_of(diag.source_ref.start_byte);
    let end = line_index.position_of(diag.source_ref.end_byte);

    let range = if diag.source_ref.start_byte == 0 && diag.source_ref.end_byte == 0 {
        Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        }
    } else {
        Range { start, end }
    };

    let severity = map_severity(diag.severity);

    let data = Some(Value::String(serde_json::to_string(&diag.semantic_truth).unwrap_or_default()));

    Diagnostic {
        range,
        severity: Some(severity),
        code: Some(NumberOrString::String(format!(
            "S{}",
            diag.code.as_str()
        ))),
        source: Some("domainforge-semantic".to_string()),
        message: diag.message.clone(),
        data,
        ..Default::default()
    }
}

pub fn create_pack_diagnostic(message: &str, code: &str) -> Diagnostic {
    Diagnostic {
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        },
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(NumberOrString::String(format!("S{}", code))),
        source: Some("domainforge-semantic".to_string()),
        message: message.to_string(),
        data: Some(Value::String(
            serde_json::to_string(&SemanticTruth::Unknown).unwrap_or_default(),
        )),
        ..Default::default()
    }
}

fn map_severity(severity: SeaDiagnosticSeverity) -> DiagnosticSeverity {
    match severity {
        SeaDiagnosticSeverity::Error => DiagnosticSeverity::ERROR,
        SeaDiagnosticSeverity::Warning => DiagnosticSeverity::WARNING,
        SeaDiagnosticSeverity::Info => DiagnosticSeverity::INFORMATION,
        SeaDiagnosticSeverity::Hint => DiagnosticSeverity::HINT,
    }
}
