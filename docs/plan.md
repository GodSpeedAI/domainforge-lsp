# DomainForge LSP — Phased Implementation Plan

> **Purpose**: MECE (Mutually Exclusive, Collectively Exhaustive) implementation plan for the DomainForge LSP and VS Code extension. Each checkbox represents a discrete unit of work for the implementing agent to mark complete.
>
> **Last Updated**: 2025-12-15

---

## Phase 0: Foundation & CI/CD Infrastructure

> **Goal**: Establish the build, test, and release infrastructure before writing feature code. This prevents technical debt by ensuring all code is linted, tested, and buildable from day one.

### 0.1 LSP Server Repository (`domainforge-lsp`)

- [ ] **Cargo.toml Configuration**

  - [ ] Add `sea-core` path dependency: `sea-core = { path = "../domainforge/sea-core" }`
  - [ ] Add `tower-lsp` dependency with latest version
  - [ ] Add `tokio` with `full` features
  - [ ] Add `serde` and `serde_json` for JSON-RPC
  - [ ] Configure binary target: `[[bin]] name = "domainforge-lsp"`
  - [ ] Set edition, version, and metadata fields

- [ ] **Project Structure**

  - [ ] Create `src/main.rs` with minimal entry point stub
  - [ ] Create `src/backend.rs` for `Backend` struct placeholder
  - [ ] Create `src/capabilities.rs` for capability declarations
  - [ ] Create `src/diagnostics.rs` for diagnostic mapping utilities
  - [ ] Create `src/formatting.rs` for format handler

- [ ] **Testing Infrastructure**
  - [ ] Create `tests/` directory
  - [ ] Create `tests/integration.rs` for integration test harness
  - [ ] Add test fixtures directory: `tests/fixtures/` with sample `.sea` files
  - [ ] Verify `cargo test -p domainforge-lsp` runs (even if empty)

### 0.2 VS Code Extension Repository (`domainforge-vsc-extension`)

- [ ] **package.json Completion**

  - [ ] Add `contributes.languages` for `domainforge` language ID
  - [ ] Add `contributes.grammars` for `.sea` file association
  - [ ] Add `activationEvents` for `.sea` files
  - [ ] Add `contributes.configuration` schema matching spec section 8
  - [ ] Add development dependencies: `@types/vscode`, `esbuild`, `typescript`
  - [ ] Add runtime dependency: `vscode-languageclient`

- [ ] **Build Configuration**

  - [ ] Verify/update `esbuild.js` for production bundling
  - [ ] Configure `tsconfig.json` with strict settings
  - [ ] Add `pnpm run compile` script
  - [ ] Add `pnpm run watch` script for development
  - [ ] Add `pnpm run package` script for VSIX creation

- [ ] **Testing Infrastructure**
  - [ ] Create `.vscode-test.mjs` configuration
  - [ ] Create `src/test/` directory structure
  - [ ] Add `pnpm test` script
  - [ ] Verify test runner executes (even if empty)

### 0.3 CI/CD Pipeline

- [ ] **LSP Server CI** (`domainforge-lsp/.github/workflows/ci.yml`)

  - [ ] Lint step: `cargo fmt --check && cargo clippy -- -D warnings`
  - [ ] Test step: `cargo test -p domainforge-lsp`
  - [ ] Build step: `cargo build --release -p domainforge-lsp`
  - [ ] Cache Cargo dependencies for faster builds
  - [ ] Run on push to `main` and all PRs

- [ ] **Extension Client CI** (`domainforge-vsc-extension/.github/workflows/ci.yml`)

  - [ ] Lint step: `pnpm lint`
  - [ ] Build step: `pnpm run compile`
  - [ ] Test step: `pnpm test`
  - [ ] Cache `node_modules`
  - [ ] Run on push to `main` and all PRs

- [ ] **Cross-Compilation Pipeline** (`domainforge-lsp/.github/workflows/release.yml`)

  - [ ] Build matrix for targets:
    - [ ] `x86_64-unknown-linux-gnu`
    - [ ] `x86_64-pc-windows-msvc`
    - [ ] `x86_64-apple-darwin`
    - [ ] `aarch64-apple-darwin`
  - [ ] Upload artifacts per platform
  - [ ] Trigger on version tags (`v*.*.*`)

- [ ] **VSIX Packaging Pipeline** (`domainforge-vsc-extension/.github/workflows/release.yml`)
  - [ ] Download platform binaries from LSP release
  - [ ] Bundle binaries into extension
  - [ ] Run `vsce package`
  - [ ] Upload VSIX artifact
  - [ ] Trigger on version tags

---

## Phase 1: Minimal LSP Server (Text Sync + Diagnostics)

> **Goal**: Deliver a working LSP that opens `.sea` files and shows parse errors. This is the minimum viable product.

### 1.1 Tower-LSP Scaffold

- [ ] **Backend Struct Implementation**

  - [ ] Define `Backend` struct with `RwLock<HashMap<Url, String>>` for document storage
  - [ ] Implement `tower_lsp::LanguageServer` trait for `Backend`
  - [ ] Implement `initialize` with capability declaration
  - [ ] Implement `initialized` with logging
  - [ ] Implement `shutdown`

- [ ] **Text Document Sync**

  - [ ] Implement `textDocument/didOpen` — store full content
  - [ ] Implement `textDocument/didChange` — apply incremental changes
  - [ ] Implement `textDocument/didClose` — remove from storage
  - [ ] Implement `textDocument/didSave` — trigger validation

- [ ] **Server Entry Point**
  - [ ] Configure `tokio` runtime in `main.rs`
  - [ ] Create `tower_lsp::Server` with stdio transport
  - [ ] Wire `Backend` to server
  - [ ] Add startup logging to stderr

### 1.2 Diagnostics Integration

- [ ] **Validation Pipeline**

  - [ ] On open/change/save: call `sea_core::parser::parse_to_graph`
  - [ ] Capture `ParseResult` errors
  - [ ] If additional validation exists, call `sea_core` validation

- [ ] **Diagnostic Mapping**

  - [ ] Create `fn validation_error_to_diagnostic(e: &ValidationError) -> lsp::Diagnostic`
  - [ ] Map `sea_core::validation_error::SourceRange` → `lsp::Range` (subtract 1 for 0-indexing)
  - [ ] Map `ErrorCode` → `diagnostic.code`
  - [ ] Map error message → `diagnostic.message`
  - [ ] Map severity appropriately (Error/Warning/Info/Hint)

- [ ] **Publish Diagnostics**
  - [ ] Call `client.publish_diagnostics(uri, diagnostics, version)` after validation
  - [ ] Clear diagnostics when document closes

### 1.3 Unit Tests for Phase 1

- [ ] **Test Document Storage**

  - [ ] Test `didOpen` stores content correctly
  - [ ] Test `didChange` applies incremental edits
  - [ ] Test `didClose` removes content

- [ ] **Test Diagnostic Mapping**
  - [ ] Test valid `.sea` produces empty diagnostics
  - [ ] Test syntax error produces E003 diagnostic
  - [ ] Test undefined entity produces E001 diagnostic
  - [ ] Test range conversion is correct (1-based → 0-based)

---

## Phase 2: Formatting Handler

> **Goal**: Enable `Format Document` command in VS Code using `sea-core` formatter.

### 2.1 Format Provider

- [ ] **Capability Declaration**

  - [ ] Add `documentFormattingProvider: true` to server capabilities

- [ ] **Handler Implementation**

  - [ ] Implement `textDocument/formatting` request handler
  - [ ] Extract formatting options from request (indent style, width)
  - [ ] Map to `sea_core::formatter::FormatConfig`
  - [ ] Call `sea_core::formatter::format(source, config)`
  - [ ] Return `Vec<TextEdit>` replacing entire document (or compute minimal diff)

- [ ] **Error Handling**
  - [ ] If source has parse errors, return empty edits (don't format broken code)
  - [ ] Log formatting errors to stderr for debugging

### 2.2 Configuration Sync

- [ ] **Server-Side Config**

  - [ ] Define config struct matching spec section 8 options
  - [ ] Implement `workspace/didChangeConfiguration` handler
  - [ ] Store configuration in `Backend`

- [ ] **Client-Side Config Forwarding**
  - [ ] In extension, listen for configuration changes
  - [ ] Send `workspace/didChangeConfiguration` notification to server
  - [ ] Include relevant `domainforge.*` settings

### 2.3 Unit Tests for Phase 2

- [ ] **Test Formatting**
  - [ ] Test well-formed `.sea` file returns formatted output
  - [ ] Test malformed `.sea` file returns empty edits
  - [ ] Test indent style option is respected
  - [ ] Test indent width option is respected

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

- [ ] **Capability Declaration**

  - [ ] Add `hoverProvider: true`

- [ ] **Implementation**
  - [ ] Implement `textDocument/hover` handler
  - [ ] Find semantic node at cursor position in `Graph`
  - [ ] For Entity: show name, namespace, version, annotations
  - [ ] For Resource: show name, unit, namespace
  - [ ] For Flow: show resource, source, target, quantity
  - [ ] Format as Markdown for rich display

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

- [ ] **Test Hover**

  - [ ] Test entity hover shows expected metadata
  - [ ] Test hovering whitespace returns nothing

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

## Phase 7: Release & Distribution

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

| Phase | Verification Method       | Command/Steps                                                |
| ----- | ------------------------- | ------------------------------------------------------------ |
| 0     | CI passes                 | `cargo test`, `pnpm test`, GitHub Actions green              |
| 1     | Unit tests + manual       | `cargo test -p domainforge-lsp`, open `.sea` in VS Code      |
| 2     | Unit tests + manual       | `cargo test`, execute Format Document command                |
| 3     | Integration test + manual | `pnpm test`, open extension in debug host, check diagnostics |
| 4     | Unit tests                | `cargo test` for all providers                               |
| 5     | Unit tests                | `cargo test` for code actions                                |
| 6     | WASM tests + manual       | Test in vscode.dev (if pursued)                              |
| 7     | Manual                    | Install from VSIX, verify all features                       |

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

---

## Notes for Implementing Agent

1. **Order Matters**: Complete Phase 0 first. CI must be green before feature work.
2. **Check Off As You Go**: Mark `[x]` as each item is completed.
3. **Commit Granularly**: One commit per logical unit (e.g., "Add didOpen handler").
4. **Test Before Proceeding**: Don't move to next phase until current phase tests pass.
5. **Ask When Blocked**: If `sea-core` API differs from spec, clarify before proceeding.
