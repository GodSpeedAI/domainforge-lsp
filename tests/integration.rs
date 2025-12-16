//! Integration tests for the DomainForge LSP server.
//!
//! These tests verify the server's behavior end-to-end.

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
