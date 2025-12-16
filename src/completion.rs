use sea_core::Graph;
use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, CompletionResponse, Position};

use crate::line_index::LineIndex;
use crate::semantic_index::SemanticIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletionContext {
    Any,
    EntityName,
    ResourceName,
    InstanceRef,
    ImportPrefix,
}

pub fn completion(
    source: &str,
    line_index: &LineIndex,
    position: Position,
    graph: Option<&Graph>,
    index: Option<&SemanticIndex>,
) -> Option<CompletionResponse> {
    let offset = line_index.offset_of(position)?;
    let ctx = detect_context(source, line_index, offset);

    let mut items = Vec::new();
    if let Some(graph) = graph {
        if matches!(ctx, CompletionContext::Any | CompletionContext::EntityName) {
            for entity in graph.all_entities() {
                items.push(CompletionItem {
                    label: entity.name().to_string(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some("Entity".to_string()),
                    ..Default::default()
                });
            }
        }

        if matches!(
            ctx,
            CompletionContext::Any | CompletionContext::ResourceName
        ) {
            for res in graph.all_resources() {
                items.push(CompletionItem {
                    label: res.name().to_string(),
                    kind: Some(CompletionItemKind::CONSTANT),
                    detail: Some(format!("Resource ({})", res.unit().symbol())),
                    ..Default::default()
                });
            }
        }

        if matches!(ctx, CompletionContext::Any | CompletionContext::InstanceRef) {
            for inst in graph.all_entity_instances() {
                items.push(CompletionItem {
                    label: format!("@{}", inst.name()),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some(format!("Instance of {}", inst.entity_type())),
                    insert_text: Some(format!("@{}", inst.name())),
                    ..Default::default()
                });
            }
        }
    }

    if matches!(
        ctx,
        CompletionContext::Any | CompletionContext::ImportPrefix
    ) {
        if let Some(index) = index {
            for prefix in &index.import_prefixes {
                items.push(CompletionItem {
                    label: prefix.clone(),
                    kind: Some(CompletionItemKind::MODULE),
                    detail: Some("Import prefix".to_string()),
                    ..Default::default()
                });
            }
        }
    }

    items.sort_by(|a, b| {
        kind_rank(a.kind)
            .cmp(&kind_rank(b.kind))
            .then_with(|| a.label.cmp(&b.label))
    });
    items.dedup_by(|a, b| a.label == b.label && a.kind == b.kind);

    Some(CompletionResponse::Array(items))
}

fn kind_rank(kind: Option<CompletionItemKind>) -> u8 {
    match kind {
        Some(k) if k == CompletionItemKind::CLASS => 0,
        Some(k) if k == CompletionItemKind::CONSTANT => 1,
        Some(k) if k == CompletionItemKind::VARIABLE => 2,
        Some(k) if k == CompletionItemKind::MODULE => 3,
        _ => 9,
    }
}

fn detect_context(source: &str, line_index: &LineIndex, offset: usize) -> CompletionContext {
    let pos = line_index.position_of(offset);
    let line_start_offset = line_index.offset_of(Position {
        line: pos.line,
        character: 0,
    });
    let Some(line_start_offset) = line_start_offset else {
        return CompletionContext::Any;
    };
    let prefix = &source[line_start_offset.min(source.len())..offset.min(source.len())];
    let prefix_trimmed = prefix.trim_end();
    let lower = prefix_trimmed.to_ascii_lowercase();

    if lower.ends_with("@") {
        return CompletionContext::InstanceRef;
    }

    for needle in [" of \"", " from \"", " to \""] {
        if lower.ends_with(needle) {
            return CompletionContext::EntityName;
        }
    }
    if lower.ends_with("flow \"") {
        return CompletionContext::ResourceName;
    }
    if lower.ends_with("import * as ") || lower.ends_with("import {") {
        return CompletionContext::ImportPrefix;
    }

    CompletionContext::Any
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line_index::LineIndex;
    use crate::semantic_index::SemanticIndex;
    use std::collections::HashSet;

    #[test]
    fn suggests_entities_after_of_quote() {
        let source = r#"
Entity "Vendor"
Entity "Warehouse"

Instance vendor_123 of "Vendor"
"#;
        let graph = sea_core::parse_to_graph(source).unwrap();
        let line_index = LineIndex::new(source);
        let index = SemanticIndex::build(source);

        let offset = source.rfind("of \"Vendor\"").unwrap() + "of \"".len();
        let position = line_index.position_of(offset);

        let result = completion(source, &line_index, position, Some(&graph), Some(&index))
            .expect("completion response");
        let CompletionResponse::Array(items) = result else {
            panic!("expected array response");
        };
        assert!(
            items.iter().any(|i| i.label == "Vendor"),
            "should suggest entity names"
        );
        assert!(
            items.iter().any(|i| i.label == "Warehouse"),
            "should suggest entity names"
        );
    }

    #[test]
    fn suggests_resources_in_flow_context_without_duplicates() {
        let source = r#"
Entity "Warehouse"
Entity "Factory"
Resource "Cameras" units

Flow "Cameras" from "Warehouse" to "Factory"
"#;
        let graph = sea_core::parse_to_graph(source).unwrap();
        let line_index = LineIndex::new(source);
        let index = SemanticIndex::build(source);

        let offset = source.find("Flow \"Cameras\"").unwrap() + "Flow \"".len();
        let position = line_index.position_of(offset);

        let result = completion(source, &line_index, position, Some(&graph), Some(&index)).unwrap();
        let CompletionResponse::Array(items) = result else {
            panic!("expected array response");
        };

        assert!(
            items.iter().any(|i| i.label == "Cameras"),
            "should suggest resource names"
        );

        let mut seen = HashSet::new();
        for item in &items {
            assert!(
                seen.insert((item.label.clone(), kind_rank(item.kind))),
                "duplicate completion item: {:?}",
                item.label
            );
        }
    }
}
