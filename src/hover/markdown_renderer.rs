use super::HoverModel;

pub struct MarkdownRenderResult {
    pub markdown: String,
    pub truncated_sections: Vec<String>,
}

pub fn render_markdown(model: &HoverModel) -> MarkdownRenderResult {
    let mut lines: Vec<String> = Vec::with_capacity(64);
    let mut truncated_sections = Vec::new();
    let mut code_blocks_used = 0usize;

    // Signature
    lines.push("## Signature".to_string());
    push_code_block(
        &mut lines,
        "sea",
        &model.primary.signature_or_shape,
        40,
        &mut code_blocks_used,
        2,
        &mut truncated_sections,
    );

    // Summary
    lines.push("## Summary".to_string());
    push_text_lines(
        &mut lines,
        &model.primary.summary,
        3,
        "summary",
        &mut truncated_sections,
    );

    // Facts
    lines.push("## Facts".to_string());
    if !model.primary.badges.is_empty() {
        let mut badges = model.primary.badges.clone();
        badges.sort();
        badges.dedup();
        lines.push(format!("- **badges**: {}", badges.join(", ")));
    }
    let mut facts = model.primary.facts.clone();
    facts.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let max_facts = 20usize;
    if facts.len() > max_facts {
        truncated_sections.push("facts".to_string());
    }
    for (k, v) in facts.into_iter().take(max_facts) {
        lines.push(format!("- **{}**: {}", k, v));
    }
    if model.primary.badges.is_empty() && model.primary.facts.is_empty() {
        lines.push("- (no facts)".to_string());
    }

    // Diagnostics
    if model.symbol.resolution_confidence != "exact" || !model.limits.truncated_sections.is_empty()
    {
        lines.push("## Diagnostics".to_string());
        if model.symbol.resolution_confidence != "exact" {
            lines.push(format!(
                "- **resolution**: {}",
                model.symbol.resolution_confidence
            ));
        }
        if !model.limits.truncated_sections.is_empty() {
            let mut t = model.limits.truncated_sections.clone();
            t.sort();
            t.dedup();
            lines.push(format!("- **limits**: {}", t.join(", ")));
        }
    }

    // Resolution (progressive disclosure)
    lines.push("## Resolution".to_string());
    lines.push("<details><summary>Details</summary>".to_string());
    lines.push(String::new());
    lines.push(format!("- **qualified**: {}", model.symbol.qualified_name));
    lines.push(format!("- **resolve_id**: {}", model.symbol.resolve_id));
    lines.push("</details>".to_string());

    // Expansion (placeholder for future deep details)
    if model.symbol.kind == "Flow" {
        lines.push("## Expansion".to_string());
        lines.push("<details><summary>Notes</summary>".to_string());
        lines.push(String::new());
        lines.push("- Flow hovers are derived from the parsed document snapshot.".to_string());
        lines.push("</details>".to_string());
    }

    // Usage (placeholder for future)
    if !model.related.is_empty() {
        lines.push("## Usage".to_string());
        lines.push("<details><summary>Related symbols</summary>".to_string());
        lines.push(String::new());
        lines.push(format!("- {} related item(s)", model.related.len()));
        lines.push("</details>".to_string());
    }

    // Related
    if !model.related.is_empty() {
        lines.push("## Related".to_string());
        for rel in &model.related {
            lines.push(format!(
                "- {} ({})",
                rel.qualified_name.trim(),
                rel.kind.trim()
            ));
        }
    }

    let mut markdown = lines.join("\n");

    let max_bytes = model.limits.max_markdown_bytes;
    if markdown.len() > max_bytes {
        let mut kept = String::with_capacity(max_bytes);
        let mut byte_count = 0usize;
        for line in lines {
            let line_bytes = line.len() + 1;
            if byte_count + line_bytes > max_bytes.saturating_sub(64) {
                truncated_sections.push("markdown".to_string());
                break;
            }
            kept.push_str(&line);
            kept.push('\n');
            byte_count += line_bytes;
        }
        kept.push_str("… truncated. Use hoverPlus for full detail.");
        markdown = kept;
    }

    MarkdownRenderResult {
        markdown,
        truncated_sections,
    }
}

fn push_code_block(
    lines: &mut Vec<String>,
    language: &str,
    content: &str,
    max_lines: usize,
    code_blocks_used: &mut usize,
    max_code_blocks: usize,
    truncated_sections: &mut Vec<String>,
) {
    if *code_blocks_used >= max_code_blocks {
        truncated_sections.push("code_blocks".to_string());
        return;
    }

    lines.push(format!("```{}", language));
    *code_blocks_used += 1;

    let mut content_lines = content.lines();
    for (idx, line) in content_lines.by_ref().take(max_lines).enumerate() {
        let _ = idx;
        lines.push(line.to_string());
    }
    if content.lines().count() > max_lines {
        truncated_sections.push("code_block_lines".to_string());
        lines.push("… truncated. Use hoverPlus for full detail.".to_string());
    }
    lines.push("```".to_string());
}

fn push_text_lines(
    lines: &mut Vec<String>,
    content: &str,
    max_lines: usize,
    section: &str,
    truncated_sections: &mut Vec<String>,
) {
    let all_lines: Vec<&str> = content.lines().collect();
    for line in all_lines.iter().take(max_lines) {
        lines.push((*line).to_string());
    }
    if all_lines.len() > max_lines {
        truncated_sections.push(section.to_string());
        lines.push("… truncated. Use hoverPlus for full detail.".to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hover::*;

    #[test]
    fn truncates_when_exceeding_max_bytes() {
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
                summary: "a".repeat(10_000),
                badges: vec![],
                facts: vec![("k".to_string(), "v".to_string())],
            },
            related: vec![],
            limits: HoverLimits {
                max_markdown_bytes: 256,
                max_json_bytes: 1024,
                truncated_sections: vec![],
            },
        };

        let rendered = render_markdown(&model);
        assert!(rendered.markdown.as_bytes().len() <= 256 + 64);
        assert!(
            rendered.markdown.contains("… truncated"),
            "should include truncation marker"
        );
    }

    #[test]
    fn heading_order_is_stable() {
        let mut model = HoverModel {
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
                summary: "line1\nline2\nline3\nline4".to_string(),
                badges: vec!["ambiguous".to_string()],
                facts: vec![("namespace".to_string(), "default".to_string())],
            },
            related: vec![HoverRelated {
                qualified_name: "default::Y".to_string(),
                kind: "Entity".to_string(),
                relevance_score: 1,
            }],
            limits: HoverLimits {
                max_markdown_bytes: 4096,
                max_json_bytes: 1024,
                truncated_sections: vec![],
            },
        };
        model.symbol.resolution_confidence = "ambiguous".to_string();

        let rendered = render_markdown(&model).markdown;
        let sig = rendered.find("## Signature").unwrap();
        let sum = rendered.find("## Summary").unwrap();
        let facts = rendered.find("## Facts").unwrap();
        let diag = rendered.find("## Diagnostics").unwrap();
        let res = rendered.find("## Resolution").unwrap();
        let related = rendered.find("## Related").unwrap();
        assert!(sig < sum && sum < facts && facts < diag && diag < res && res < related);
        assert_eq!(rendered.matches("## Signature").count(), 1);
        assert_eq!(rendered.matches("```sea").count(), 1);
    }
}
