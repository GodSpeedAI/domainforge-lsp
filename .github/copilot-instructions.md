# DomainForge LSP - Copilot Instructions

Targeted guidance for AI coding agents working on the `domainforge-lsp` Rust Language Server.

## Architecture & Core Philosophy

**"The LSP is a thin wrapper."**

1.  **Delegate, Don't Duplicate**: This server handles LSP communication (JSON-RPC) and delegates ALL actual work to `sea-core`.

    - **Parsing**: `sea_core::parser::parse_to_graph`
    - **Formatting**: `sea_core::formatter::format`
    - **Diagnostics**: `sea_core::error::diagnostics` (validation)
    - **Semantic Model**: `sea_core::graph::Graph`

2.  **Dependencies**:
    - `sea-core`: The workspace crate containing all logic.
    - `tower-lsp`: The library handling LSP protocol details.
    - `tokio`: Async runtime.

## Common Workflows

### Build

```bash
# Build the server executable
cargo build -p domainforge-lsp

# Build for release (optimized size/speed)
cargo build --release -p domainforge-lsp
```

### Test

```bash
# Run unit tests for the LSP crate
cargo test -p domainforge-lsp
```

## Code Patterns & Conventions

### 1. Handling Updates (`textDocument/didChange`)

Always maintain the latest source in memory (or delegate to a `Backend` struct that holds state). When validation is required, parse the full document via `sea-core` to get a fresh `Graph` or `Ast`.

```rust
// ✅ CORRECT: Re-parse using sea-core
let source = backend.get_document(uri)?;
let graph = sea_core::parse_to_graph(&source)?;

// ❌ WRONG: Attempting to parse regex or fragments manually
if source.contains("Entity") { ... }
```

### 2. Reporting Diagnostics

Map `sea_core::validation_error::ValidationError` directly to LSP `Diagnostic` types.

```rust
// ✅ CORRECT: Use sea-core's diagnostic mapping (or manual mapping if needed)
let diagnostics = errors.iter().map(|e| {
    lsp::Diagnostic {
        range: lsp_range_from_validation_range(e.range()),
        message: e.message().to_string(),
        severity: Some(lsp::DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(e.error_code().as_str().to_string())),
        ..Default::default()
    }
}).collect();
```

### 3. Positioning

`sea-core` uses **1-based** indexing for lines. LSP uses **0-based**.
**ALWAYS** subtract 1 when converting `sea-core` positions to LSP positions.

## Project Structure

- `src/main.rs`: Entry point and `Backend` struct implementation.
- `Cargo.toml`: Must depend on `sea-core` (path dependency).

## References

- `docs/spec.md`: Detailed specification for this LSP and the client.
- `../domainforge/sea-core/grammar/sea.pest`: The authoritative grammar.
