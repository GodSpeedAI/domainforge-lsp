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
    range: Range,
    diagnostics: &[Diagnostic],
    text: &str,
) -> Vec<CodeActionOrCommand> {
    let mut actions = Vec::new();
    let end_position = calculate_end_position(text);

    // Quick fixes based on diagnostics
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
                "E500" => {
                    // Namespace not found - offer to add import
                    if let Some(fix) = create_namespace_import_fix(uri, diagnostic) {
                        actions.push(fix);
                    }
                }
                "E504" => {
                    // Symbol not exported - offer to use wildcard import or suggest available exports
                    if let Some(fix) = create_symbol_export_fix(uri, diagnostic) {
                        actions.push(fix);
                    }
                }
                "E000" => {
                    // Generic Error (legacy fallback for namespace issues)
                    // TODO: Remove this once all namespace errors use E500+
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

    // Refactoring actions based on selection
    actions.extend(provide_refactoring_actions(uri, range, text));

    actions
}

/// Provide refactoring code actions based on the selected range.
///
/// These are not diagnostic-based fixes, but refactoring operations triggered
/// when the user selects text.
pub fn provide_refactoring_actions(
    uri: &Url,
    range: Range,
    text: &str,
) -> Vec<CodeActionOrCommand> {
    let mut actions = Vec::new();

    // Check for Extract to Pattern refactoring
    if let Some(action) = create_extract_to_pattern_action(uri, range, text) {
        actions.push(action);
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
    let suffix = match last_newline_pos {
        Some(pos) => &text[pos + 1..],
        None => text,
    };

    // Convert logic to UTF-16 code unit count as per LSP spec
    let character = suffix.encode_utf16().count();

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

/// Create a Quick Fix for E500: Namespace not found.
/// Generates an import statement for the missing namespace.
fn create_namespace_import_fix(uri: &Url, diagnostic: &Diagnostic) -> Option<CodeActionOrCommand> {
    // Message format: "Namespace 'xxx' not found" or "Namespace 'xxx' not found. Did you mean 'yyy'?"
    let message = &diagnostic.message;

    // Extract namespace name from message
    let start_quote = message.find('\'')?;
    let rest = &message[start_quote + 1..];
    let end_quote = rest.find('\'')?;
    let namespace = &rest[..end_quote];

    // Check for suggestion
    let suggested = if message.contains("Did you mean") {
        // Extract the suggested namespace
        let did_you_mean_idx = message.find("Did you mean")? + "Did you mean '".len();
        let rest_after = &message[did_you_mean_idx..];
        let end_sug = rest_after.find('\'')?;
        Some(&rest_after[..end_sug])
    } else {
        None
    };

    // Use the suggestion if available, otherwise use the original namespace
    let import_ns = suggested.unwrap_or(namespace);
    let new_text = format!(
        "import * as {} from \"{}\"\n",
        import_ns.replace([':', '.'], "_"),
        import_ns
    );

    Some(CodeActionOrCommand::CodeAction(CodeAction {
        title: format!("Add import for '{}'", import_ns),
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

/// Create a Quick Fix for E504: Symbol not exported.
/// Suggests using a wildcard import or lists available exports.
fn create_symbol_export_fix(uri: &Url, diagnostic: &Diagnostic) -> Option<CodeActionOrCommand> {
    // Message format: "Symbol 'xxx' is not exported by module 'yyy'. Available exports: a, b, c"
    let message = &diagnostic.message;

    // Extract module name
    let module_marker = "module '";
    let module_start = message.find(module_marker)? + module_marker.len();
    let rest = &message[module_start..];
    let module_end = rest.find('\'')?;
    let module = &rest[..module_end];

    // Create a wildcard import as a fix
    let new_text = format!(
        "import * as {} from \"{}\"\n",
        module.replace([':', '.'], "_"),
        module
    );

    Some(CodeActionOrCommand::CodeAction(CodeAction {
        title: format!("Import all from '{}' (wildcard)", module),
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
        is_preferred: Some(false), // Not preferred since wildcard imports are less precise
        ..Default::default()
    }))
}

/// Create an "Extract to Pattern" refactoring action.
///
/// This action is offered when the user selects a string literal that looks like
/// a regex pattern. It extracts the string into a named Pattern declaration.
fn create_extract_to_pattern_action(
    uri: &Url,
    range: Range,
    text: &str,
) -> Option<CodeActionOrCommand> {
    // Extract the selected text from the document
    let selected_text = get_text_at_range(text, range)?;

    // Must be a string literal (starts/ends with quotes)
    let trimmed = selected_text.trim();
    if !trimmed.starts_with('"') || !trimmed.ends_with('"') {
        return None;
    }

    // Get the inner content (without quotes)
    let inner = &trimmed[1..trimmed.len() - 1];

    // Check if it looks like a regex pattern
    if !is_regex_pattern(inner) {
        return None;
    }

    // Generate a pattern name from the content
    let pattern_name = generate_pattern_name(inner);

    // Find the best insertion point for the pattern declaration
    let insert_pos = find_pattern_insertion_point(text);

    // Create the Pattern declaration
    let pattern_decl = format!("Pattern \"{}\" matches {}\n\n", pattern_name, trimmed);

    // Create the workspace edit with two changes:
    // 1. Insert the pattern declaration at the appropriate location
    // 2. Optionally: Replace the inline string with a reference (for now, we just add the pattern)
    Some(CodeActionOrCommand::CodeAction(CodeAction {
        title: format!("Extract to Pattern '{}'", pattern_name),
        kind: Some(CodeActionKind::REFACTOR_EXTRACT),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(
                vec![(
                    uri.clone(),
                    vec![TextEdit {
                        range: Range {
                            start: insert_pos,
                            end: insert_pos,
                        },
                        new_text: pattern_decl,
                    }],
                )]
                .into_iter()
                .collect(),
            ),
            ..Default::default()
        }),
        is_preferred: Some(false),
        ..Default::default()
    }))
}

/// Extract text at a given LSP range from the document.
fn get_text_at_range(text: &str, range: Range) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();

    let start_line = range.start.line as usize;
    let end_line = range.end.line as usize;

    if start_line >= lines.len() {
        return None;
    }

    if start_line == end_line {
        // Single line selection
        let line = lines.get(start_line)?;
        let start_char = range.start.character as usize;
        let end_char = range.end.character as usize;

        // Convert from UTF-16 to byte offsets (simplified - assumes ASCII/BMP)
        if start_char > line.len() || end_char > line.len() {
            return None;
        }
        Some(line[start_char..end_char].to_string())
    } else {
        // Multi-line selection
        let mut result = String::new();

        // First line
        if let Some(first_line) = lines.get(start_line) {
            let start_char = range.start.character as usize;
            if start_char <= first_line.len() {
                result.push_str(&first_line[start_char..]);
            }
        }

        // Middle lines
        for line_idx in (start_line + 1)..end_line {
            if let Some(line) = lines.get(line_idx) {
                result.push('\n');
                result.push_str(line);
            }
        }

        // Last line
        if let Some(last_line) = lines.get(end_line) {
            result.push('\n');
            let end_char = range.end.character as usize;
            if end_char <= last_line.len() {
                result.push_str(&last_line[..end_char]);
            }
        }

        Some(result)
    }
}

/// Check if a string looks like a regex pattern.
///
/// Uses heuristics to detect common regex metacharacters and patterns.
fn is_regex_pattern(s: &str) -> bool {
    // Must have some content
    if s.is_empty() || s.len() < 2 {
        return false;
    }

    // Common regex metacharacters and patterns
    let regex_indicators = [
        "^",   // Start anchor
        "$",   // End anchor
        "\\d", // Digit
        "\\w", // Word character
        "\\s", // Whitespace
        "\\b", // Word boundary
        "[",   // Character class start
        "]",   // Character class end
        "*",   // Zero or more
        "+",   // One or more
        "?",   // Optional
        "|",   // Alternation
        "(",   // Group start
        ")",   // Group end
        "{",   // Quantifier start
        "}",   // Quantifier end
        ".",   // Any character (when not escaped)
    ];

    // Count how many regex indicators are present
    let indicator_count = regex_indicators
        .iter()
        .filter(|ind| s.contains(*ind))
        .count();

    // Need at least 2 indicators to be confident it's a regex
    indicator_count >= 2
}

/// Generate a pattern name from regex content.
///
/// Attempts to create a meaningful name based on the regex structure.
fn generate_pattern_name(regex: &str) -> String {
    // Common regex patterns with semantic names
    // Order matters: more specific patterns first

    // Email pattern: contains @ and escaped dot
    if regex.contains("@") && regex.contains("\\.") {
        return "Email".to_string();
    }

    // URL pattern: starts with http or contains ://
    if regex.starts_with("^http") || regex.contains("://") {
        return "Url".to_string();
    }

    // Date format: specific quantifiers like {4} for year
    if regex.contains("\\d{4}") && regex.contains("-") {
        return "DateFormat".to_string();
    }

    // Password: mixed case and digits
    if regex.contains("[A-Z]") && regex.contains("[a-z]") && regex.contains("\\d") {
        return "Password".to_string();
    }

    // Hex string: hex character class
    if regex.contains("[A-Fa-f0-9]") || regex.contains("[0-9a-f]") {
        return "HexString".to_string();
    }

    // Phone/numeric: digits with separators (checked after date format)
    if regex.contains("\\d")
        && (regex.contains("-") || regex.contains("\\."))
        && !regex.contains("@")
    {
        if regex.len() > 10 {
            return "PhoneNumber".to_string();
        }
        return "NumericId".to_string();
    }

    // Default: use a generic name
    "CustomPattern".to_string()
}

/// Find the best position to insert a new Pattern declaration.
///
/// Strategy:
/// 1. After existing Pattern declarations (to group patterns together)
/// 2. Before the first Policy declaration
/// 3. At the start of the file
fn find_pattern_insertion_point(text: &str) -> Position {
    let lines: Vec<&str> = text.lines().collect();
    let mut last_pattern_line: Option<usize> = None;
    let mut first_policy_line: Option<usize> = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("Pattern ") {
            last_pattern_line = Some(i);
        }
        if trimmed.starts_with("Policy ") && first_policy_line.is_none() {
            first_policy_line = Some(i);
        }
    }

    // Insert after the last pattern (on a new line after it)
    if let Some(line) = last_pattern_line {
        return Position {
            line: (line + 1) as u32,
            character: 0,
        };
    }

    // Insert before the first policy
    if let Some(line) = first_policy_line {
        return Position {
            line: line as u32,
            character: 0,
        };
    }

    // Insert at the start of the file
    Position {
        line: 0,
        character: 0,
    }
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
        assert_eq!(
            calculate_end_position("hello\nuni©ode"), // '©' is 2 bytes in UTF-8, 1 unit in UTF-16
            Position {
                line: 1,
                // u, n, i, ©, o, d, e = 7 chars
                character: 7
            }
        );
        assert_eq!(
            calculate_end_position("hello\nuni🤔de"), // '🤔' is 4 bytes in UTF-8, 2 units in UTF-16
            Position {
                line: 1,
                // u, n, i, 🤔(2), d, e = 3 + 2 + 2 = 7 units
                character: 7
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

    #[test]
    fn test_is_regex_pattern() {
        // Should be detected as regex
        assert!(is_regex_pattern("^hello$"));
        assert!(is_regex_pattern("[a-z]+"));
        assert!(is_regex_pattern("\\d{3}-\\d{4}"));
        assert!(is_regex_pattern("(foo|bar)"));
        assert!(is_regex_pattern(".*@.*\\.com"));

        // Should NOT be detected as regex
        assert!(!is_regex_pattern("hello"));
        assert!(!is_regex_pattern("simple text"));
        assert!(!is_regex_pattern(""));
        assert!(!is_regex_pattern("a"));
    }

    #[test]
    fn test_generate_pattern_name() {
        assert_eq!(generate_pattern_name(".*@.*\\.com"), "Email");
        assert_eq!(generate_pattern_name("^https?://"), "Url");
        assert_eq!(generate_pattern_name("\\d{4}-\\d{2}-\\d{2}"), "DateFormat");
        assert_eq!(generate_pattern_name("[A-Fa-f0-9]+"), "HexString");
        assert_eq!(generate_pattern_name("^[a-z]+$"), "CustomPattern");
    }

    #[test]
    fn test_find_pattern_insertion_point() {
        // Empty file
        assert_eq!(
            find_pattern_insertion_point(""),
            Position {
                line: 0,
                character: 0
            }
        );

        // File with existing patterns
        let text = r#"Pattern "Email" matches ".*@.*"
Pattern "Phone" matches "\\d+"
Policy "CheckEmail" when email matches Email"#;
        let pos = find_pattern_insertion_point(text);
        assert_eq!(pos.line, 2); // After the second Pattern line

        // File with policy but no patterns
        let text = r#"Entity "User"
Policy "CheckUser" when user.valid"#;
        let pos = find_pattern_insertion_point(text);
        assert_eq!(pos.line, 1); // Before the Policy line
    }

    #[test]
    fn test_extract_to_pattern_action() {
        let uri = Url::parse("file:///test.sea").unwrap();
        // Text with a regex pattern in a policy expression
        let text = r#"Policy "ValidateEmail" when email matches "^[a-z]+@[a-z]+\\.[a-z]+$""#;

        // Range selecting the regex string literal (from char 43 to 67 inclusive of quotes)
        let range = Range {
            start: Position {
                line: 0,
                character: 42,
            },
            end: Position {
                line: 0,
                character: 68,
            },
        };

        let actions = provide_refactoring_actions(&uri, range, text);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            CodeActionOrCommand::CodeAction(action) => {
                assert!(action.title.contains("Extract to Pattern"));
                assert_eq!(action.kind, Some(CodeActionKind::REFACTOR_EXTRACT));
                let edit = action.edit.as_ref().unwrap();
                let changes = edit.changes.as_ref().unwrap();
                let edits = changes.get(&uri).unwrap();
                assert!(edits[0].new_text.starts_with("Pattern "));
            }
            _ => panic!("Expected CodeAction"),
        }
    }

    #[test]
    fn test_no_extract_for_plain_string() {
        let uri = Url::parse("file:///test.sea").unwrap();
        let text = r#"Entity "User""#;

        // Range selecting the plain string "User"
        let range = Range {
            start: Position {
                line: 0,
                character: 7,
            },
            end: Position {
                line: 0,
                character: 13,
            },
        };

        let actions = provide_refactoring_actions(&uri, range, text);

        // Should not offer Extract to Pattern for plain strings
        assert!(actions.is_empty());
    }

    #[test]
    fn test_e500_code_action() {
        let uri = Url::parse("file:///test.sea").unwrap();
        let diag = create_diagnostic("E500", "Namespace 'com.example' not found");
        let text = "import com.example";

        let actions = provide_code_actions(&uri, Range::default(), &[diag], text);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            CodeActionOrCommand::CodeAction(action) => {
                assert!(action.title.contains("Add import"));
                assert!(action.title.contains("com.example"));
            }
            _ => panic!("Expected CodeAction"),
        }
    }

    #[test]
    fn test_e504_code_action() {
        let uri = Url::parse("file:///test.sea").unwrap();
        let diag = create_diagnostic(
            "E504",
            "Symbol 'Foo' is not exported by module 'com.example'. Available exports: Bar, Baz",
        );
        let text = "import { Foo } from com.example";

        let actions = provide_code_actions(&uri, Range::default(), &[diag], text);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            CodeActionOrCommand::CodeAction(action) => {
                assert!(action.title.contains("Import all from"));
                assert!(action.title.contains("com.example"));
            }
            _ => panic!("Expected CodeAction"),
        }
    }
}
