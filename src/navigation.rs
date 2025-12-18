use tower_lsp::lsp_types::{Location, Position, Url};

use crate::line_index::LineIndex;
use crate::semantic_index::SemanticIndex;

pub fn goto_definition(
    uri: &Url,
    line_index: &LineIndex,
    position: Position,
    index: &SemanticIndex,
) -> Option<Location> {
    let offset = line_index.offset_of(position)?;
    let occ = index.symbol_at_offset(offset)?;
    let def_range = if occ.is_definition {
        occ.range
    } else {
        index.definition_range(occ.kind, &occ.name)?
    };
    Some(SemanticIndex::lsp_location(uri, line_index, def_range))
}

pub fn find_references(
    uri: &Url,
    line_index: &LineIndex,
    position: Position,
    index: &SemanticIndex,
    include_declaration: bool,
) -> Vec<Location> {
    let Some(offset) = line_index.offset_of(position) else {
        return Vec::new();
    };
    let Some(occ) = index.symbol_at_offset(offset) else {
        return Vec::new();
    };

    let mut locations: Vec<Location> = index
        .reference_ranges(occ.kind, &occ.name)
        .into_iter()
        .map(|r| SemanticIndex::lsp_location(uri, line_index, r))
        .collect();

    if include_declaration {
        if let Some(def_range) = index.definition_range(occ.kind, &occ.name) {
            locations.push(SemanticIndex::lsp_location(uri, line_index, def_range));
        }
    }

    locations.sort_by(|a, b| {
        a.uri
            .as_str()
            .cmp(b.uri.as_str())
            .then_with(|| position_key(a).cmp(&position_key(b)))
    });
    locations.dedup_by(|a, b| a.uri == b.uri && a.range == b.range);
    locations
}

fn position_key(loc: &Location) -> (u32, u32, u32, u32) {
    (
        loc.range.start.line,
        loc.range.start.character,
        loc.range.end.line,
        loc.range.end.character,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line_index::LineIndex;
    use crate::semantic_index::SemanticIndex;
    use crate::semantic_index::SymbolKind;

    #[test]
    fn goto_definition_finds_entity_decl_from_instance_type() {
        let source = r#"
Entity "Vendor" in domain

Instance vendor_123 of "Vendor" {
  name: "Acme"
}
"#;
        let uri = Url::parse("file:///test.sea").unwrap();
        let line_index = LineIndex::new(source);
        let index = SemanticIndex::build(source);

        // Use rfind to get the second occurrence (the usage in 'of "Vendor"'),
        // not the first one (the definition 'Entity "Vendor"').
        let offset = source.rfind("\"Vendor\"").unwrap() + 2;
        let pos = line_index.position_of(offset);
        let loc = goto_definition(&uri, &line_index, pos, &index).expect("definition");

        let def_range = index
            .definition_range(SymbolKind::Entity, "Vendor")
            .expect("entity definition");
        let expected = SemanticIndex::lsp_location(&uri, &line_index, def_range);
        assert_eq!(loc.range, expected.range);
    }

    #[test]
    fn find_references_includes_flow_and_instance_usages() {
        let source = r#"
Entity "Warehouse"
Entity "Factory"
Resource "Cameras" units
Flow "Cameras" from "Warehouse" to "Factory" quantity 10
Instance vendor_123 of "Warehouse" {}
"#;
        let uri = Url::parse("file:///test.sea").unwrap();
        let line_index = LineIndex::new(source);
        let index = SemanticIndex::build(source);

        let offset = source.find("\"Warehouse\"").unwrap() + 2;
        let pos = line_index.position_of(offset);
        let refs = find_references(&uri, &line_index, pos, &index, true);

        assert!(
            refs.len() >= 3,
            "expected definition + at least two references"
        );

        let def_range = index
            .definition_range(SymbolKind::Entity, "Warehouse")
            .expect("entity definition");
        let def_loc = SemanticIndex::lsp_location(&uri, &line_index, def_range);
        assert!(
            refs.iter().any(|l| l.range == def_loc.range),
            "should include declaration when requested"
        );
    }

    #[test]
    fn goto_definition_from_flow_endpoint_to_entity_decl() {
        let source = r#"
Entity "Warehouse"
Entity "Factory"
Resource "Cameras" units
Flow "Cameras" from "Warehouse" to "Factory" quantity 10
"#;
        let uri = Url::parse("file:///test.sea").unwrap();
        let line_index = LineIndex::new(source);
        let index = SemanticIndex::build(source);

        let offset = source.rfind("from \"Warehouse\"").unwrap() + "from \"".len() + 1;
        let pos = line_index.position_of(offset);
        let loc = goto_definition(&uri, &line_index, pos, &index).expect("definition");

        let def_range = index
            .definition_range(SymbolKind::Entity, "Warehouse")
            .expect("entity definition");
        let expected = SemanticIndex::lsp_location(&uri, &line_index, def_range);
        assert_eq!(loc.range, expected.range);
    }
}
