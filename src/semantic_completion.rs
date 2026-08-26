use sea_core::semantic_pack::schema::{ConceptStatus, SemanticPack};
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, Documentation, MarkupContent, MarkupKind,
    InsertTextFormat,
};

pub fn get_semantic_completions(
    pack: &SemanticPack,
    prefix: &str,
    include_deprecated: bool,
) -> Vec<CompletionItem> {
    let lower_prefix = prefix.to_ascii_lowercase();

    let mut items: Vec<CompletionItem> = Vec::new();

    for concept in &pack.concepts {
        match concept.status {
            ConceptStatus::Active => {}
            ConceptStatus::Deprecated if include_deprecated => {}
            ConceptStatus::Proposed => {}
            _ => continue,
        }

        if !lower_prefix.is_empty() {
            let name_match = concept
                .canonical_name
                .to_ascii_lowercase()
                .contains(&lower_prefix);
            let id_match = concept.id.to_ascii_lowercase().contains(&lower_prefix);
            let alias_match = pack
                .aliases
                .iter()
                .filter(|a| a.target_concept_id == concept.id)
                .any(|a| a.alias.to_ascii_lowercase().contains(&lower_prefix));

            if !name_match && !id_match && !alias_match {
                continue;
            }
        }

        let kind = map_concept_kind(concept.kind);

        let mut tags = Vec::new();
        if concept.status == ConceptStatus::Deprecated {
            tags.push(tower_lsp::lsp_types::CompletionItemTag::DEPRECATED);
        }

        let detail = format!(
            "{} ({})",
            concept.canonical_name,
            status_label(concept.status)
        );

        let documentation = if concept.definition.text.is_empty() {
            None
        } else {
            Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::PlainText,
                value: concept.definition.text.clone(),
            }))
        };

        let matching_alias = pack
            .aliases
            .iter()
            .filter(|a| a.target_concept_id == concept.id)
            .find(|a| {
                !lower_prefix.is_empty()
                    && a.alias.to_ascii_lowercase().contains(&lower_prefix)
            })
            .map(|a| a.alias.clone());

        let label = matching_alias.unwrap_or_else(|| concept.canonical_name.clone());

        items.push(CompletionItem {
            label,
            kind: Some(kind),
            detail: Some(detail),
            documentation,
            deprecated: Some(concept.status == ConceptStatus::Deprecated),
            tags: if tags.is_empty() { None } else { Some(tags) },
            insert_text: Some(concept.canonical_name.clone()),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            filter_text: Some(format!(
                "{} {} {}",
                concept.canonical_name,
                concept.id,
                pack.aliases
                    .iter()
                    .filter(|a| a.target_concept_id == concept.id)
                    .map(|a| a.alias.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            )),
            sort_text: Some(format!(
                "{:03}{}",
                status_sort_rank(concept.status),
                concept.canonical_name
            )),
            ..Default::default()
        });
    }

    items.sort_by(|a, b| {
        a.sort_text
            .cmp(&b.sort_text)
            .then_with(|| a.label.cmp(&b.label))
    });

    items
}

fn map_concept_kind(
    kind: sea_core::semantic_pack::schema::ConceptKind,
) -> CompletionItemKind {
    use sea_core::semantic_pack::schema::ConceptKind;
    match kind {
        ConceptKind::Entity => CompletionItemKind::CLASS,
        ConceptKind::Resource => CompletionItemKind::CONSTANT,
        ConceptKind::Role => CompletionItemKind::ENUM_MEMBER,
        ConceptKind::Flow => CompletionItemKind::EVENT,
        ConceptKind::Policy => CompletionItemKind::ENUM,
        ConceptKind::Metric => CompletionItemKind::FIELD,
        ConceptKind::Dimension => CompletionItemKind::STRUCT,
        ConceptKind::Unit => CompletionItemKind::UNIT,
        ConceptKind::External => CompletionItemKind::INTERFACE,
    }
}

fn status_label(status: ConceptStatus) -> &'static str {
    match status {
        ConceptStatus::Active => "active",
        ConceptStatus::Proposed => "proposed",
        ConceptStatus::Deprecated => "deprecated",
        ConceptStatus::Rejected => "rejected",
        ConceptStatus::ExternalOnly => "external",
    }
}

fn status_sort_rank(status: ConceptStatus) -> u8 {
    match status {
        ConceptStatus::Active => 0,
        ConceptStatus::Proposed => 1,
        ConceptStatus::Deprecated => 2,
        ConceptStatus::ExternalOnly => 3,
        ConceptStatus::Rejected => 4,
    }
}
