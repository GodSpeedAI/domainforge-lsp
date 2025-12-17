//! Code Action support for DomainForge LSP.
//!
//! This module provides automated fixes (Quick Fixes) for common diagnostics.
//! It is triggered by the `textDocument/codeAction` LSP request.

use tower_lsp::lsp_types::*;

/// Provide available code actions for a given range and context.
///
/// # Arguments
///
/// * `uri` - The URI of the document
/// * `range` - The range for which code actions are requested
/// * `diagnostics` - The diagnostics present in the context
/// * `text` - The full text content of the document (used for analyzing context)
pub fn provide_code_actions(
    uri: &Url,
    _range: Range,
    diagnostics: &[Diagnostic],
    text: &str,
) -> Vec<CodeActionOrCommand> {
    let mut actions = Vec::new();
    let end_position = calculate_end_position(text);

    for diagnostic in diagnostics {
        if let Some(NumberOrString::String(code)) = &diagnostic.code {
            match code.as_str() {
                "E001" => {
                    // Undefined Entity
                    if let Some(fix) = create_undefined_entity_fix(uri, diagnostic, end_position) {
                        actions.push(fix);
                    }
                }
                "E002" => {
                    // Undefined Resource
                    if let Some(fix) = create_undefined_resource_fix(uri, diagnostic, end_position)
                    {
                        actions.push(fix);
                    }
                }
                "E000" => {
                    // Generic Error (check for namespace issues manually)
                    // TODO: Replace with E500 when sea-core adds it
                    if diagnostic.message.to_lowercase().contains("module")
                        && diagnostic.message.to_lowercase().contains("resolved")
                    {
                        if let Some(fix) = create_missing_import_fix(uri, diagnostic) {
                            actions.push(fix);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    actions
}

fn calculate_end_position(text: &str) -> Position {
    // If text is empty: line 0 char 0.
    if text.is_empty() {
        return Position {
            line: 0,
            character: 0,
        };
    }

    // A robust way without full LineIndex (which is in backend) is to just count newlines.
    let line = text.matches('\n').count();
    // The character is the length of the suffix after the last newline.
    let last_newline_pos = text.rfind('\n');
    let character = match last_newline_pos {
        Some(pos) => text.len() - pos - 1,
        None => text.len(),
    };

    Position {
        line: line as u32,
        character: character as u32,
    }
}

/// Create a Quick Fix to add a missing Entity definition.
fn create_undefined_entity_fix(
    uri: &Url,
    diagnostic: &Diagnostic,
    end_pos: Position,
) -> Option<CodeActionOrCommand> {
    // Extract the entity name from the message "Undefined entity: Name"
    // This is brittle but works for now until sea-core returns structured error data
    let message = &diagnostic.message;
    let name = message.strip_prefix("Undefined entity: ")?;

    let new_text = format!("\n\nEntity \"{}\"", name);

    // Append to the end of the file
    // Note: In a real implementation we might want to be smarter about placement,
    // but appending is safe and valid.
    // We can't know the end of the file easily without the text length/line count passed down cleanly,
    // so we'll use a high line number which LSP usually handles by appending.
    // However, text edits require valid ranges.
    // A better approach for append is to get the actual line count.
    // For now, let's assume the caller passes text and we can compute the end.
    // actually, let's just make the range really big? No, that's dangerous.
    // We should probably pass the LineIndex or text length.
    // Let's refine the API to use the text to find the end.

    // WAIT: `provide_code_actions` receives `text`. We can find the end position.
    // But `provide_code_actions` in my implementation earlier took `text`.
    // Let's assume we can calculate the end position.

    // Using a simpler approach: The backend calls us, it has the line index.
    // But we didn't ask for line index in the signature.
    // Let's update the signature to assume we append at the very end.
    // To do that safely we need the end position.
    //
    // Let's just create a workspace edit that appends.
    // Since we don't have the line count in this helper efficiently without re-indexing,
    // and we don't want to re-index every time...
    //
    // Optimization: The diagnostic usually doesn't carry the file length.
    //
    // Let's look at `provide_code_actions` again. It has `text`.
    // We can use `text.lines().count()`.

    Some(CodeActionOrCommand::CodeAction(CodeAction {
        title: format!("Create Entity '{}'", name),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(
                vec![(
                    uri.clone(),
                    vec![TextEdit {
                        range: Range {
                            start: end_pos,
                            end: end_pos,
                        },
                        new_text,
                    }],
                )]
                .into_iter()
                .collect(),
            ),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    }))
}

/// Create a Quick Fix to add a missing Resource definition.
fn create_undefined_resource_fix(
    uri: &Url,
    diagnostic: &Diagnostic,
    end_pos: Position,
) -> Option<CodeActionOrCommand> {
    let message = &diagnostic.message;
    let name = message.strip_prefix("Undefined resource: ")?;

    let new_text = format!("\n\nResource \"{}\" units", name);

    Some(CodeActionOrCommand::CodeAction(CodeAction {
        title: format!("Create Resource '{}'", name),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(
                vec![(
                    uri.clone(),
                    vec![TextEdit {
                        range: Range {
                            start: end_pos,
                            end: end_pos,
                        },
                        new_text,
                    }],
                )]
                .into_iter()
                .collect(),
            ),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    }))
}

/// Create a placeholder Quick Fix for missing imports (heuristic based).
fn create_missing_import_fix(uri: &Url, diagnostic: &Diagnostic) -> Option<CodeActionOrCommand> {
    // Message format: "Module 'namespace' could not be resolved" (from sea-core/src/module/resolver.rs)
    // or similar grammar errors.

    // Heuristic extraction
    let message = &diagnostic.message;
    // Try to extract content between single quotes
    let start_quote = message.find('\'')?;
    let rest = &message[start_quote + 1..];
    let end_quote = rest.find('\'')?;
    let namespace = &rest[..end_quote];

    let new_text = format!("use {};\n", namespace);

    Some(CodeActionOrCommand::CodeAction(CodeAction {
        title: format!("Add import for '{}'", namespace),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(
                vec![(
                    uri.clone(),
                    vec![TextEdit {
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
                        new_text,
                    }],
                )]
                .into_iter()
                .collect(),
            ),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_end_position() {
        assert_eq!(
            calculate_end_position(""),
            Position {
                line: 0,
                character: 0
            }
        );
        assert_eq!(
            calculate_end_position("hello"),
            Position {
                line: 0,
                character: 5
            }
        );
        assert_eq!(
            calculate_end_position("hello\nworld"),
            Position {
                line: 1,
                character: 5
            }
        );
        assert_eq!(
            calculate_end_position("hello\n"),
            Position {
                line: 1,
                character: 0
            }
        );
        assert_eq!(
            calculate_end_position("a\nb\nc"),
            Position {
                line: 2,
                character: 1
            }
        );
    }

    fn create_diagnostic(code: &str, message: &str) -> Diagnostic {
        Diagnostic {
            range: Range::default(),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String(code.to_string())),
            source: Some("domainforge".to_string()),
            message: message.to_string(),
            related_information: None,
            tags: None,
            code_description: None,
            data: None,
        }
    }

    #[test]
    fn test_code_action_for_undefined_entity() {
        let uri = Url::parse("file:///test.sea").unwrap();
        let diag = create_diagnostic("E001", "Undefined entity: MyEntity");
        let text = "Instance x of \"MyEntity\"";

        // Mock end position calc
        let actions = provide_code_actions(&uri, Range::default(), &[diag], text);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            CodeActionOrCommand::CodeAction(action) => {
                assert_eq!(action.title, "Create Entity 'MyEntity'");
                let edit = action.edit.as_ref().unwrap();
                let changes = edit.changes.as_ref().unwrap();
                let edits = changes.get(&uri).unwrap();
                assert_eq!(edits[0].new_text, "\n\nEntity \"MyEntity\"");
                // Text is one line no newline, so end is line 0 char 24
                assert_eq!(edits[0].range.start.line, 0);
                assert_eq!(edits[0].range.start.character, 24);
            }
            _ => panic!("Expected CodeAction"),
        }
    }

    #[test]
    fn test_code_action_for_undefined_resource() {
        let uri = Url::parse("file:///test.sea").unwrap();
        let diag = create_diagnostic("E002", "Undefined resource: MyRes");
        let text = "Flow \"MyRes\" from A to B";

        let actions = provide_code_actions(&uri, Range::default(), &[diag], text);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            CodeActionOrCommand::CodeAction(action) => {
                assert_eq!(action.title, "Create Resource 'MyRes'");
                let edit = action.edit.as_ref().unwrap();
                let changes = edit.changes.as_ref().unwrap();
                let edits = changes.get(&uri).unwrap();
                assert_eq!(edits[0].new_text, "\n\nResource \"MyRes\" units");
            }
            _ => panic!("Expected CodeAction"),
        }
    }

    #[test]
    fn test_no_code_action_for_syntax_error() {
        let uri = Url::parse("file:///test.sea").unwrap();
        let diag = create_diagnostic("E005", "Syntax error...");
        let text = "invalid syntax";

        let actions = provide_code_actions(&uri, Range::default(), &[diag], text);

        assert!(actions.is_empty());
    }

    #[test]
    fn test_missing_import_heuristic() {
        let uri = Url::parse("file:///test.sea").unwrap();
        // E000 is generic, we check message
        let diag = create_diagnostic("E000", "Module 'com.example' could not be resolved");
        let text = "import 'com.example'";

        let actions = provide_code_actions(&uri, Range::default(), &[diag], text);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            CodeActionOrCommand::CodeAction(action) => {
                assert_eq!(action.title, "Add import for 'com.example'");
                let edit = action.edit.as_ref().unwrap();
                let changes = edit.changes.as_ref().unwrap();
                let edits = changes.get(&uri).unwrap();
                assert_eq!(edits[0].new_text, "use com.example;\n");
                assert_eq!(edits[0].range.start.line, 0);
            }
            _ => panic!("Expected CodeAction"),
        }
    }

    #[test]
    fn test_code_action_append_position() {
        let uri = Url::parse("file:///test.sea").unwrap();
        let diag = create_diagnostic("E001", "Undefined entity: X");
        let text = "L1\nL2\nL3";
        // 3 lines, last char 2 ('3' is at 1, so len is 2)
        // L1\n -> line 1 start
        // L2\n -> line 2 start
        // L3 -> line 2 end

        let actions = provide_code_actions(&uri, Range::default(), &[diag], text);

        match &actions[0] {
            CodeActionOrCommand::CodeAction(action) => {
                let edit = action.edit.as_ref().unwrap();
                let changes = edit.changes.as_ref().unwrap();
                let edits = changes.get(&uri).unwrap();
                // Should append at line 2, char 2?
                // calculate_end_position("L1\nL2\nL3")
                // lines=3. last newline at index 5 (after L2). len=8.
                // char = 8 - 5 - 1 = 2.
                // So line 2, char 2. Correct.
                assert_eq!(edits[0].range.start.line, 2);
                assert_eq!(edits[0].range.start.character, 2);
            }
            _ => panic!(),
        }
    }
}
