# DomainForge LSP — Phased Implementation Plan

> **Purpose**: MECE (Mutually Exclusive, Collectively Exhaustive) implementation plan for the DomainForge LSP and VS Code extension. Each checkbox represents a discrete unit of work for the implementing agent to mark complete.
>
> **Last Updated**: 2025-12-16

---

## Phase 0: Foundation & CI/CD Infrastructure

> **Goal**: Establish the build, test, and release infrastructure before writing feature code. This prevents technical debt by ensuring all code is linted, tested, and buildable from day one.

### 0.1 LSP Server Repository (`domainforge-lsp`)

- [x] **Cargo.toml Configuration**

  - [x] Add `sea-core` path dependency: `sea-core = { path = "../domainforge/sea-core" }`
  - [x] Add `tower-lsp` dependency with latest version
  - [x] Add `tokio` with `full` features
  - [x] Add `serde` and `serde_json` for JSON-RPC
  - [x] Configure binary target: `[[bin]] name = "domainforge-lsp"`
  - [x] Set edition, version, and metadata fields

- [x] **Project Structure**

  - [x] Create `src/main.rs` with minimal entry point stub
  - [x] Create `src/backend.rs` for `Backend` struct placeholder
  - [x] Create `src/capabilities.rs` for capability declarations
  - [x] Create `src/diagnostics.rs` for diagnostic mapping utilities
  - [x] Create `src/formatting.rs` for format handler

- [x] **Testing Infrastructure**
  - [x] Create `tests/` directory
  - [x] Create `tests/integration.rs` for integration test harness
  - [x] Add test fixtures directory: `tests/fixtures/` with sample `.sea` files
  - [x] Verify `cargo test -p domainforge-lsp` runs (even if empty)

### 0.2 VS Code Extension Repository (`domainforge-vsc-extension`)

- [x] **package.json Completion**

  - [x] Add `contributes.languages` for `domainforge` language ID
  - [x] Add `contributes.grammars` for `.sea` file association
  - [x] Add `activationEvents` for `.sea` files
  - [x] Add `contributes.configuration` schema matching spec section 8
  - [x] Add development dependencies: `@types/vscode`, `esbuild`, `typescript`
  - [x] Add runtime dependency: `vscode-languageclient`

- [x] **Build Configuration**

  - [x] Verify/update `esbuild.js` for production bundling
  - [x] Configure `tsconfig.json` with strict settings
  - [x] Add `pnpm run compile` script
  - [x] Add `pnpm run watch` script for development
  - [x] Add `pnpm run package` script for VSIX creation

- [x] **Testing Infrastructure**
  - [x] Create `.vscode-test.mjs` configuration
  - [x] Create `src/test/` directory structure
  - [x] Add `pnpm test` script
  - [x] Verify test runner executes (even if empty)

### 0.3 CI/CD Pipeline

- [x] **LSP Server CI** (`domainforge-lsp/.github/workflows/ci.yml`)

  - [x] Lint step: `cargo fmt --check && cargo clippy -- -D warnings`
  - [x] Test step: `cargo test -p domainforge-lsp`
  - [x] Build step: `cargo build --release -p domainforge-lsp`
  - [x] Cache Cargo dependencies for faster builds
  - [x] Run on push to `main` and all PRs

- [x] **Extension Client CI** (`domainforge-vsc-extension/.github/workflows/ci.yml`)

  - [x] Lint step: `pnpm lint`
  - [x] Build step: `pnpm run compile`
  - [x] Test step: `pnpm test`
  - [x] Cache `node_modules`
  - [x] Run on push to `main` and all PRs

- [x] **Cross-Compilation Pipeline** (`domainforge-lsp/.github/workflows/release.yml`)

  - [x] Build matrix for targets:
    - [x] `x86_64-unknown-linux-gnu`
    - [x] `x86_64-pc-windows-msvc`
    - [x] `x86_64-apple-darwin`
    - [x] `aarch64-apple-darwin`
  - [x] Upload artifacts per platform
  - [x] Trigger on version tags (`v*.*.*`)

- [x] **VSIX Packaging Pipeline** (`domainforge-vsc-extension/.github/workflows/release.yml`)
  - [x] Download platform binaries from LSP release
  - [x] Bundle binaries into extension
  - [x] Run `vsce package`
  - [x] Upload VSIX artifact
  - [x] Trigger on version tags

---

## Phase 1: Minimal LSP Server (Text Sync + Diagnostics)

> **Goal**: Deliver a working LSP that opens `.sea` files and shows parse errors. This is the minimum viable product.

### 1.1 Tower-LSP Scaffold

- [x] **Backend Struct Implementation**

  - [x] Define `Backend` struct with `RwLock<HashMap<Url, String>>` for document storage
  - [x] Implement `tower_lsp::LanguageServer` trait for `Backend`
  - [x] Implement `initialize` with capability declaration
  - [x] Implement `initialized` with logging
  - [x] Implement `shutdown`

- [x] **Text Document Sync**

  - [x] Implement `textDocument/didOpen` — store full content
  - [x] Implement `textDocument/didChange` — apply incremental changes
  - [x] Implement `textDocument/didClose` — remove from storage
  - [x] Implement `textDocument/didSave` — trigger validation

- [x] **Server Entry Point**
  - [x] Configure `tokio` runtime in `main.rs`
  - [x] Create `tower_lsp::Server` with stdio transport
  - [x] Wire `Backend` to server
  - [x] Add startup logging to stderr

### 1.2 Diagnostics Integration

- [x] **Validation Pipeline**

  - [x] On open/change/save: call `sea_core::parser::parse_to_graph`
  - [x] Capture `ParseResult` errors
  - [x] If additional validation exists, call `sea_core` validation

- [x] **Diagnostic Mapping**

  - [x] Create `fn parse_error_to_diagnostic(e: &ParseError) -> lsp::Diagnostic`
  - [x] Map `sea_core::validation_error::SourceRange` → `lsp::Range` (subtract 1 for 0-indexing)
  - [x] Map `ErrorCode` → `diagnostic.code`
  - [x] Map error message → `diagnostic.message`
  - [x] Map severity appropriately (Error/Warning/Info/Hint)

- [x] **Publish Diagnostics**
  - [x] Call `client.publish_diagnostics(uri, diagnostics, version)` after validation
  - [x] Clear diagnostics when document closes

### 1.3 Unit Tests for Phase 1

- [x] **Test Document Storage**

  - [x] Test `didOpen` stores content correctly
  - [x] Test `didChange` applies incremental edits
  - [x] Test `didClose` removes content

- [x] **Test Diagnostic Mapping**
  - [x] Test valid `.sea` produces empty diagnostics
  - [x] Test syntax error produces E005 diagnostic
  - [x] Test undefined entity produces E001 diagnostic
  - [x] Test range conversion is correct (1-based → 0-based)

---

## Phase 2: Formatting Handler

> **Goal**: Enable `Format Document` command in VS Code using `sea-core` formatter.

### 2.1 Format Provider

- [x] **Capability Declaration**

  - [x] Add `documentFormattingProvider: true` to server capabilities

- [x] **Handler Implementation**

  - [x] Implement `textDocument/formatting` request handler
  - [x] Extract formatting options from request (indent style, width)
  - [x] Map to `sea_core::formatter::FormatConfig`
  - [x] Call `sea_core::formatter::format(source, config)`
  - [x] Return `Vec<TextEdit>` replacing entire document (or compute minimal diff)

- [x] **Error Handling**
  - [x] If source has parse errors, return empty edits (don't format broken code)
  - [x] Log formatting errors to stderr for debugging

### 2.2 Configuration Sync

- [x] **Server-Side Config**

  - [x] Define config struct matching spec section 8 options
  - [x] Implement `workspace/didChangeConfiguration` handler
  - [x] Store configuration in `Backend`

- [x] **Client-Side Config Forwarding**
  - [x] In extension, listen for configuration changes
  - [x] Send `workspace/didChangeConfiguration` notification to server
  - [x] Include relevant `domainforge.*` settings

### 2.3 Unit Tests for Phase 2

- [x] **Test Formatting**
  - [x] Test well-formed `.sea` file returns formatted output
  - [x] Test malformed `.sea` file returns empty edits
  - [x] Test indent style option is respected
  - [x] Test indent width option is respected

---

## Phase 3: VS Code Client Integration

> **Goal**: Complete the VS Code extension so it spawns the server and routes LSP messages.

### 3.1 Language Client Setup

- [ ] **Extension Activation**

  - [ ] Implement `activate` function in `extension.ts`
  - [ ] Register `domainforge` language
  - [ ] Register `.sea` file association

- [ ] **Server Process Spawn**

  - [ ] Detect current platform (`linux`, `darwin`, `win32`, `darwin-arm64`)
  - [ ] Locate bundled binary path based on platform
  - [ ] Configure `ServerOptions` with executable path and stdio transport
  - [ ] Handle missing binary gracefully with user notification

- [ ] **Client Options**

  - [ ] Configure `documentSelector` for `domainforge` language
  - [ ] Configure `synchronize.configurationSection` for `domainforge`
  - [ ] Enable `middleware` for debugging if needed

- [ ] **Lifecycle Management**
  - [ ] Start client on activation
  - [ ] Implement `deactivate` to stop client cleanly
  - [ ] Handle server crash with restart logic

### 3.2 Manual Integration Testing

- [ ] **Test Extension Loads**

  - [ ] Open VS Code with extension installed
  - [ ] Open a `.sea` file
  - [ ] Verify status bar shows language as `DomainForge`

- [ ] **Test Diagnostics Appear**

  - [ ] Create `.sea` file with syntax error
  - [ ] Verify red squiggle appears under error
  - [ ] Verify Problems panel shows diagnostic

- [ ] **Test Formatting Works**
  - [ ] Open valid `.sea` file with inconsistent indentation
  - [ ] Execute `Format Document` command
  - [ ] Verify file is reformatted

---

## Phase 4: Advanced Language Features

> **Goal**: Implement completion, hover, go-to-definition, and find-references for rich IDE experience.

### 4.1 Completion Provider

- [ ] **Capability Declaration**

  - [ ] Add `completionProvider` with triggerCharacters: `"`, `@`, `.`

- [ ] **Implementation**

  - [ ] Implement `textDocument/completion` handler
  - [ ] Parse current document to get `Graph`
  - [ ] Extract all entity names → completion items
  - [ ] Extract all resource names → completion items
  - [ ] Extract namespace prefixes from imports → completion items
  - [ ] Set appropriate `CompletionItemKind` for each type

- [ ] **Context-Aware Completion**
  - [ ] After `of "` suggest entity names
  - [ ] After `from "` or `to "` suggest entity names
  - [ ] After `Flow "` suggest resource names

### 4.2 Hover Provider

> **Architecture**: Implement the canonical hover model from `hover_plan.yml` with dual endpoints for human and machine consumption.

- [ ] **State Management**

  - [ ] Refactor `Backend` to cache `Graph` per document (see Phase 4.2.1)
  - [ ] Add `DocumentState` struct with `text`, `version`, and `graph` fields
  - [ ] Update all document sync handlers to maintain cached graph
  - [ ] Implement cache invalidation on document changes

- [ ] **Canonical Hover Model**

  - [ ] Create `src/hover/mod.rs` module
  - [ ] Define `HoverModel` struct matching `hover_plan.yml` schema
  - [ ] Implement required fields: `schema_version`, `id`, `symbol`, `context`, `primary`, `limits`
  - [ ] Implement `symbol` resolution: name, kind, qualified_name, uri, range, resolve_id
  - [ ] Implement `context` extraction: document_version, position, scope_summary, config_hash
  - [ ] Implement `primary` section: header, signature_or_shape, summary, badges

- [ ] **DSL Adaptation Layer**

  - [ ] Create `src/hover/symbol_resolver.rs`
  - [ ] Implement position-to-symbol lookup in `Graph`
  - [ ] Support symbol kinds: Entity, Resource, Flow, Instance, Role, Relation, Pattern
  - [ ] Build qualified identity: module path + symbol name
  - [ ] Extract interpretation context: resolved target, scope, environment
  - [ ] Implement shape/type extraction from DSL primitives

- [ ] **Standard LSP Hover Endpoint**

  - [ ] Add `hoverProvider: true` to server capabilities
  - [ ] Implement `textDocument/hover` handler
  - [ ] Build `HoverModel` from cursor position
  - [ ] Render `HoverModel` to Markdown via pure function
  - [ ] Return `MarkupContent` with markdown format
  - [ ] Implement payload limits: max 32KB, max 2 code blocks, max 40 lines per block
  - [ ] Add truncation markers when limits exceeded

- [ ] **HoverPlus Custom Endpoint**

  - [ ] Implement `textDocument/hoverPlus` custom LSP method
  - [ ] Accept optional parameters: `include_markdown`, `include_project_signals`, `max_detail_level`
  - [ ] Return full `HoverModel` as JSON
  - [ ] Optionally include pre-rendered markdown
  - [ ] Support detail levels: `core`, `standard`, `deep`
  - [ ] Implement payload limits: max 128KB for JSON

- [ ] **Markdown Renderer**

  - [ ] Create `src/hover/markdown_renderer.rs`
  - [ ] Implement pure function: `HoverModel -> MarkdownString`
  - [ ] Follow heading order: Signature, Summary, Facts, Diagnostics, Resolution, Expansion, Usage, Related
  - [ ] Render signature/shape as code block
  - [ ] Render badges as compact bullet list
  - [ ] Implement progressive disclosure sections (expandable)
  - [ ] Apply truncation rules per section

- [ ] **Performance Optimization**

  - [ ] Implement LRU cache for `HoverModel` (512 entries)
  - [ ] Implement LRU cache for rendered markdown (256 entries)
  - [ ] Cache key: `(uri, version, position, view_kind)`
  - [ ] Set compute budget: 40ms CPU time
  - [ ] Implement graceful degradation on budget exceed
  - [ ] Target latencies: p50 < 100ms, p95 < 250ms (warm)

- [ ] **Determinism Guarantees**

  - [ ] Ensure same snapshot produces byte-identical `HoverModel`
  - [ ] Sort all lists deterministically (relevance desc, then name asc)
  - [ ] Use stable hashing for `hover_id` generation
  - [ ] Exclude timestamps from content

### 4.3 Go to Definition Provider

- [ ] **Capability Declaration**

  - [ ] Add `definitionProvider: true`

- [ ] **Implementation**

  - [ ] Implement `textDocument/definition` handler
  - [ ] Find identifier at cursor position
  - [ ] Look up definition location in `Graph`
  - [ ] Return `Location` pointing to declaration

- [ ] **Supported Targets**
  - [ ] Entity references in `of "EntityName"`
  - [ ] Entity references in `from "EntityName"` / `to "EntityName"`
  - [ ] Resource references in `Flow "ResourceName"`
  - [ ] Instance references

### 4.4 Find References Provider

- [ ] **Capability Declaration**

  - [ ] Add `referencesProvider: true`

- [ ] **Implementation**
  - [ ] Implement `textDocument/references` handler
  - [ ] Find symbol at cursor position
  - [ ] Scan `Graph` for all references to that symbol
  - [ ] Return `Vec<Location>` with all reference sites

### 4.5 Unit Tests for Phase 4

- [ ] **Test Completion**

  - [ ] Test entity names appear after `of "`
  - [ ] Test resource names appear in flow context
  - [ ] Test no duplicates in completion list

- [ ] **Test Hover - Golden Snapshots**

  - [ ] Test Entity hover shows expected metadata (name, namespace, annotations)
  - [ ] Test Resource hover shows name, unit, namespace
  - [ ] Test Flow hover shows resource, source, target
  - [ ] Test hovering whitespace returns nothing
  - [ ] Test Rule reference resolution
  - [ ] Test ambiguous reference handling
  - [ ] Test normalized form display
  - [ ] Test diagnostics with constraint failure + fixes
  - [ ] Test deprecated symbol with since metadata
  - [ ] Test truncation markers when limits exceeded

- [ ] **Test Hover - Determinism**

  - [ ] Test identical output for same (uri, version, position)
  - [ ] Test heading order is stable across runs
  - [ ] Test no duplicate signature in output
  - [ ] Test lists are sorted deterministically

- [ ] **Test Hover - Performance**

  - [ ] Test hover response time < 250ms (p95, warm cache)
  - [ ] Test hover response time < 500ms (p95, cold cache)
  - [ ] Test payload never exceeds 32KB for markdown
  - [ ] Test payload never exceeds 128KB for JSON
  - [ ] Test cache hit rate > 80% for repeated hovers

- [ ] **Test HoverPlus Endpoint**

  - [ ] Test `textDocument/hoverPlus` returns valid JSON
  - [ ] Test detail level parameter is respected
  - [ ] Test `include_markdown` parameter works
  - [ ] Test response includes all required HoverModel fields

- [ ] **Test Go to Definition**

  - [ ] Test navigation from `Instance x of "Entity"` to Entity declaration
  - [ ] Test navigation from Flow endpoint to Entity

- [ ] **Test Find References**
  - [ ] Test finding all uses of an Entity

---

## Phase 5: Code Actions & Quick Fixes

> **Goal**: Provide automated fixes for common errors.

### 5.1 Quick Fix: Undefined Reference

- [ ] **Detection**

  - [ ] Identify E001 (UndefinedEntity) and E002 (UndefinedResource) diagnostics

- [ ] **Fix Generation**
  - [ ] Offer "Create Entity 'X'" action
  - [ ] Offer "Create Resource 'X'" action
  - [ ] Generate declaration stub at appropriate location

### 5.2 Quick Fix: Add Missing Import

- [ ] **Detection**

  - [ ] Identify E500 (NamespaceNotFound) diagnostics

- [ ] **Fix Generation**
  - [ ] Offer "Add import for 'namespace'" action
  - [ ] Insert `use namespace;` at file top

### 5.3 Refactoring: Extract to Pattern

- [ ] **Trigger**

  - [ ] User selects text matching a regex-like expression

- [ ] **Action**
  - [ ] Offer "Extract to Pattern" action
  - [ ] Generate `Pattern "Name" matches "..."` declaration
  - [ ] Replace inline regex with pattern reference

### 5.4 Unit Tests for Phase 5

- [ ] **Test Undefined Reference Fix**

  - [ ] Verify code action appears for E001
  - [ ] Verify applying action creates Entity stub

- [ ] **Test Missing Import Fix**
  - [ ] Verify code action appears for E500
  - [ ] Verify applying action adds import

---

## Phase 6: WASM Web Extension Support

> **Goal**: Enable `sea-core` to run in browser for vscode.dev support.

### 6.1 WASM Build Target

- [ ] **sea-core WASM Verification**

  - [ ] Verify `sea-core` builds with `wasm32-unknown-unknown` target
  - [ ] Verify formatter feature works in WASM
  - [ ] Verify parser works in WASM

- [ ] **LSP WASM Considerations**
  - [ ] Research `tower-lsp` WASM compatibility
  - [ ] Determine if full server WASM or TypeScript wrapper around WASM core

### 6.2 Web Extension Manifest

- [ ] **package.json Updates**

  - [ ] Add `browser` entry point
  - [ ] Add `web` extension kind
  - [ ] Bundle WASM binary

- [ ] **Fallback Strategy**
  - [ ] Detect if running in browser vs desktop
  - [ ] Use WASM in browser, native binary in desktop

---

## Phase 8: MCP Server Integration

> **Goal**: Enable AI agents (e.g., VS Code Copilot, Claude) to query the DomainForge LSP via the Model Context Protocol (MCP). The MCP server acts as a safe, controlled bridge between AI agents and the language server.

### 8.1 Architecture Overview

- [ ] **Component Topology**

  - [ ] Document the data flow: `VS Code Extension (Node/TS) → MCP Server (Rust) → Rust LSP (stdio)`
  - [ ] Define MCP server as a separate binary target in `Cargo.toml`: `[[bin]] name = "domainforge-mcp"`
  - [ ] Design the MCP server to spawn/connect to the LSP server per workspace
  - [ ] Treat the LSP as the single source of truth for all language features

- [ ] **Crate Structure**
  - [ ] Create `src/mcp/mod.rs` for MCP server implementation
  - [ ] Create `src/mcp/tools.rs` for exposed tool definitions
  - [ ] Create `src/mcp/guardrails.rs` for security and rate limiting
  - [ ] Create `src/mcp/transport.rs` for MCP protocol handling

### 8.2 MCP Tool Exposure

> **Principle**: Expose read-only, safe operations. No auto-apply mutations without human confirmation.

- [ ] **Diagnostics Tool**

  - [ ] Implement `domainforge/diagnostics` tool
  - [ ] Accept: `uri: string` (file path)
  - [ ] Return: Array of diagnostics with severity, message, range, code
  - [ ] Rate limit: Max 10 requests/second per workspace

- [ ] **Hover Tool**

  - [ ] Implement `domainforge/hover` tool
  - [ ] Accept: `uri: string`, `line: number`, `character: number`
  - [ ] Reuse `HoverModel` builder from Phase 4.2
  - [ ] Return: Hover content (markdown) or null
  - [ ] Optionally return full `HoverModel` JSON for agent consumption
  - [ ] Rate limit: Max 20 requests/second per workspace

- [ ] **Definition Tool**

  - [ ] Implement `domainforge/definition` tool
  - [ ] Accept: `uri: string`, `line: number`, `character: number`
  - [ ] Return: Location(s) of definition or empty array
  - [ ] Rate limit: Max 10 requests/second per workspace

- [ ] **References Tool**

  - [ ] Implement `domainforge/references` tool
  - [ ] Accept: `uri: string`, `line: number`, `character: number`, `includeDeclaration: boolean`
  - [ ] Return: Array of locations
  - [ ] Rate limit: Max 5 requests/second per workspace

- [ ] **Rename Preview Tool**

  - [ ] Implement `domainforge/rename-preview` tool
  - [ ] Accept: `uri: string`, `line: number`, `character: number`, `newName: string`
  - [ ] Return: `WorkspaceEdit` preview (NOT applied automatically)
  - [ ] Flag response with `requiresHumanApproval: true`
  - [ ] Rate limit: Max 2 requests/second per workspace

- [ ] **Code Actions Tool (Read-Only)**
  - [ ] Implement `domainforge/code-actions` tool
  - [ ] Accept: `uri: string`, `range: Range`, `context: CodeActionContext`
  - [ ] Return: Array of available code actions with titles and kinds
  - [ ] Do NOT auto-apply; return `WorkspaceEdit` preview for each action
  - [ ] Flag response with `requiresHumanApproval: true`
  - [ ] Rate limit: Max 5 requests/second per workspace

### 8.3 Guardrails & Security

- [ ] **Path Allowlists**

  - [ ] Accept workspace root(s) at MCP server initialization
  - [ ] Validate all `uri` parameters against allowlist before processing
  - [ ] Reject requests for files outside allowed paths with clear error
  - [ ] Support glob patterns for fine-grained access control

- [ ] **Repo-Scoped Access**

  - [ ] Bind MCP server instance to specific workspace/repository
  - [ ] Prevent cross-workspace information leakage
  - [ ] Include workspace identifier in all tool responses

- [ ] **Rate Limiting**

  - [ ] Implement token bucket rate limiter per tool type
  - [ ] Configure limits via environment variables or config file
  - [ ] Return `429 Too Many Requests` equivalent for exceeded limits
  - [ ] Log rate limit violations for monitoring

- [ ] **Human-in-the-Loop Apply**

  - [ ] Never auto-apply `WorkspaceEdit` mutations
  - [ ] Return edits as preview data with `requiresHumanApproval: true`
  - [ ] Document the expected client-side confirmation flow
  - [ ] Optionally implement `domainforge/apply-edit` tool gated by explicit user confirmation token

- [ ] **Audit Logging**
  - [ ] Log all MCP tool invocations with timestamp, tool name, parameters (sanitized)
  - [ ] Log all denied requests with reason
  - [ ] Support configurable log destinations (stderr, file, external service)

### 8.4 MCP Server Entry Point

- [ ] **Binary Configuration**

  - [ ] Add `src/mcp/main.rs` as entry point for `domainforge-mcp` binary
  - [ ] Accept `--workspace-root` argument for path allowlist initialization
  - [ ] Accept `--lsp-path` argument to locate the LSP server binary
  - [ ] Support `--config` for JSON/TOML configuration file

- [ ] **LSP Server Management**

  - [ ] Spawn LSP server as child process with stdio transport
  - [ ] Implement reconnection logic on LSP server crash
  - [ ] Forward initialization params from MCP config to LSP

- [ ] **MCP Protocol Transport**
  - [ ] Implement MCP protocol over stdio (for CLI/agent integration)
  - [ ] Optionally support HTTP SSE transport for web-based agents
  - [ ] Handle MCP `initialize`, `tools/list`, `tools/call` methods

### 8.5 VS Code Extension Integration

- [ ] **MCP Server Lifecycle**

  - [ ] Spawn `domainforge-mcp` alongside or instead of direct LSP spawn
  - [ ] Pass workspace folder paths as allowlist
  - [ ] Handle MCP server process lifecycle (start, stop, restart)

- [ ] **Configuration Options**
  - [ ] Add `domainforge.mcp.enable: boolean` setting (default: false)
  - [ ] Add `domainforge.mcp.rateLimits` object for per-tool limits
  - [ ] Add `domainforge.mcp.auditLog.path` for log file location

### 8.6 Unit Tests for Phase 8

- [ ] **Test Tool Responses**

  - [ ] Test `domainforge/diagnostics` returns correct format
  - [ ] Test `domainforge/hover` returns markdown content
  - [ ] Test `domainforge/definition` returns valid locations
  - [ ] Test `domainforge/references` includes declaration when requested
  - [ ] Test `domainforge/rename-preview` returns non-applied edit
  - [ ] Test `domainforge/code-actions` returns edits with approval flag

- [ ] **Test Guardrails**

  - [ ] Test path allowlist rejects out-of-workspace files
  - [ ] Test rate limiter correctly throttles requests
  - [ ] Test denied requests are logged

- [ ] **Test LSP Integration**
  - [ ] Test MCP server correctly forwards requests to LSP
  - [ ] Test MCP server handles LSP server restart gracefully
  - [ ] Test workspace scoping prevents cross-workspace access

---

## Phase 9: Release & Distribution

> **Goal**: Publish to VS Code Marketplace and establish release process.

### 7.1 Marketplace Preparation

- [ ] **Extension Metadata**

  - [ ] Add icon: 128x128 PNG
  - [ ] Write README with feature list and screenshots
  - [ ] Add CHANGELOG.md
  - [ ] Add LICENSE file
  - [ ] Set publisher ID in package.json

- [ ] **Quality Checklist**
  - [ ] All CI checks pass
  - [ ] Manual testing on Windows, macOS, Linux
  - [ ] README has installation instructions
  - [ ] README has usage examples

### 7.2 Publish Pipeline

- [ ] **Marketplace Credentials**

  - [ ] Set up Azure DevOps PAT for publishing
  - [ ] Store as GitHub secret

- [ ] **Automated Publish**
  - [ ] Add publish step to release workflow
  - [ ] Run `vsce publish` after VSIX is built
  - [ ] Create GitHub Release with VSIX attachment

### 7.3 Version Management

- [ ] **Semantic Versioning**

  - [ ] Establish versioning policy (e.g., major for breaking LSP changes)
  - [ ] Sync version between `Cargo.toml` and `package.json`

- [ ] **Release Checklist**
  - [ ] Update CHANGELOG
  - [ ] Bump version in manifests
  - [ ] Create annotated git tag
  - [ ] Push tag to trigger release workflow

---

## Verification Plan Summary

| Phase | Verification Method       | Command/Steps                                                                |
| ----- | ------------------------- | ---------------------------------------------------------------------------- |
| 0     | CI passes                 | `cargo test`, `pnpm test`, GitHub Actions green                              |
| 1     | Unit tests + manual       | `cargo test -p domainforge-lsp`, open `.sea` in VS Code                      |
| 2     | Unit tests + manual       | `cargo test`, execute Format Document command                                |
| 3     | Integration test + manual | `pnpm test`, open extension in debug host, check diagnostics                 |
| 4     | Unit tests                | `cargo test` for all providers                                               |
| 5     | Unit tests                | `cargo test` for code actions                                                |
| 6     | WASM tests + manual       | Test in vscode.dev (if pursued)                                              |
| 8     | Unit + integration tests  | `cargo test -p domainforge-mcp`, test MCP tools via agent, verify guardrails |
| 9     | Manual                    | Install from VSIX, verify all features                                       |

---

## Technical Debt Prevention Checklist

> This section ensures no shortcuts are taken that would create debt.

- [ ] **No Grammar Re-Implementation**: All parsing uses `sea-core` — no regex parsing in LSP
- [ ] **No Position Bugs**: All `sea-core` → LSP range conversions tested
- [ ] **No Hardcoded Paths**: Binary paths computed dynamically based on platform
- [ ] **No Skipped Tests**: Each phase adds tests before marking complete
- [ ] **No Unhandled Errors**: All `Result`/`Option` types properly handled with logging
- [ ] **No Configuration Drift**: Client and server use identical config schema
- [ ] **No Dead Code**: `cargo clippy` and `pnpm lint` enforced in CI
- [ ] **No Undocumented Features**: README updated as features are added
- [ ] **No Unsafe MCP Mutations**: All MCP tools that return edits require human approval
- [ ] **No Path Traversal**: MCP server enforces workspace path allowlists strictly
- [ ] **No Rate Limit Bypass**: Rate limiting tested under load to prevent abuse

---

## Notes for Implementing Agent

1. **Order Matters**: Complete Phase 0 first. CI must be green before feature work.
2. **Check Off As You Go**: Mark `[x]` as each item is completed.
3. **Commit Granularly**: One commit per logical unit (e.g., "Add didOpen handler").
4. **Test Before Proceeding**: Don't move to next phase until current phase tests pass.
5. **Ask When Blocked**: If `sea-core` API differs from spec, clarify before proceeding.
