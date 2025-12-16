use std::collections::BTreeMap;

use blake3::Hasher;
use sea_core::Graph;
use tower_lsp::lsp_types::{Position, Url};

use crate::line_index::LineIndex;
use crate::semantic_index::{ByteRange, FlowDecl, Occurrence, SemanticIndex, SymbolKind};

use super::{
    DetailLevel, HoverContext, HoverHeader, HoverLimits, HoverModel, HoverPosition, HoverRange,
    HoverRelated, HoverScopeSummary, HoverSymbol,
};

const SCHEMA_VERSION: &str = "1.0";
const MAX_MARKDOWN_BYTES: usize = 32 * 1024;
const MAX_JSON_BYTES: usize = 128 * 1024;
const MAX_FLOW_SCAN: usize = 2000;

#[derive(Debug, Clone)]
pub struct HoverBuildInput<'a> {
    pub uri: &'a Url,
    pub document_version: i32,
    pub position: Position,
    pub config_hash: &'a str,
    pub detail_level: DetailLevel,
    pub line_index: &'a LineIndex,
    pub index: &'a SemanticIndex,
    pub graph: Option<&'a Graph>,
}

pub fn build_hover_model(input: HoverBuildInput<'_>) -> Option<HoverModel> {
    let offset = input.line_index.offset_of(input.position)?;
    let occurrence = input.index.symbol_at_offset(offset)?;

    let resolved = resolve_occurrence(occurrence, input.index, input.graph, input.detail_level);
    let id = hover_id(
        input.uri,
        input.document_version,
        input.position,
        input.config_hash,
        &resolved.resolve_id,
        input.detail_level,
    );

    let range = byte_range_to_hover_range(input.line_index, occurrence.range);

    let mut related = resolved.related;
    related.sort_by(|a, b| {
        b.relevance_score
            .cmp(&a.relevance_score)
            .then_with(|| a.qualified_name.cmp(&b.qualified_name))
            .then_with(|| a.kind.cmp(&b.kind))
    });
    related.truncate(5);

    let mut model = HoverModel {
        schema_version: SCHEMA_VERSION.to_string(),
        id,
        symbol: HoverSymbol {
            name: resolved.name.clone(),
            kind: resolved.kind_label.to_string(),
            qualified_name: resolved.qualified_name.clone(),
            uri: input.uri.to_string(),
            range,
            resolve_id: resolved.resolve_id,
            resolution_confidence: resolved.confidence,
        },
        context: HoverContext {
            document_version: input.document_version,
            position: HoverPosition::from(input.position),
            scope_summary: HoverScopeSummary {
                module: None,
                enclosing_rule: None,
                namespaces_in_scope: input.index.import_prefixes.clone(),
            },
            config_hash: input.config_hash.to_string(),
        },
        primary: super::HoverPrimary {
            header: HoverHeader {
                display_name: resolved.name,
                kind_label: resolved.kind_label.to_string(),
                qualified_path: resolved.qualified_name.clone(),
            },
            signature_or_shape: resolved.signature,
            summary: resolved.summary,
            badges: resolved.badges,
            facts: resolved.facts,
        },
        related,
        limits: HoverLimits {
            max_markdown_bytes: MAX_MARKDOWN_BYTES,
            max_json_bytes: MAX_JSON_BYTES,
            truncated_sections: resolved.truncated_sections,
        },
    };
    model.limits.truncated_sections.sort();
    model.limits.truncated_sections.dedup();
    Some(model)
}

#[derive(Debug, Clone)]
struct ResolvedSymbol {
    name: String,
    kind_label: &'static str,
    qualified_name: String,
    resolve_id: String,
    confidence: String,
    signature: String,
    summary: String,
    badges: Vec<String>,
    facts: Vec<(String, String)>,
    related: Vec<HoverRelated>,
    truncated_sections: Vec<String>,
}

fn resolve_occurrence(
    occ: &Occurrence,
    index: &SemanticIndex,
    graph: Option<&Graph>,
    detail_level: DetailLevel,
) -> ResolvedSymbol {
    match occ.kind {
        SymbolKind::Entity => resolve_entity(&occ.name, graph, detail_level),
        SymbolKind::Resource => resolve_resource(&occ.name, graph, detail_level),
        SymbolKind::Flow => resolve_flow(occ.range, index, graph),
        SymbolKind::Role => resolve_role(&occ.name, graph),
        SymbolKind::Relation => resolve_relation(&occ.name, graph),
        SymbolKind::Pattern => resolve_pattern(&occ.name, graph),
        SymbolKind::Instance => resolve_instance(&occ.name, graph, detail_level),
        SymbolKind::Policy => resolve_policy(&occ.name, graph),
    }
}

fn resolve_entity(name: &str, graph: Option<&Graph>, detail_level: DetailLevel) -> ResolvedSymbol {
    let mut badges = Vec::new();
    let mut facts = Vec::new();
    let mut related = Vec::new();
    let mut truncated_sections = Vec::new();

    let (resolve_id, qualified_name, confidence, namespace, flow_counts, role_names) = match graph {
        Some(graph) => {
            let mut matches: Vec<_> = graph
                .all_entities()
                .into_iter()
                .filter(|e| e.name() == name)
                .collect();
            matches.sort_by(|a, b| {
                a.namespace()
                    .cmp(b.namespace())
                    .then_with(|| a.id().to_string().cmp(&b.id().to_string()))
            });
            match matches.as_slice() {
                [] => (
                    "<unresolved>".to_string(),
                    name.to_string(),
                    "error_fallback".to_string(),
                    None,
                    None,
                    None,
                ),
                [entity] => {
                    let flows_from = graph.flows_from(entity.id()).len();
                    let flows_to = graph.flows_to(entity.id()).len();
                    let roles = graph.role_names_for_entity(entity.id());
                    if let Some(version) = entity.version() {
                        facts.push(("version".to_string(), version.to_string()));
                    }
                    if let Some(replaces) = entity.replaces() {
                        facts.push(("replaces".to_string(), replaces.to_string()));
                    }
                    if !entity.changes().is_empty() {
                        facts.push(("changes".to_string(), entity.changes().join("; ")));
                    }
                    (
                        entity.id().to_string(),
                        format!("{}::{}", entity.namespace(), entity.name()),
                        "exact".to_string(),
                        Some(entity.namespace().to_string()),
                        Some((flows_from, flows_to)),
                        Some(roles),
                    )
                }
                [first, ..] => (
                    first.id().to_string(),
                    format!("{}::{}", first.namespace(), first.name()),
                    "ambiguous".to_string(),
                    Some(first.namespace().to_string()),
                    None,
                    None,
                ),
            }
        }
        None => (
            "<no-graph>".to_string(),
            name.to_string(),
            "error_fallback".to_string(),
            None,
            None,
            None,
        ),
    };

    if confidence == "ambiguous" {
        badges.push("ambiguous".to_string());
    }
    if confidence == "error_fallback" {
        badges.push("unresolved".to_string());
    }

    if let Some(ns) = namespace {
        facts.push(("namespace".to_string(), ns));
    }
    if let Some((from_count, to_count)) = flow_counts {
        facts.push(("flows_from".to_string(), from_count.to_string()));
        facts.push(("flows_to".to_string(), to_count.to_string()));
    }
    if let Some(roles) = role_names {
        if !roles.is_empty() {
            let mut roles = roles;
            roles.sort();
            facts.push(("roles".to_string(), roles.join(", ")));
        }
    }

    if matches!(detail_level, DetailLevel::Standard | DetailLevel::Deep) {
        if let Some(graph) = graph {
            let mut resources_by_count: BTreeMap<String, i32> = BTreeMap::new();
            let flows = graph.all_flows();
            if flows.len() > MAX_FLOW_SCAN {
                truncated_sections.push("budget_exceeded".to_string());
            }
            for flow in flows.into_iter().take(MAX_FLOW_SCAN) {
                let involves = graph
                    .get_entity(flow.from_id())
                    .is_some_and(|e| e.name() == name)
                    || graph
                        .get_entity(flow.to_id())
                        .is_some_and(|e| e.name() == name);
                if !involves {
                    continue;
                }
                if let Some(res) = graph.get_resource(flow.resource_id()) {
                    *resources_by_count
                        .entry(format!("{}::{}", res.namespace(), res.name()))
                        .or_default() += 1;
                }
            }
            for (qname, score) in resources_by_count {
                related.push(HoverRelated {
                    qualified_name: qname,
                    kind: "Resource".to_string(),
                    relevance_score: score,
                });
            }
        }
    }

    ResolvedSymbol {
        name: name.to_string(),
        kind_label: "Entity",
        qualified_name,
        resolve_id,
        confidence,
        signature: format!("Entity \"{}\"", name),
        summary: "DomainForge entity".to_string(),
        badges,
        facts,
        related,
        truncated_sections,
    }
}

fn resolve_resource(
    name: &str,
    graph: Option<&Graph>,
    detail_level: DetailLevel,
) -> ResolvedSymbol {
    let mut badges = Vec::new();
    let mut facts = Vec::new();
    let mut related = Vec::new();
    let mut truncated_sections = Vec::new();

    let (resolve_id, qualified_name, confidence, namespace, unit_symbol) = match graph {
        Some(graph) => {
            let mut matches: Vec<_> = graph
                .all_resources()
                .into_iter()
                .filter(|r| r.name() == name)
                .collect();
            matches.sort_by(|a, b| {
                a.namespace()
                    .cmp(b.namespace())
                    .then_with(|| a.id().to_string().cmp(&b.id().to_string()))
            });
            match matches.as_slice() {
                [] => (
                    "<unresolved>".to_string(),
                    name.to_string(),
                    "error_fallback".to_string(),
                    None,
                    None,
                ),
                [res] => (
                    res.id().to_string(),
                    format!("{}::{}", res.namespace(), res.name()),
                    "exact".to_string(),
                    Some(res.namespace().to_string()),
                    Some(res.unit().symbol().to_string()),
                ),
                [first, ..] => (
                    first.id().to_string(),
                    format!("{}::{}", first.namespace(), first.name()),
                    "ambiguous".to_string(),
                    Some(first.namespace().to_string()),
                    Some(first.unit().symbol().to_string()),
                ),
            }
        }
        None => (
            "<no-graph>".to_string(),
            name.to_string(),
            "error_fallback".to_string(),
            None,
            None,
        ),
    };

    if confidence == "ambiguous" {
        badges.push("ambiguous".to_string());
    }
    if confidence == "error_fallback" {
        badges.push("unresolved".to_string());
    }
    if let Some(ns) = namespace {
        facts.push(("namespace".to_string(), ns));
    }
    if let Some(unit) = unit_symbol {
        facts.push(("unit".to_string(), unit));
    }

    if matches!(detail_level, DetailLevel::Standard | DetailLevel::Deep) {
        if let Some(graph) = graph {
            let mut entities_by_count: BTreeMap<String, i32> = BTreeMap::new();
            let flows = graph.all_flows();
            if flows.len() > MAX_FLOW_SCAN {
                truncated_sections.push("budget_exceeded".to_string());
            }
            for flow in flows.into_iter().take(MAX_FLOW_SCAN) {
                if let Some(res) = graph.get_resource(flow.resource_id()) {
                    if res.name() != name {
                        continue;
                    }
                } else {
                    continue;
                }

                if let Some(from) = graph.get_entity(flow.from_id()) {
                    *entities_by_count
                        .entry(format!("{}::{}", from.namespace(), from.name()))
                        .or_default() += 1;
                }
                if let Some(to) = graph.get_entity(flow.to_id()) {
                    *entities_by_count
                        .entry(format!("{}::{}", to.namespace(), to.name()))
                        .or_default() += 1;
                }
            }
            for (qname, score) in entities_by_count {
                related.push(HoverRelated {
                    qualified_name: qname,
                    kind: "Entity".to_string(),
                    relevance_score: score,
                });
            }
        }
    }

    ResolvedSymbol {
        name: name.to_string(),
        kind_label: "Resource",
        qualified_name,
        resolve_id,
        confidence,
        signature: format!("Resource \"{}\"", name),
        summary: "DomainForge resource".to_string(),
        badges,
        facts,
        related,
        truncated_sections,
    }
}

fn resolve_instance(
    name: &str,
    graph: Option<&Graph>,
    detail_level: DetailLevel,
) -> ResolvedSymbol {
    let mut badges = Vec::new();
    let mut facts = Vec::new();
    let mut related = Vec::new();
    let truncated_sections = Vec::new();

    let (resolve_id, qualified_name, confidence, entity_type, field_count) = match graph {
        Some(graph) => match graph.get_entity_instance(name) {
            None => (
                "<unresolved>".to_string(),
                name.to_string(),
                "error_fallback".to_string(),
                None,
                None,
            ),
            Some(instance) => (
                instance.id().to_string(),
                format!("{}::{}", instance.namespace(), instance.name()),
                "exact".to_string(),
                Some(instance.entity_type().to_string()),
                Some(instance.fields().len()),
            ),
        },
        None => (
            "<no-graph>".to_string(),
            name.to_string(),
            "error_fallback".to_string(),
            None,
            None,
        ),
    };

    if confidence == "error_fallback" {
        badges.push("unresolved".to_string());
    }

    if let Some(entity_type) = entity_type {
        facts.push(("of".to_string(), entity_type.clone()));
        if matches!(detail_level, DetailLevel::Standard | DetailLevel::Deep) {
            related.push(HoverRelated {
                qualified_name: entity_type,
                kind: "Entity".to_string(),
                relevance_score: 10,
            });
        }
    }
    if let Some(field_count) = field_count {
        facts.push(("fields".to_string(), field_count.to_string()));
    }

    ResolvedSymbol {
        name: name.to_string(),
        kind_label: "Instance",
        qualified_name,
        resolve_id,
        confidence,
        signature: format!("Instance {} of \"…\"", name),
        summary: "DomainForge entity instance".to_string(),
        badges,
        facts,
        related,
        truncated_sections,
    }
}

fn resolve_role(name: &str, graph: Option<&Graph>) -> ResolvedSymbol {
    let mut badges = Vec::new();
    let mut facts = Vec::new();
    let truncated_sections = Vec::new();

    let (resolve_id, qualified_name, confidence, namespace) = match graph {
        Some(graph) => {
            let mut matches: Vec<_> = graph
                .all_roles()
                .into_iter()
                .filter(|r| r.name() == name)
                .collect();
            matches.sort_by(|a, b| {
                a.namespace()
                    .cmp(b.namespace())
                    .then_with(|| a.id().to_string().cmp(&b.id().to_string()))
            });
            match matches.as_slice() {
                [] => (
                    "<unresolved>".to_string(),
                    name.to_string(),
                    "error_fallback".to_string(),
                    None,
                ),
                [role] => (
                    role.id().to_string(),
                    format!("{}::{}", role.namespace(), role.name()),
                    "exact".to_string(),
                    Some(role.namespace().to_string()),
                ),
                [first, ..] => (
                    first.id().to_string(),
                    format!("{}::{}", first.namespace(), first.name()),
                    "ambiguous".to_string(),
                    Some(first.namespace().to_string()),
                ),
            }
        }
        None => (
            "<no-graph>".to_string(),
            name.to_string(),
            "error_fallback".to_string(),
            None,
        ),
    };

    if confidence != "exact" {
        badges.push(if confidence == "ambiguous" {
            "ambiguous".to_string()
        } else {
            "unresolved".to_string()
        });
    }

    if let Some(ns) = namespace {
        facts.push(("namespace".to_string(), ns));
    }

    ResolvedSymbol {
        name: name.to_string(),
        kind_label: "Role",
        qualified_name,
        resolve_id,
        confidence,
        signature: format!("Role \"{}\"", name),
        summary: "DomainForge role".to_string(),
        badges,
        facts,
        related: Vec::new(),
        truncated_sections,
    }
}

fn resolve_relation(name: &str, graph: Option<&Graph>) -> ResolvedSymbol {
    let mut badges = Vec::new();
    let mut facts = Vec::new();
    let truncated_sections = Vec::new();

    let (resolve_id, qualified_name, confidence, namespace) = match graph {
        Some(graph) => {
            let mut matches: Vec<_> = graph
                .all_relations()
                .into_iter()
                .filter(|r| r.name() == name)
                .collect();
            matches.sort_by(|a, b| {
                a.namespace()
                    .cmp(b.namespace())
                    .then_with(|| a.id().to_string().cmp(&b.id().to_string()))
            });
            match matches.as_slice() {
                [] => (
                    "<unresolved>".to_string(),
                    name.to_string(),
                    "error_fallback".to_string(),
                    None,
                ),
                [rel] => (
                    rel.id().to_string(),
                    format!("{}::{}", rel.namespace(), rel.name()),
                    "exact".to_string(),
                    Some(rel.namespace().to_string()),
                ),
                [first, ..] => (
                    first.id().to_string(),
                    format!("{}::{}", first.namespace(), first.name()),
                    "ambiguous".to_string(),
                    Some(first.namespace().to_string()),
                ),
            }
        }
        None => (
            "<no-graph>".to_string(),
            name.to_string(),
            "error_fallback".to_string(),
            None,
        ),
    };

    if confidence != "exact" {
        badges.push(if confidence == "ambiguous" {
            "ambiguous".to_string()
        } else {
            "unresolved".to_string()
        });
    }

    if let Some(ns) = namespace {
        facts.push(("namespace".to_string(), ns));
    }

    ResolvedSymbol {
        name: name.to_string(),
        kind_label: "Relation",
        qualified_name,
        resolve_id,
        confidence,
        signature: format!("Relation \"{}\"", name),
        summary: "DomainForge relation".to_string(),
        badges,
        facts,
        related: Vec::new(),
        truncated_sections,
    }
}

fn resolve_pattern(name: &str, graph: Option<&Graph>) -> ResolvedSymbol {
    let mut badges = Vec::new();
    let truncated_sections = Vec::new();

    let (resolve_id, qualified_name, confidence) = match graph {
        Some(graph) => {
            let mut matches: Vec<_> = graph
                .all_patterns()
                .into_iter()
                .filter(|p| p.name() == name)
                .collect();
            matches.sort_by(|a, b| {
                a.namespace()
                    .cmp(b.namespace())
                    .then_with(|| a.id().to_string().cmp(&b.id().to_string()))
            });
            match matches.as_slice() {
                [] => (
                    "<unresolved>".to_string(),
                    name.to_string(),
                    "error_fallback".to_string(),
                ),
                [pat] => (
                    pat.id().to_string(),
                    format!("{}::{}", pat.namespace(), pat.name()),
                    "exact".to_string(),
                ),
                [first, ..] => (
                    first.id().to_string(),
                    format!("{}::{}", first.namespace(), first.name()),
                    "ambiguous".to_string(),
                ),
            }
        }
        None => (
            "<no-graph>".to_string(),
            name.to_string(),
            "error_fallback".to_string(),
        ),
    };

    if confidence != "exact" {
        badges.push(if confidence == "ambiguous" {
            "ambiguous".to_string()
        } else {
            "unresolved".to_string()
        });
    }

    ResolvedSymbol {
        name: name.to_string(),
        kind_label: "Pattern",
        qualified_name,
        resolve_id,
        confidence,
        signature: format!("Pattern \"{}\"", name),
        summary: "DomainForge pattern".to_string(),
        badges,
        facts: Vec::new(),
        related: Vec::new(),
        truncated_sections,
    }
}

fn resolve_policy(name: &str, graph: Option<&Graph>) -> ResolvedSymbol {
    let mut badges = Vec::new();
    let mut facts = Vec::new();
    let truncated_sections = Vec::new();

    let (resolve_id, qualified_name, confidence, namespace, modality, kind, priority, expr_summary) =
        match graph {
            Some(graph) => {
                let mut matches: Vec<_> = graph
                    .all_policies()
                    .into_iter()
                    .filter(|p| p.name == name)
                    .collect();
                matches.sort_by(|a, b| {
                    a.namespace
                        .cmp(&b.namespace)
                        .then_with(|| a.id.to_string().cmp(&b.id.to_string()))
                });
                match matches.as_slice() {
                    [] => (
                        "<unresolved>".to_string(),
                        name.to_string(),
                        "error_fallback".to_string(),
                        None,
                        None,
                        None,
                        None,
                        None,
                    ),
                    [policy] => {
                        let expr_str = format!("{}", policy.expression);
                        let expr_summary = if expr_str.len() > 80 {
                            format!("{}…", &expr_str[..77])
                        } else {
                            expr_str
                        };
                        (
                            policy.id.to_string(),
                            format!("{}::{}", policy.namespace, policy.name),
                            "exact".to_string(),
                            Some(policy.namespace.clone()),
                            Some(format!("{:?}", policy.modality)),
                            Some(format!("{:?}", policy.kind)),
                            Some(policy.priority),
                            Some(expr_summary),
                        )
                    }
                    [first, ..] => (
                        first.id.to_string(),
                        format!("{}::{}", first.namespace, first.name),
                        "ambiguous".to_string(),
                        Some(first.namespace.clone()),
                        None,
                        None,
                        None,
                        None,
                    ),
                }
            }
            None => (
                "<no-graph>".to_string(),
                name.to_string(),
                "error_fallback".to_string(),
                None,
                None,
                None,
                None,
                None,
            ),
        };

    if confidence != "exact" {
        badges.push(if confidence == "ambiguous" {
            "ambiguous".to_string()
        } else {
            "unresolved".to_string()
        });
    }

    if let Some(ns) = namespace {
        facts.push(("namespace".to_string(), ns));
    }
    if let Some(modality) = modality {
        facts.push(("modality".to_string(), modality));
    }
    if let Some(kind) = kind {
        facts.push(("kind".to_string(), kind));
    }
    if let Some(priority) = priority {
        facts.push(("priority".to_string(), priority.to_string()));
    }

    let signature = if let Some(expr) = expr_summary {
        format!("Policy {} as:\n    {}", name, expr)
    } else {
        format!("Policy {} as: …", name)
    };

    ResolvedSymbol {
        name: name.to_string(),
        kind_label: "Policy",
        qualified_name,
        resolve_id,
        confidence,
        signature,
        summary: "DomainForge policy (business rule)".to_string(),
        badges,
        facts,
        related: Vec::new(),
        truncated_sections,
    }
}

fn resolve_flow(range: ByteRange, index: &SemanticIndex, graph: Option<&Graph>) -> ResolvedSymbol {
    let mut facts = Vec::new();
    let truncated_sections = Vec::new();

    let decl = index.flow_decl_for_range(range);
    let FlowDecl {
        resource,
        from_entity,
        to_entity,
        quantity,
        ..
    } = decl.cloned().unwrap_or(FlowDecl {
        range,
        resource: "<unknown>".to_string(),
        from_entity: "<unknown>".to_string(),
        to_entity: "<unknown>".to_string(),
        quantity: None,
    });

    facts.push(("resource".to_string(), resource.clone()));
    facts.push(("from".to_string(), from_entity.clone()));
    facts.push(("to".to_string(), to_entity.clone()));
    if let Some(q) = quantity.clone() {
        facts.push(("quantity".to_string(), q));
    }

    if let Some(graph) = graph {
        let unit = graph
            .all_resources()
            .into_iter()
            .find(|r| r.name() == resource)
            .map(|r| r.unit().symbol().to_string());
        if let Some(unit) = unit {
            facts.push(("unit".to_string(), unit));
        }
    }

    ResolvedSymbol {
        name: "Flow".to_string(),
        kind_label: "Flow",
        qualified_name: format!("Flow {} -> {} ({})", from_entity, to_entity, resource),
        resolve_id: format!("flow@{}..{}", range.start, range.end),
        confidence: if decl.is_some() {
            "exact".to_string()
        } else {
            "error_fallback".to_string()
        },
        signature: if let Some(q) = quantity {
            format!(
                "Flow \"{}\" from \"{}\" to \"{}\" quantity {}",
                resource, from_entity, to_entity, q
            )
        } else {
            format!(
                "Flow \"{}\" from \"{}\" to \"{}\"",
                resource, from_entity, to_entity
            )
        },
        summary: "DomainForge flow".to_string(),
        badges: Vec::new(),
        facts,
        related: Vec::new(),
        truncated_sections,
    }
}

fn hover_id(
    uri: &Url,
    version: i32,
    position: Position,
    config_hash: &str,
    resolve_id: &str,
    detail_level: DetailLevel,
) -> String {
    let mut hasher = Hasher::new();
    hasher.update(uri.as_str().as_bytes());
    hasher.update(version.to_string().as_bytes());
    hasher.update(position.line.to_string().as_bytes());
    hasher.update(position.character.to_string().as_bytes());
    hasher.update(config_hash.as_bytes());
    hasher.update(resolve_id.as_bytes());
    hasher.update(format!("{detail_level:?}").as_bytes());
    hasher.finalize().to_hex().to_string()
}

fn byte_range_to_hover_range(line_index: &LineIndex, range: ByteRange) -> HoverRange {
    let start = line_index.position_of(range.start);
    let end = line_index.position_of(range.end);
    HoverRange {
        start: HoverPosition::from(start),
        end: HoverPosition::from(end),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hover::markdown_renderer::render_markdown;
    use crate::line_index::LineIndex;
    use crate::semantic_index::SemanticIndex;

    #[test]
    fn hover_model_is_deterministic_for_same_snapshot() {
        let source = r#"
Entity "Warehouse" in logistics
Entity "Factory" in logistics
Resource "Cameras" units in inventory
Flow "Cameras" from "Warehouse" to "Factory" quantity 10
"#;
        let graph = sea_core::parse_to_graph(source).unwrap();
        let index = SemanticIndex::build(source);
        let line_index = LineIndex::new(source);
        let uri = Url::parse("file:///test.sea").unwrap();

        let offset = source.find("\"Warehouse\"").unwrap() + 2;
        let position = line_index.position_of(offset);

        let input1 = HoverBuildInput {
            uri: &uri,
            document_version: 1,
            position,
            config_hash: "cfg",
            detail_level: DetailLevel::Standard,
            line_index: &line_index,
            index: &index,
            graph: Some(&graph),
        };

        let input2 = HoverBuildInput {
            uri: &uri,
            document_version: 1,
            position,
            config_hash: "cfg",
            detail_level: DetailLevel::Standard,
            line_index: &line_index,
            index: &index,
            graph: Some(&graph),
        };

        let m1 = build_hover_model(input1).expect("hover model");
        let m2 = build_hover_model(input2).expect("hover model");

        let j1 = serde_json::to_string(&m1).unwrap();
        let j2 = serde_json::to_string(&m2).unwrap();
        assert_eq!(j1, j2);
        assert!(j1.as_bytes().len() <= m1.limits.max_json_bytes);
    }

    #[test]
    fn hover_markdown_includes_expected_facts_for_entity_resource_and_flow() {
        let source = r#"
Entity "Warehouse"
Entity "Factory"
Resource "Cameras" units
Flow "Cameras" from "Warehouse" to "Factory" quantity 10
"#;
        let graph = sea_core::parse_to_graph(source).unwrap();
        let index = SemanticIndex::build(source);
        let line_index = LineIndex::new(source);
        let uri = Url::parse("file:///test.sea").unwrap();

        // Entity hover
        let entity_offset = source.find("Entity \"Warehouse\"").unwrap() + "Entity \"".len() + 1;
        let entity_pos = line_index.position_of(entity_offset);
        let entity_model = build_hover_model(HoverBuildInput {
            uri: &uri,
            document_version: 1,
            position: entity_pos,
            config_hash: "cfg",
            detail_level: DetailLevel::Standard,
            line_index: &line_index,
            index: &index,
            graph: Some(&graph),
        })
        .unwrap();
        let entity_md = render_markdown(&entity_model).markdown;
        assert!(entity_md.contains("**namespace**"));

        // Resource hover
        let res_offset = source.find("Resource \"Cameras\"").unwrap() + "Resource \"".len() + 1;
        let res_pos = line_index.position_of(res_offset);
        let res_model = build_hover_model(HoverBuildInput {
            uri: &uri,
            document_version: 1,
            position: res_pos,
            config_hash: "cfg",
            detail_level: DetailLevel::Standard,
            line_index: &line_index,
            index: &index,
            graph: Some(&graph),
        })
        .unwrap();
        let res_md = render_markdown(&res_model).markdown;
        assert!(res_md.contains("**unit**"));

        // Flow hover (hover on the Flow keyword)
        let flow_offset = source.find("Flow \"Cameras\"").unwrap() + 1;
        let flow_pos = line_index.position_of(flow_offset);
        let flow_model = build_hover_model(HoverBuildInput {
            uri: &uri,
            document_version: 1,
            position: flow_pos,
            config_hash: "cfg",
            detail_level: DetailLevel::Standard,
            line_index: &line_index,
            index: &index,
            graph: Some(&graph),
        })
        .unwrap();
        let flow_md = render_markdown(&flow_model).markdown;
        assert!(flow_md.contains("Flow \"Cameras\""));
        assert!(flow_md.contains("**from**"));
        assert!(flow_md.contains("**to**"));
    }

    #[test]
    fn related_symbols_are_sorted_deterministically() {
        let source = r#"
Entity "Warehouse"
Entity "Factory"
Resource "Cameras" units
Resource "Widgets" units
Flow "Cameras" from "Warehouse" to "Factory" quantity 10
Flow "Widgets" from "Warehouse" to "Factory" quantity 5
"#;
        let graph = sea_core::parse_to_graph(source).unwrap();
        let index = SemanticIndex::build(source);
        let line_index = LineIndex::new(source);
        let uri = Url::parse("file:///test.sea").unwrap();

        let offset = source.find("Entity \"Warehouse\"").unwrap() + "Entity \"".len() + 1;
        let position = line_index.position_of(offset);
        let model = build_hover_model(HoverBuildInput {
            uri: &uri,
            document_version: 1,
            position,
            config_hash: "cfg",
            detail_level: DetailLevel::Standard,
            line_index: &line_index,
            index: &index,
            graph: Some(&graph),
        })
        .unwrap();

        assert!(model.related.len() >= 2);
        let a = &model.related[0].qualified_name;
        let b = &model.related[1].qualified_name;
        assert!(a <= b, "expected stable name ordering, got {a} then {b}");
    }

    #[test]
    fn hover_detail_level_core_omits_related() {
        let source = r#"
Entity "Warehouse"
Entity "Factory"
Resource "Cameras" units
Flow "Cameras" from "Warehouse" to "Factory" quantity 10
"#;
        let graph = sea_core::parse_to_graph(source).unwrap();
        let index = SemanticIndex::build(source);
        let line_index = LineIndex::new(source);
        let uri = Url::parse("file:///test.sea").unwrap();

        let offset = source.find("Entity \"Warehouse\"").unwrap() + "Entity \"".len() + 1;
        let position = line_index.position_of(offset);

        let core = build_hover_model(HoverBuildInput {
            uri: &uri,
            document_version: 1,
            position,
            config_hash: "cfg",
            detail_level: DetailLevel::Core,
            line_index: &line_index,
            index: &index,
            graph: Some(&graph),
        })
        .unwrap();
        assert!(core.related.is_empty());

        let standard = build_hover_model(HoverBuildInput {
            uri: &uri,
            document_version: 1,
            position,
            config_hash: "cfg",
            detail_level: DetailLevel::Standard,
            line_index: &line_index,
            index: &index,
            graph: Some(&graph),
        })
        .unwrap();
        assert!(!standard.related.is_empty());
    }

    #[test]
    fn hovering_whitespace_returns_none() {
        let source = r#"
Entity "Warehouse"
"#;
        let graph = sea_core::parse_to_graph(source).unwrap();
        let index = SemanticIndex::build(source);
        let line_index = LineIndex::new(source);
        let uri = Url::parse("file:///test.sea").unwrap();

        let model = build_hover_model(HoverBuildInput {
            uri: &uri,
            document_version: 1,
            position: Position {
                line: 0,
                character: 0,
            },
            config_hash: "cfg",
            detail_level: DetailLevel::Standard,
            line_index: &line_index,
            index: &index,
            graph: Some(&graph),
        });
        assert!(model.is_none());
    }

    #[test]
    fn hover_policy_shows_expected_metadata() {
        let source = r#"
@namespace "logistics"
@version "1.0.0"

Entity "Warehouse"
Policy all_named per Constraint Obligation priority 5 as:
    true
"#;

        let graph = sea_core::parse_to_graph(source).unwrap();
        let index = SemanticIndex::build(source);
        let line_index = LineIndex::new(source);
        let uri = Url::parse("file:///test.sea").unwrap();

        let offset = source.find("all_named").unwrap() + 2;
        let position = line_index.position_of(offset);

        let model = build_hover_model(HoverBuildInput {
            uri: &uri,
            document_version: 1,
            position,
            config_hash: "cfg",
            detail_level: DetailLevel::Standard,
            line_index: &line_index,
            index: &index,
            graph: Some(&graph),
        })
        .unwrap();

        assert_eq!(model.symbol.kind, "Policy");
        assert_eq!(model.symbol.name, "all_named");
        assert!(model
            .primary
            .signature_or_shape
            .contains("Policy all_named"));
        assert!(model.primary.facts.iter().any(|(k, _)| k == "modality"));
        assert!(model.primary.facts.iter().any(|(k, _)| k == "kind"));
    }
}
