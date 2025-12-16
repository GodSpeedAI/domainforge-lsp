use std::collections::HashMap;

use pest::iterators::Pair;
use pest::Parser;
use sea_core::parser::{unescape_string, Rule, SeaParser};
use serde::{Deserialize, Serialize};
use tower_lsp::lsp_types::{Location, Range, Url};

use crate::line_index::LineIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolKind {
    Entity,
    Resource,
    Flow,
    Pattern,
    Role,
    Relation,
    Instance,
    Policy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

impl ByteRange {
    pub fn contains(&self, offset: usize) -> bool {
        offset >= self.start && offset < self.end
    }
}

#[derive(Debug, Clone)]
pub struct Occurrence {
    pub kind: SymbolKind,
    pub name: String,
    pub range: ByteRange,
    pub is_definition: bool,
}

#[derive(Debug, Clone)]
pub struct FlowDecl {
    pub range: ByteRange,
    pub resource: String,
    pub from_entity: String,
    pub to_entity: String,
    pub quantity: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SemanticIndex {
    pub occurrences: Vec<Occurrence>,
    definitions: HashMap<(SymbolKind, String), ByteRange>,
    references: HashMap<(SymbolKind, String), Vec<ByteRange>>,
    pub import_prefixes: Vec<String>,
    pub flows: Vec<FlowDecl>,
}

impl SemanticIndex {
    pub fn build(source: &str) -> Self {
        let mut index = Self::default();

        let Ok(mut pairs) = SeaParser::parse(Rule::program, source) else {
            return index;
        };

        if let Some(program) = pairs.next() {
            index.walk(program);
        }

        index.import_prefixes.sort();
        index.import_prefixes.dedup();
        index.flows.sort_by_key(|f| (f.range.start, f.range.end));
        index
    }

    pub fn symbol_at_offset(&self, offset: usize) -> Option<&Occurrence> {
        self.occurrences
            .iter()
            .filter(|occ| occ.range.contains(offset))
            .min_by_key(|occ| occ.range.end.saturating_sub(occ.range.start))
    }

    pub fn definition_range(&self, kind: SymbolKind, name: &str) -> Option<ByteRange> {
        self.definitions
            .get(&(kind, name.to_string()))
            .copied()
            .or_else(|| {
                // Instances are referenced as @name; treat definitions by instance identifier.
                if kind == SymbolKind::Instance {
                    self.definitions
                        .get(&(kind, name.trim_start_matches('@').to_string()))
                        .copied()
                } else {
                    None
                }
            })
    }

    pub fn reference_ranges(&self, kind: SymbolKind, name: &str) -> Vec<ByteRange> {
        self.references
            .get(&(kind, name.to_string()))
            .cloned()
            .unwrap_or_default()
    }

    pub fn flow_decl_for_range(&self, range: ByteRange) -> Option<&FlowDecl> {
        self.flows.iter().find(|f| f.range == range)
    }

    pub fn lsp_location(uri: &Url, line_index: &LineIndex, range: ByteRange) -> Location {
        Location {
            uri: uri.clone(),
            range: Range {
                start: line_index.position_of(range.start),
                end: line_index.position_of(range.end),
            },
        }
    }

    fn walk(&mut self, pair: Pair<'_, Rule>) {
        match pair.as_rule() {
            Rule::import_decl => self.parse_import_decl(pair),
            Rule::entity_decl => self.parse_entity_decl(pair),
            Rule::resource_decl => self.parse_resource_decl(pair),
            Rule::flow_decl => self.parse_flow_decl(pair),
            Rule::pattern_decl => self.parse_pattern_decl(pair),
            Rule::role_decl => self.parse_role_decl(pair),
            Rule::relation_decl => self.parse_relation_decl(pair),
            Rule::instance_decl => self.parse_instance_decl(pair),
            Rule::instance_reference => self.parse_instance_reference(pair),
            Rule::policy_decl => self.parse_policy_decl(pair),
            _ => {
                for inner in pair.into_inner() {
                    self.walk(inner);
                }
            }
        }
    }

    fn parse_import_decl(&mut self, pair: Pair<'_, Rule>) {
        // import_decl = { ^"import" ~ import_specifier ~ ^"from" ~ string_literal }
        for inner in pair.into_inner() {
            match inner.as_rule() {
                Rule::import_named => self.parse_import_named(inner),
                Rule::import_wildcard => self.parse_import_wildcard(inner),
                Rule::import_specifier => {
                    for spec in inner.into_inner() {
                        match spec.as_rule() {
                            Rule::import_named => self.parse_import_named(spec),
                            Rule::import_wildcard => self.parse_import_wildcard(spec),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn parse_import_named(&mut self, pair: Pair<'_, Rule>) {
        // import_item = { identifier ~ (^"as" ~ identifier)? }
        // Treat either alias or name as a prefix suggestion.
        for inner in pair.into_inner() {
            if inner.as_rule() == Rule::import_item {
                let mut identifiers = inner
                    .into_inner()
                    .filter(|p| p.as_rule() == Rule::identifier);
                let Some(name) = identifiers.next() else {
                    continue;
                };
                let alias = identifiers.next();
                if let Some(alias) = alias {
                    self.import_prefixes.push(alias.as_str().to_string());
                } else {
                    self.import_prefixes.push(name.as_str().to_string());
                }
            }
        }
    }

    fn parse_import_wildcard(&mut self, pair: Pair<'_, Rule>) {
        // import_wildcard = { "*" ~ ^"as" ~ identifier }
        if let Some(ident) = pair.into_inner().find(|p| p.as_rule() == Rule::identifier) {
            self.import_prefixes.push(ident.as_str().to_string());
        }
    }

    fn parse_entity_decl(&mut self, pair: Pair<'_, Rule>) {
        // entity_decl = { ^"entity" ~ name ~ ... }
        if let Some(name_pair) = pair.into_inner().find(|p| p.as_rule() == Rule::name) {
            self.record_name(SymbolKind::Entity, name_pair, true);
        }
    }

    fn parse_resource_decl(&mut self, pair: Pair<'_, Rule>) {
        if let Some(name_pair) = pair.into_inner().find(|p| p.as_rule() == Rule::name) {
            self.record_name(SymbolKind::Resource, name_pair, true);
        }
    }

    fn parse_pattern_decl(&mut self, pair: Pair<'_, Rule>) {
        if let Some(name_pair) = pair.into_inner().find(|p| p.as_rule() == Rule::name) {
            self.record_name(SymbolKind::Pattern, name_pair, true);
        }
    }

    fn parse_role_decl(&mut self, pair: Pair<'_, Rule>) {
        if let Some(name_pair) = pair.into_inner().find(|p| p.as_rule() == Rule::name) {
            self.record_name(SymbolKind::Role, name_pair, true);
        }
    }

    fn parse_relation_decl(&mut self, pair: Pair<'_, Rule>) {
        // relation_decl begins with relation name, then subject/predicate/object, optional via flow.
        let inner_pairs: Vec<Pair<'_, Rule>> = pair.into_inner().collect();

        if let Some(name_pair) = inner_pairs.iter().find(|p| p.as_rule() == Rule::name) {
            self.record_name(SymbolKind::Relation, name_pair.clone(), true);
        }

        // Remaining string_literal pairs include subject role, predicate, object role, optional via flow.
        // We treat subject/object role literals as Role references.
        // The optional via flow refers to a Flow by resource-name, so treat it as a Resource reference.
        let mut string_literals = inner_pairs
            .into_iter()
            .filter(|p| p.as_rule() == Rule::string_literal);
        let subject = string_literals.next();
        let _predicate = string_literals.next();
        let object = string_literals.next();
        let via_flow = string_literals.next();

        if let Some(subject) = subject {
            self.record_string_literal(SymbolKind::Role, subject, false);
        }
        if let Some(object) = object {
            self.record_string_literal(SymbolKind::Role, object, false);
        }
        if let Some(via_flow) = via_flow {
            self.record_string_literal(SymbolKind::Resource, via_flow, false);
        }
    }

    fn parse_flow_decl(&mut self, pair: Pair<'_, Rule>) {
        // flow_decl = { ^"flow" ~ string_literal ~ ^"from" ~ string_literal ~ ^"to" ~ string_literal ... }
        let span = pair.as_span();
        let decl_range = ByteRange {
            start: span.start(),
            end: span.end(),
        };

        let inner_pairs: Vec<Pair<'_, Rule>> = pair.into_inner().collect();
        let mut literals = inner_pairs
            .iter()
            .filter(|p| p.as_rule() == Rule::string_literal)
            .cloned();

        let resource_name = literals.next();
        let from_entity = literals.next();
        let to_entity = literals.next();

        if let Some(resource_name) = resource_name.clone() {
            self.record_string_literal(SymbolKind::Resource, resource_name, false);
        }
        if let Some(from_entity) = from_entity.clone() {
            self.record_string_literal(SymbolKind::Entity, from_entity, false);
        }
        if let Some(to_entity) = to_entity.clone() {
            self.record_string_literal(SymbolKind::Entity, to_entity, false);
        }

        let quantity = inner_pairs
            .iter()
            .find(|p| p.as_rule() == Rule::number)
            .map(|p| p.as_str().to_string());

        let resource = resource_name
            .as_ref()
            .and_then(|p| extract_string_literal_value(p.as_str()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let from = from_entity
            .as_ref()
            .and_then(|p| extract_string_literal_value(p.as_str()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let to = to_entity
            .as_ref()
            .and_then(|p| extract_string_literal_value(p.as_str()))
            .unwrap_or_else(|| "<unknown>".to_string());

        self.flows.push(FlowDecl {
            range: decl_range,
            resource,
            from_entity: from,
            to_entity: to,
            quantity,
        });

        // Record a coarse Flow occurrence so hovering the "flow" keyword yields a Flow hover.
        self.record(
            SymbolKind::Flow,
            format!("flow@{}..{}", decl_range.start, decl_range.end),
            decl_range,
            true,
        );
    }

    fn parse_instance_decl(&mut self, pair: Pair<'_, Rule>) {
        // instance_decl = { ^"instance" ~ identifier ~ ^"of" ~ string_literal ~ instance_body? }
        let inner_pairs: Vec<Pair<'_, Rule>> = pair.into_inner().collect();
        let instance_ident = inner_pairs
            .iter()
            .find(|p| p.as_rule() == Rule::identifier)
            .cloned();
        let entity_type = inner_pairs
            .iter()
            .find(|p| p.as_rule() == Rule::string_literal)
            .cloned();

        if let Some(instance_ident) = instance_ident {
            self.record_identifier(SymbolKind::Instance, instance_ident, true);
        }
        if let Some(entity_type) = entity_type {
            self.record_string_literal(SymbolKind::Entity, entity_type, false);
        }
    }

    fn parse_instance_reference(&mut self, pair: Pair<'_, Rule>) {
        let span = pair.as_span();
        let raw = pair.as_str();
        let name = raw.trim_start_matches('@').to_string();
        let range = ByteRange {
            start: span.start(),
            end: span.end(),
        };
        self.record(SymbolKind::Instance, name, range, false);
    }

    fn parse_policy_decl(&mut self, pair: Pair<'_, Rule>) {
        // policy_decl = { ^"policy" ~ identifier ~ ... }
        // Policies use bare identifiers for names, not quoted strings
        // We need to walk the inner pairs to capture instance references in the expression
        let mut found_name = false;
        for inner in pair.into_inner() {
            if !found_name && inner.as_rule() == Rule::identifier {
                // First identifier is the policy name
                self.record_identifier(SymbolKind::Policy, inner, true);
                found_name = true;
            } else {
                // Walk other children to capture instance references etc.
                self.walk(inner);
            }
        }
    }

    fn record_name(&mut self, kind: SymbolKind, pair: Pair<'_, Rule>, is_definition: bool) {
        let Some(literal) = pair
            .into_inner()
            .find(|p| matches!(p.as_rule(), Rule::string_literal | Rule::multiline_string))
        else {
            return;
        };
        match literal.as_rule() {
            Rule::string_literal => self.record_string_literal(kind, literal, is_definition),
            Rule::multiline_string => self.record_multiline_string(kind, literal, is_definition),
            _ => {}
        }
    }

    fn record_identifier(&mut self, kind: SymbolKind, pair: Pair<'_, Rule>, is_definition: bool) {
        let span = pair.as_span();
        let range = ByteRange {
            start: span.start(),
            end: span.end(),
        };
        self.record(kind, pair.as_str().to_string(), range, is_definition);
    }

    fn record_string_literal(
        &mut self,
        kind: SymbolKind,
        pair: Pair<'_, Rule>,
        is_definition: bool,
    ) {
        let span = pair.as_span();
        let raw = pair.as_str();
        let name = extract_string_literal_value(raw).unwrap_or_else(|| raw.to_string());
        let range = ByteRange {
            start: span.start(),
            end: span.end(),
        };
        self.record(kind, name, range, is_definition);
    }

    fn record_multiline_string(
        &mut self,
        kind: SymbolKind,
        pair: Pair<'_, Rule>,
        is_definition: bool,
    ) {
        let span = pair.as_span();
        let raw = pair.as_str();
        let inner = raw
            .strip_prefix("\"\"\"")
            .and_then(|s| s.strip_suffix("\"\"\""))
            .unwrap_or(raw);
        let range = ByteRange {
            start: span.start(),
            end: span.end(),
        };
        self.record(kind, inner.to_string(), range, is_definition);
    }

    fn record(&mut self, kind: SymbolKind, name: String, range: ByteRange, is_definition: bool) {
        self.occurrences.push(Occurrence {
            kind,
            name: name.clone(),
            range,
            is_definition,
        });

        if is_definition {
            self.definitions.insert((kind, name), range);
        } else {
            self.references.entry((kind, name)).or_default().push(range);
        }
    }
}

fn extract_string_literal_value(raw: &str) -> Option<String> {
    let unquoted = raw.strip_prefix('"').and_then(|s| s.strip_suffix('"'))?;
    Some(
        unescape_string(unquoted)
            .ok()
            .unwrap_or_else(|| unquoted.to_string()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line_index::LineIndex;

    #[test]
    fn builds_definitions_and_references() {
        let source = r#"
import * as logistics from "logistics.sea"

Entity "Warehouse" in logistics
Entity "Factory" in logistics
Resource "Cameras" units

Flow "Cameras" from "Warehouse" to "Factory" quantity 10

Instance vendor_123 of "Warehouse" {
  name: "Acme"
}

Policy p as: @vendor_123 = @vendor_123
"#;

        let index = SemanticIndex::build(source);
        assert!(
            index.import_prefixes.contains(&"logistics".to_string()),
            "should capture import prefix"
        );

        let def = index.definition_range(SymbolKind::Entity, "Warehouse");
        assert!(def.is_some(), "should index Entity definition");

        let refs = index.reference_ranges(SymbolKind::Entity, "Warehouse");
        assert!(!refs.is_empty(), "should index Entity references");

        let refs_instances = index.reference_ranges(SymbolKind::Instance, "vendor_123");
        assert_eq!(refs_instances.len(), 2, "should index @instance references");

        let line_index = LineIndex::new(source);
        let offset = source
            .find("\"Warehouse\"")
            .expect("Warehouse literal exists")
            + 2;
        let occ = index
            .symbol_at_offset(offset)
            .expect("should resolve symbol at offset");
        assert_eq!(occ.kind, SymbolKind::Entity);
        assert_eq!(occ.name, "Warehouse");
        assert!(occ.is_definition);

        let pos = line_index.position_of(offset);
        assert!(pos.line > 0);
    }
}
