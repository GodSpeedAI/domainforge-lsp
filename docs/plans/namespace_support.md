# Plan: Proper Namespace Import Support (Phase 5.2)

## Context

Code actions for "Add missing import" are currently implemented via heuristic string matching on generic grammar error messages (`E000`). This is brittle and depends on specific error message formatting from `sea-core`.

## Objective

Implement robust, structured namespace resolution errors and corresponding code actions.

## Strategy

1.  **sea-core Update**:

    - Introduce a dedicated `ParseError` variant: `NamespaceNotFound(String)` or similar.
    - Ensure the error contains the raw unresolved namespace string.
    - Assign a stable error code (e.g., `E500`).

2.  **LSP Handover**:

    - Update `parse_error_to_diagnostic` in `diagnostics.rs` to map `NamespaceNotFound` to `E500`.
    - Update `code_actions.rs` to match on `E500` specifically, removing the heuristic `E000` matching.

3.  **Semantic Index Integration**:
    - Eventually, the LSP should query the `SemanticIndex` to suggest _available_ namespaces that match fuzzy queries, rather than just using the exact string in the error.

## Success Criteria

- `sea-core` emits structured `NamespaceNotFound` errors.
- LSP correctly identifies `E500`.
- Code action suggests valid `use <namespace>;` statements.
- No reliance on `error.message.contains("...")`.
