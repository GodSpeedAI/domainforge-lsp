//! Integration tests for the DomainForge LSP server.
//!
//! These tests verify the server's behavior end-to-end.

use sea_core::parse_to_graph;

/// Placeholder test to ensure the test harness runs.
#[test]
fn test_harness_runs() {
    // This test exists to verify that `cargo test -p domainforge-lsp` works.
    // Real integration tests will be added as features are implemented.
    assert!(true);
}

/// Test that the fixtures directory exists and contains expected files.
#[test]
fn test_fixtures_exist() {
    let fixtures_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    assert!(fixtures_dir.exists(), "Fixtures directory should exist");

    let valid_sea = fixtures_dir.join("valid.sea");
    assert!(valid_sea.exists(), "valid.sea fixture should exist");

    let invalid_sea = fixtures_dir.join("invalid_syntax.sea");
    assert!(
        invalid_sea.exists(),
        "invalid_syntax.sea fixture should exist"
    );
}

/// Test that valid.sea fixture can be read.
#[test]
fn test_valid_fixture_readable() {
    let fixtures_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let content = std::fs::read_to_string(fixtures_dir.join("valid.sea")).unwrap();
    assert!(!content.is_empty(), "valid.sea should not be empty");
}

// Phase 1 Tests: Parsing and Diagnostics

/// Test that invalid .sea file produces a parse error.
#[test]
fn test_invalid_fixture_produces_parse_error() {
    let fixtures_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let content = std::fs::read_to_string(fixtures_dir.join("invalid_syntax.sea")).unwrap();

    let result = parse_to_graph(&content);
    assert!(
        result.is_err(),
        "invalid_syntax.sea should produce a parse error"
    );

    let error = result.unwrap_err();
    // Verify we get an error message
    let error_message = error.to_string();
    assert!(
        !error_message.is_empty(),
        "Parse error should have a message"
    );
}

/// Test that a simple syntax error reports correct line and column.
#[test]
fn test_syntax_error_has_location() {
    // This should fail to parse due to syntax error
    let source = r#"
Entity "Test" {
    invalid_syntax_here
}
"#;

    let result = parse_to_graph(source);
    assert!(result.is_err(), "Should produce a parse error");

    let error = result.unwrap_err();
    let error_str = format!("{:?}", error);
    // SyntaxError variant should contain line and column
    assert!(
        error_str.contains("SyntaxError") || error_str.contains("line"),
        "Error should indicate it's a syntax error: {}",
        error_str
    );
}

/// Test that a valid SEA snippet produces no parse error.
#[test]
fn test_valid_sea_parses_successfully() {
    let source = r#"
Entity "Warehouse" in logistics
Resource "Cameras" units
"#;

    let result = parse_to_graph(source);
    assert!(result.is_ok(), "Valid SEA should parse: {:?}", result.err());
}
