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

- [x] **Extension Activation**

  - [x] Implement `activate` function in `extension.ts`
  - [x] Register `domainforge` language
  - [x] Register `.sea` file association

- [x] **Server Process Spawn**

  - [x] Detect current platform (`linux`, `darwin`, `win32`, `darwin-arm64`)
  - [x] Locate bundled binary path based on platform
  - [x] Configure `ServerOptions` with executable path and stdio transport
  - [x] Handle missing binary gracefully with user notification

- [x] **Client Options**

  - [x] Configure `documentSelector` for `domainforge` language
  - [x] Configure `synchronize.configurationSection` for `domainforge`
  - [x] Enable `middleware` for debugging if needed

- [x] **Lifecycle Management**
  - [x] Start client on activation
  - [x] Implement `deactivate` to stop client cleanly
  - [x] Handle server crash with restart logic

### 3.2 Manual Integration Testing

- [x] **Test Extension Loads**

  - [x] Open VS Code with extension installed
  - [x] Open a `.sea` file
  - [x] Verify status bar shows language as `DomainForge`

- [ ] **Test Diagnostics Appear**

  - [ ] Create `.sea` file with syntax error
  - [ ] Verify red squiggle appears under error
  - [ ] Verify Problems panel shows diagnostic

- [ ] **Test Formatting Works**
  - [ ] Open valid `.sea` file with inconsistent indentation
  - [ ] Execute `Format Document` command
  - [ ] Verify file is reformatted

> **Test diagnostics appear and formatting works tests require the actual compiled LSP binary to be present in the bin/ directory, which happens during the full release process or by manually downloading artifacts.**

---

## Phase 4: Advanced Language Features

> **Goal**: Implement completion, hover, go-to-definition, and find-references for rich IDE experience.

### 4.1 Completion Provider

- [x] **Capability Declaration**

  - [x] Add `completionProvider` with triggerCharacters: `"`, `@`, `.`

- [x] **Implementation**

  - [x] Implement `textDocument/completion` handler
  - [x] Parse current document to get `Graph`
  - [x] Extract all entity names → completion items
  - [x] Extract all resource names → completion items
  - [x] Extract namespace prefixes from imports → completion items
  - [x] Set appropriate `CompletionItemKind` for each type

- [x] **Context-Aware Completion**
  - [x] After `of "` suggest entity names
  - [x] After `from "` or `to "` suggest entity names
  - [x] After `Flow "` suggest resource names

### 4.2 Hover Provider

> **Architecture**: Implement the canonical hover model from `hover_plan.yml` with dual endpoints for human and machine consumption.

- [x] **State Management**

  - [x] Refactor `Backend` to cache `Graph` per document (see Phase 4.2.1)
  - [x] Add `DocumentState` struct with `text`, `version`, and `graph` fields
  - [x] Update all document sync handlers to maintain cached graph
  - [x] Implement cache invalidation on document changes

- [x] **Canonical Hover Model**

  - [x] Create `src/hover/mod.rs` module
  - [x] Define `HoverModel` struct matching `hover_plan.yml` schema
  - [x] Implement required fields: `schema_version`, `id`, `symbol`, `context`, `primary`, `limits`
  - [x] Implement `symbol` resolution: name, kind, qualified_name, uri, range, resolve_id
  - [x] Implement `context` extraction: document_version, position, scope_summary, config_hash
  - [x] Implement `primary` section: header, signature_or_shape, summary, badges

- [x] **DSL Adaptation Layer**

  - [x] Create `src/hover/symbol_resolver.rs`
  - [x] Implement position-to-symbol lookup in `Graph`
  - [x] Support symbol kinds: Entity, Resource, Flow, Instance, Role, Relation, Pattern
  - [x] Build qualified identity: module path + symbol name
  - [x] Extract interpretation context: resolved target, scope, environment
  - [x] Implement shape/type extraction from DSL primitives

- [x] **Standard LSP Hover Endpoint**

  - [x] Add `hoverProvider: true` to server capabilities
  - [x] Implement `textDocument/hover` handler
  - [x] Build `HoverModel` from cursor position
  - [x] Render `HoverModel` to Markdown via pure function
  - [x] Return `MarkupContent` with markdown format
  - [x] Implement payload limits: max 32KB, max 2 code blocks, max 40 lines per block
  - [x] Add truncation markers when limits exceeded

- [x] **HoverPlus Custom Endpoint**

  - [x] Implement `textDocument/hoverPlus` custom LSP method
  - [x] Accept optional parameters: `include_markdown`, `include_project_signals`, `max_detail_level`
  - [x] Return full `HoverModel` as JSON
  - [x] Optionally include pre-rendered markdown
  - [x] Support detail levels: `core`, `standard`, `deep`
  - [x] Implement payload limits: max 128KB for JSON

- [x] **Markdown Renderer**

  - [x] Create `src/hover/markdown_renderer.rs`
  - [x] Implement pure function: `HoverModel -> MarkdownString`
  - [x] Follow heading order: Signature, Summary, Facts, Diagnostics, Resolution, Expansion, Usage, Related
  - [x] Render signature/shape as code block
  - [x] Render badges as compact bullet list
  - [x] Implement progressive disclosure sections (expandable)
  - [x] Apply truncation rules per section

- [ ] **Performance Optimization**

  - [x] Implement LRU cache for `HoverModel` (512 entries)
  - [x] Implement LRU cache for rendered markdown (256 entries)
  - [x] Cache key: `(uri, version, position, view_kind)`
  - [x] Set compute budget: 40ms CPU time
  - [x] Implement graceful degradation on budget exceed
  - [ ] Target latencies: p50 < 100ms, p95 < 250ms (warm) (not enforceable in deterministic unit tests)

- [x] **Determinism Guarantees**

  - [x] Ensure same snapshot produces byte-identical `HoverModel`
  - [x] Sort all lists deterministically (relevance desc, then name asc)
  - [x] Use stable hashing for `hover_id` generation
  - [x] Exclude timestamps from content

### 4.3 Go to Definition Provider

- [x] **Capability Declaration**

  - [x] Add `definitionProvider: true`

- [x] **Implementation**

  - [x] Implement `textDocument/definition` handler
  - [x] Find identifier at cursor position
  - [x] Look up definition location in `Graph`
  - [x] Return `Location` pointing to declaration

- [x] **Supported Targets**
  - [x] Entity references in `of "EntityName"`
  - [x] Entity references in `from "EntityName"` / `to "EntityName"`
  - [x] Resource references in `Flow "ResourceName"`
  - [x] Instance references

### 4.4 Find References Provider

- [x] **Capability Declaration**

  - [x] Add `referencesProvider: true`

- [x] **Implementation**
  - [x] Implement `textDocument/references` handler
  - [x] Find symbol at cursor position
  - [x] Scan semantic index for all references to that symbol (Graph lacks stable source locations)
  - [x] Return `Vec<Location>` with all reference sites

### 4.5 Unit Tests for Phase 4

- [x] **Test Completion**

  - [x] Test entity names appear after `of "`
  - [x] Test resource names appear in flow context
  - [x] Test no duplicates in completion list

- [ ] **Test Hover - Golden Snapshots**

  - [x] Test Entity hover shows expected metadata (name, namespace, annotations)
  - [x] Test Resource hover shows name, unit, namespace
  - [x] Test Flow hover shows resource, source, target
  - [x] Test Policy hover shows name, modality, kind, expression signature
  - [x] Test hovering whitespace returns nothing

  - [ ] Test Rule reference resolution (blocked: SEA DSL has no Rule symbols today)
  - [ ] Test ambiguous reference handling (blocked: SEA Graph forbids duplicate IDs; ambiguity requires module resolver)
  - [ ] Test normalized form display (blocked: requires canonical normalizer/pretty-printer integration in hover model)
  - [ ] Test diagnostics with constraint failure + fixes (blocked: requires policy evaluation + code action integration)
  - [ ] Test deprecated symbol with since metadata (blocked: no deprecation metadata in core primitives today)
  - [x] Test truncation markers when limits exceeded

- [x] **Test Hover - Determinism**

  - [x] Test identical output for same (uri, version, position)
  - [x] Test heading order is stable across runs
  - [x] Test no duplicate signature in output
  - [x] Test lists are sorted deterministically

- [ ] **Test Hover - Performance**

  - [ ] Test hover response time < 250ms (p95, warm cache) (blocked: perf tests are nondeterministic in unit tests)
  - [ ] Test hover response time < 500ms (p95, cold cache) (blocked: perf tests are nondeterministic in unit tests)
  - [x] Test payload never exceeds 32KB for markdown
  - [x] Test payload never exceeds 128KB for JSON
  - [ ] Test cache hit rate > 80% for repeated hovers (blocked: requires harness/telemetry across many requests)

- [x] **Test HoverPlus Endpoint**

  - [x] Test `textDocument/hoverPlus` returns valid JSON
  - [x] Test detail level parameter is respected
  - [x] Test `include_markdown` parameter works
  - [x] Test response includes all required HoverModel fields

- [x] **Test Go to Definition**

  - [x] Test navigation from `Instance x of "Entity"` to Entity declaration
  - [x] Test navigation from Flow endpoint to Entity

- [x] **Test Find References**
  - [x] Test finding all uses of an Entity

---

## Phase 5: Code Actions & Quick Fixes

> **Goal**: Provide automated fixes for common errors.

### 5.1 Quick Fix: Undefined Reference

- [x] **Detection**

  - [x] Identify E001 (UndefinedEntity) and E002 (UndefinedResource) diagnostics

- [x] **Fix Generation**
  - [x] Offer "Create Entity 'X'" action
  - [x] Offer "Create Resource 'X'" action
  - [x] Generate declaration stub at appropriate location

### 5.2 Quick Fix: Add Missing Import

> **Note**: Implemented via heuristic. Full logical support deferred until sea-core adds dedicated E500 error.

- [x] **Detection**

  - [x] Identify E500 (NamespaceNotFound) diagnostics (via heuristic E000 check)

- [x] **Fix Generation**
  - [x] Offer "Add import for 'namespace'" action
  - [x] Insert `use namespace;` at file top

### 5.3 Refactoring: Extract to Pattern

> **Note**: Deferred to focus on core stability.

- [ ] **Trigger**

  - [ ] User selects text matching a regex-like expression

- [ ] **Action**
  - [ ] Offer "Extract to Pattern" action
  - [ ] Generate `Pattern "Name" matches "..."` declaration
  - [ ] Replace inline regex with pattern reference

### 5.4 Unit Tests for Phase 5

- [x] **Test Undefined Reference Fix**
- [x] Verify applying action creates Entity stub
- [x] Verify applying action adds import
- [x] **Test Missing Import Fix**
- [x] Verify code action appears for E500 (heuristically)

---

## Phase 6: WASM Web Extension Support

> **Goal**: Enable `sea-core` to run in browser for vscode.dev support.

### 6.1 WASM Build Target

- [x] **sea-core WASM Verification**

  - [x] Verify `sea-core` builds with `wasm32-unknown-unknown` target
  - [x] Verify formatter feature works in WASM
  - [x] Verify parser works in WASM

- [x] **LSP WASM Considerations**
  - [x] Research `tower-lsp` WASM compatibility
  - [x] Determine if full server WASM or TypeScript wrapper around WASM core
  - [x] **Decision**: TypeScript LSP server wrapper using `vscode-languageserver/browser` with sea-core WASM for parsing/formatting

### 6.2 Web Extension Manifest

- [x] **package.json Updates**

  - [x] Add `browser` entry point (`./dist/web/extensionWeb.js`)
  - [x] Add `workspace` extension kind
  - [x] Bundle WASM binary via esbuild copy plugin

- [x] **Fallback Strategy**
  - [x] Detect if running in browser vs desktop via entry points (`main` vs `browser`)
  - [x] Use WASM in browser, native binary in desktop

### 6.3 Browser LSP Server Implementation

- [x] Create `src/web/browserServer.ts` using `vscode-languageserver/browser`
- [x] Implement `BrowserMessageReader`/`BrowserMessageWriter` for web worker communication
- [x] Integrate sea-core WASM `Graph.parse()` for diagnostics
- [x] Integrate sea-core WASM `formatSource()` for formatting

### 6.4 Browser Extension Client

- [x] Create `src/web/extensionWeb.ts` browser entry point
- [x] Implement Web Worker-based language client
- [x] Add restart command support

### 6.5 Build Infrastructure

- [x] Update `esbuild.js` for dual desktop/web builds
- [x] Add WASM copy plugin to bundle `sea_core.js` and `sea_core_bg.wasm`
- [x] Create separate `tsconfig.web.json` to avoid DOM/WebWorker type conflicts
- [x] Add `compile-web` and `test-web` npm scripts

### 6.6 Testing & CI

- [x] Build compiles successfully with all files generated
- [x] Add web bundle size check to CI workflow
- [x] Add web extension file validation to CI workflow
- [ ] Test in vscode.dev (manual, requires Playwright: `pnpm exec playwright install`)

### 6.7 Documentation

- [x] Update `README.md` with web support section and feature comparison table
- [x] Document extension settings
- [x] Add development instructions

---

## Phase 8: MCP Server Integration

> **Goal**: Enable AI agents (e.g., VS Code Copilot, Claude) to query the DomainForge LSP via the Model Context Protocol (MCP). The MCP server acts as a safe, controlled bridge between AI agents and the language server.

### 8.1 Architecture Overview

- [x] **Component Topology**

  - [x] Document the data flow: `VS Code Extension (Node/TS) → MCP Server (Rust) → Rust LSP (stdio)`
  - [x] Define MCP server as a separate binary target in `Cargo.toml`: `[[bin]] name = "domainforge-mcp"`
  - [x] Design the MCP server to spawn/connect to the LSP server per workspace
  - [x] Treat the LSP as the single source of truth for all language features

- [x] **Crate Structure**
  - [x] Create `src/mcp/mod.rs` for MCP server implementation
  - [x] Create `src/mcp/tools.rs` for exposed tool definitions
  - [x] Create `src/mcp/guardrails.rs` for security and rate limiting
  - [x] Create `src/mcp/transport.rs` for MCP protocol handling

### 8.2 MCP Tool Exposure

> **Principle**: Expose read-only, safe operations. No auto-apply mutations without human confirmation.

- [x] **Diagnostics Tool**

  - [x] Implement `domainforge/diagnostics` tool
  - [x] Accept: `uri: string` (file path)
  - [x] Return: Array of diagnostics with severity, message, range, code
  - [x] Rate limit: Max 10 requests/second per workspace

- [x] **Hover Tool**

  - [x] Implement `domainforge/hover` tool
  - [x] Accept: `uri: string`, `line: number`, `character: number`
  - [x] Reuse `HoverModel` builder from Phase 4.2
  - [x] Return: Hover content (markdown) or null
  - [x] Optionally return full `HoverModel` JSON for agent consumption
  - [x] Rate limit: Max 20 requests/second per workspace

- [x] **Definition Tool**

  - [x] Implement `domainforge/definition` tool
  - [x] Accept: `uri: string`, `line: number`, `character: number`
  - [x] Return: Location(s) of definition or empty array
  - [x] Rate limit: Max 10 requests/second per workspace

- [x] **References Tool**

  - [x] Implement `domainforge/references` tool
  - [x] Accept: `uri: string`, `line: number`, `character: number`, `includeDeclaration: boolean`
  - [x] Return: Array of locations
  - [x] Rate limit: Max 5 requests/second per workspace

- [x] **Rename Preview Tool**

  - [x] Implement `domainforge/rename-preview` tool
  - [x] Accept: `uri: string`, `line: number`, `character: number`, `newName: string`
  - [x] Return: `WorkspaceEdit` preview (NOT applied automatically)
  - [x] Flag response with `requiresHumanApproval: true`
  - [x] Rate limit: Max 2 requests/second per workspace

- [x] **Code Actions Tool (Read-Only)**
  - [x] Implement `domainforge/code-actions` tool
  - [x] Accept: `uri: string`, `range: Range`, `context: CodeActionContext`
  - [x] Return: Array of available code actions with titles and kinds
  - [x] Do NOT auto-apply; return `WorkspaceEdit` preview for each action
  - [x] Flag response with `requiresHumanApproval: true`
  - [x] Rate limit: Max 5 requests/second per workspace

### 8.3 Guardrails & Security

- [x] **Path Allowlists**

  - [x] Accept workspace root(s) at MCP server initialization
  - [x] Validate all `uri` parameters against allowlist before processing
  - [x] Reject requests for files outside allowed paths with clear error
  - [x] Support glob patterns for fine-grained access control

- [x] **Repo-Scoped Access**

  - [x] Bind MCP server instance to specific workspace/repository
  - [x] Prevent cross-workspace information leakage
  - [x] Include workspace identifier in all tool responses

- [x] **Rate Limiting**

  - [x] Implement token bucket rate limiter per tool type
  - [x] Configure limits via environment variables or config file
  - [x] Return `429 Too Many Requests` equivalent for exceeded limits
  - [x] Log rate limit violations for monitoring

- [x] **Human-in-the-Loop Apply**

  - [x] Never auto-apply `WorkspaceEdit` mutations
  - [x] Return edits as preview data with `requiresHumanApproval: true`
  - [x] Document the expected client-side confirmation flow
  - [x] Optionally implement `domainforge/apply-edit` tool gated by explicit user confirmation token

- [x] **Audit Logging**
  - [x] Log all MCP tool invocations with timestamp, tool name, parameters (sanitized)
  - [x] Log all denied requests with reason
  - [x] Support configurable log destinations (stderr, file, external service)

### 8.4 MCP Server Entry Point

- [x] **Binary Configuration**

  - [x] Add `src/mcp/main.rs` as entry point for `domainforge-mcp` binary
  - [x] Accept `--workspace-root` argument for path allowlist initialization
  - [x] Accept `--lsp-path` argument to locate the LSP server binary
  - [x] Support `--config` for JSON/TOML configuration file

- [x] **LSP Server Management**

  - [x] Spawn LSP server as child process with stdio transport
  - [x] Implement reconnection logic on LSP server crash
  - [x] Forward initialization params from MCP config to LSP

- [x] **MCP Protocol Transport**
  - [x] Implement MCP protocol over stdio (for CLI/agent integration)
  - [x] Support HTTP SSE transport for web-based agents
  - [x] Handle MCP `initialize`, `tools/list`, `tools/call` methods

### 8.5 VS Code Extension Integration

- [x] **MCP Server Lifecycle**

  - [x] Spawn `domainforge-mcp` alongside or instead of direct LSP spawn
  - [x] Pass workspace folder paths as allowlist
  - [x] Handle MCP server process lifecycle (start, stop, restart)

- [x] **Configuration Options**
  - [x] Add `domainforge.mcp.enable: boolean` setting (default: false)
  - [x] Add `domainforge.mcp.rateLimits` object for per-tool limits
  - [x] Add `domainforge.mcp.auditLog.path` for log file location

### 8.6 Unit Tests for Phase 8

- [x] **Test Tool Responses**

  - [x] Test `domainforge/diagnostics` returns correct format
  - [x] Test `domainforge/hover` returns markdown content
  - [x] Test `domainforge/definition` returns valid locations
  - [x] Test `domainforge/references` includes declaration when requested
  - [x] Test `domainforge/rename-preview` returns non-applied edit
  - [x] Test `domainforge/code-actions` returns edits with approval flag

- [x] **Test Guardrails**

  - [x] Test path allowlist rejects out-of-workspace files
  - [x] Test rate limiter correctly throttles requests
  - [x] Test denied requests are logged

- [x] **Test LSP Integration**
  - [x] Test MCP server correctly forwards requests to LSP
  - [x] Test MCP server handles LSP server restart gracefully
  - [x] Test workspace scoping prevents cross-workspace access

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
