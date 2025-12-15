# DomainForge LSP & VS Code Extension Specification

```yaml
version: "1.0.0"
last_updated: "2025-12-15"
audience: ai_agent
format: yaml_structured_markdown
```

---

## 1. Overview

```yaml
project: DomainForge IDE Tooling
description: |
  Language Server Protocol implementation and VS Code extension for SEA DSL
  (Semantic Enterprise Architecture Domain-Specific Language).

core_principle: |
  Zero logic duplication. The LSP delegates ALL parsing, validation, and 
  formatting to sea-core. This ensures 100% consistency with CLI and prevents
  parser drift.

repositories:
  language_server:
    name: domainforge-lsp
    language: Rust
    role: LSP server binary - receives editor events, delegates to sea-core
  language_client:
    name: domainforge-vsc-extension
    language: TypeScript
    role: VS Code extension - spawns server, manages lifecycle, routes messages
  core_library:
    name: sea-core
    path: domainforge/sea-core
    role: Canonical implementation - grammar, parser, formatter, diagnostics
```

---

## 2. Architecture

```yaml
pattern: client_server_lsp
transport: stdio

components:
  client:
    repository: domainforge-vsc-extension
    entry: src/extension.ts
    manifest: package.json
    bundler: esbuild
    dependencies:
      - vscode-languageclient
    responsibilities:
      - spawn_server_binary
      - detect_platform_and_select_binary
      - register_language_id: "domainforge"
      - associate_file_extension: ".sea"
      - sync_configuration_to_server
      - handle_activation_events

  server:
    repository: domainforge-lsp
    entry: src/main.rs
    manifest: Cargo.toml
    dependencies:
      - sea-core: { path: "../domainforge/sea-core" }
      - tower-lsp: { version: "latest" }
      - tokio: { version: "latest", features: ["full"] }
    responsibilities:
      - implement_lsp_trait
      - handle_jsonrpc_messages
      - delegate_to_sea_core
      - return_lsp_compliant_responses

  core:
    path: domainforge/sea-core
    grammar: grammar/sea.pest
    parser: src/parser/mod.rs
    formatter: src/formatter/mod.rs
    diagnostics: src/error/diagnostics.rs
    validation: src/validation_error.rs
```

---

## 3. sea-core Integration Points

```yaml
modules_to_consume:
  parsing:
    module: sea_core::parser
    functions:
      - parse: "(source: &str) -> ParseResult<Ast>"
      - parse_to_graph: "(source: &str) -> ParseResult<Graph>"
      - parse_to_graph_with_options: "(source: &str, options: &ParseOptions) -> ParseResult<Graph>"
    types:
      - Ast
      - AstNode
      - ParseError
      - ParseOptions

  formatting:
    module: sea_core::formatter
    feature_flag: formatting
    functions:
      - format: "(source: &str, config: FormatConfig) -> Result<String, FormatError>"
      - format_preserving_comments: "(source: &str, config: FormatConfig) -> Result<String, FormatError>"
    types:
      - FormatConfig
      - IndentStyle
      - FormatError

  diagnostics:
    module: sea_core::error::diagnostics
    types:
      - JsonDiagnostic: { purpose: "machine readable output" }
      - LspFormatter: { purpose: "LSP-compatible diagnostic output" }
      - HumanFormatter: { purpose: "CLI colored output" }
    trait: DiagnosticFormatter
    method: "format(&self, error: &ValidationError, source: Option<&str>) -> String"

  validation:
    module: sea_core::validation_error
    types:
      - ValidationError:
          {
            variants:
              [
                SyntaxError,
                TypeError,
                UndefinedReference,
                DuplicateDefinition,
                InvalidExpression,
              ],
          }
      - ErrorCode: { prefix: "E", range: "001-507" }
      - Position: { fields: [line, column], indexing: "1-based" }
      - SourceRange: { fields: [start, end], types: Position }

  graph:
    module: sea_core::graph
    types:
      - Graph: { purpose: "semantic model container" }
    methods:
      - entity_count
      - resource_count
      - flow_count
      - policy_count
      - get_entity
      - get_resource
```

---

## 4. LSP Capabilities

```yaml
implemented:
  text_document_sync:
    open: true
    change: incremental
    close: true
    save: true

  diagnostics:
    method: textDocument/publishDiagnostics
    source: sea-core ValidationError
    mapping:
      error_code: diagnostic.code
      message: diagnostic.message
      range: SourceRange -> lsp::Range
      severity: ErrorCode -> DiagnosticSeverity

  formatting:
    method: textDocument/formatting
    delegate: sea_core::formatter::format
    config_source: workspace_settings

planned:
  completion:
    method: textDocument/completion
    triggers: ['"', "@", "."]
    sources:
      - entity_names: Graph.all_entities()
      - resource_names: Graph.all_resources()
      - namespace_prefixes: parsed imports

  hover:
    method: textDocument/hover
    content:
      - type_signature
      - namespace
      - docstring_from_annotations

  go_to_definition:
    method: textDocument/definition
    targets:
      - entity_declarations
      - resource_declarations
      - instance_of_references
      - flow_endpoints

  find_references:
    method: textDocument/references
    index: semantic_graph_lookup

  code_actions:
    method: textDocument/codeAction
    actions:
      - quick_fix_undefined_reference
      - add_missing_import
      - extract_to_pattern
```

---

## 5. SEA DSL Primitives

```yaml
source: sea-core/grammar/sea.pest

primitives:
  Entity:
    syntax: 'Entity "Name" [v<version>] [annotations] [in <namespace>]'
    examples:
      - 'Entity "Warehouse" in logistics'
      - 'Entity "Customer" v1.0.0 @replaces "Client"'

  Resource:
    syntax: 'Resource "Name" [<unit>] [in <namespace>]'
    examples:
      - 'Resource "Camera Units" units in inventory'
      - 'Resource "Money" USD'

  Flow:
    syntax: 'Flow "<Resource>" from "<Entity>" to "<Entity>" [quantity <n>]'
    examples:
      - 'Flow "Camera Units" from "Warehouse" to "Factory" quantity 100'

  Instance:
    syntax: 'Instance <id> of "<Entity>" { <field>: <value>, ... }'
    examples:
      - 'Instance vendor_123 of "Vendor" { name: "Acme Corp", credit_limit: 50_000 "USD" }'

  Policy:
    syntax: "Policy <name> [per <kind> <modality> priority <n>] [annotations] [v<version>] as: <expression>"
    kinds: [Constraint, Derivation, Obligation]
    modalities: [Obligation, Prohibition, Permission]
    examples:
      - "Policy check_quantity as: Flow.quantity > 0"
      - 'Policy flow_constraints per Constraint Obligation priority 1 as: (Flow.quantity > 0) and (Entity.name != "")'

  Pattern:
    syntax: 'Pattern "<Name>" matches "<regex>"'
    examples:
      - 'Pattern "Email" matches "^[a-z]+@[a-z]+\\.[a-z]+$"'

  Role:
    syntax: 'Role "<Name>" [in <namespace>]'
    examples:
      - 'Role "Approver" in governance'

  Relation:
    syntax: |
      Relation "<Name>"
        subject: "<Entity>"
        predicate: "<verb>"
        object: "<Entity>"
        [via: flow "<Resource>"]
    examples:
      - |
        Relation "Payment"
          subject: "Payer"
          predicate: "pays"
          object: "Payee"
          via: flow "Money"

  Metric:
    syntax: 'Metric "<Name>" as: <expression> [annotations]'
    annotations: [refresh_interval, unit, threshold, severity, target, window]
    examples:
      - 'Metric "total_flow" as: sum(flows.quantity) @unit "USD"'

  Dimension:
    syntax: 'Dimension "<Name>"'
    examples:
      - 'Dimension "Currency"'

  Unit:
    syntax: 'Unit "<Name>" of "<Dimension>" factor <n> base "<BaseUnit>"'
    examples:
      - 'Unit "USD" of "Currency" factor 1 base "USD"'

  ConceptChange:
    syntax: 'ConceptChange "<Name>" [annotations]'
    annotations: [from_version, to_version, migration_policy, breaking_change]

  Mapping:
    syntax: 'Mapping "<Name>" for <format> { <rules> }'
    formats: [calm, kg, sbvr, protobuf, proto]

  Projection:
    syntax: 'Projection "<Name>" for <format> { <rules> }'
    formats: [calm, kg, sbvr, protobuf, proto]

expression_features:
  operators:
    logical: [and, or, not]
    comparison:
      [
        ">=",
        "<=",
        "!=",
        "=",
        ">",
        "<",
        contains,
        startswith,
        endswith,
        matches,
        before,
        after,
        during,
        has_role,
      ]
    arithmetic: ["+", "-", "*", "/"]
  quantifiers: [forall, exists, exists_unique]
  aggregations: [count, sum, min, max, avg]
  collections: [flows, entities, resources, instances, relations]
  group_by: "group_by(<var> in <collection> [where <cond>]: <key>) { <aggregate_expr> }"
  window: 'over last <n> "<unit>"'
  cast: '<expr> as "<unit>"'
```

---

## 6. Error Codes Reference

```yaml
ranges:
  E001-E099: syntax_and_parsing
  E100-E199: type_errors
  E200-E299: control_flow
  E300-E399: unit_errors
  E400-E499: semantic_errors
  E500-E507: namespace_and_module

selected_codes:
  E001: UndefinedEntity
  E002: UndefinedResource
  E003: InvalidSyntax
  E004: MissingRequired
  E100: TypeMismatch
  E101: InvalidType
  E200: InvalidFlow
  E201: InvalidQuantity
  E300: UnitMismatch
  E301: UnitNotFound
  E302: DimensionMismatch
  E400: DuplicateId
  E401: InvalidConstraint
  E500: NamespaceNotFound
  E503: ModuleNotFound
  E505: CircularDependency
```

---

## 7. Build & Distribution

```yaml
server_build:
  toolchain: cargo
  command: "cargo build --release -p domainforge-lsp"
  output: target/release/domainforge-lsp
  cross_compile_targets:
    - x86_64-unknown-linux-gnu
    - x86_64-pc-windows-msvc
    - x86_64-apple-darwin
    - aarch64-apple-darwin
  wasm_support:
    enabled: true
    note: sea-core compiles to wasm32, enabling vscode.dev web extension

client_build:
  toolchain: pnpm
  bundler: esbuild
  commands:
    compile: "pnpm run compile"
    package: "pnpm run package"
  output: dist/extension.js

packaging:
  format: vsix
  contents:
    - bundled_typescript_client
    - platform_binaries: [linux, windows, darwin, darwin_arm64]
  file_associations:
    - extension: ".sea"
      language_id: "domainforge"
```

---

## 8. Configuration Schema

```yaml
settings_namespace: domainforge

options:
  domainforge.linter.enable:
    type: boolean
    default: true
    description: Enable/disable real-time diagnostics

  domainforge.format.onSave:
    type: boolean
    default: true
    description: Auto-format on save

  domainforge.format.indentStyle:
    type: enum
    values: [spaces, tabs]
    default: spaces

  domainforge.format.indentWidth:
    type: integer
    default: 2
    minimum: 1
    maximum: 8

  domainforge.trace.server:
    type: enum
    values: [off, messages, verbose]
    default: off
    description: LSP message tracing for debugging
```

---

## 9. Implementation Status

```yaml
status_legend:
  done: "✓"
  in_progress: "◐"
  planned: "○"
  not_started: "—"

domainforge_lsp:
  project_scaffold: done
  cargo_toml_configured: not_started
  sea_core_integration: not_started
  tower_lsp_setup: not_started
  diagnostics_handler: not_started
  formatting_handler: not_started
  completion_provider: planned
  hover_provider: planned
  definition_provider: planned
  references_provider: planned
  ci_cross_compile: planned

domainforge_vsc_extension:
  project_scaffold: done
  package_json: in_progress
  esbuild_config: done
  language_client_setup: not_started
  server_spawn_logic: not_started
  platform_detection: not_started
  configuration_sync: not_started
  file_association: not_started
  activation_events: not_started
  vsix_packaging: planned
  marketplace_publish: planned

sea_core_readiness:
  grammar: done
  parser: done
  formatter: done
  diagnostics: done
  lsp_formatter: done
  validation_errors: done
  source_ranges: done
  wasm_target: done
```

---

## 10. File Reference

```yaml
domainforge_lsp:
  Cargo.toml: manifest
  src/main.rs: entry_point
  docs/spec.md: this_document

domainforge_vsc_extension:
  package.json: manifest
  src/extension.ts: entry_point
  esbuild.js: bundler_config
  tsconfig.json: typescript_config

sea_core:
  grammar/sea.pest: dsl_grammar
  src/lib.rs: library_root
  src/parser/mod.rs: parser_module
  src/parser/ast.rs: ast_types
  src/formatter/mod.rs: formatter_module
  src/formatter/config.rs: format_config
  src/formatter/printer.rs: format_engine
  src/error/diagnostics.rs: diagnostic_formatters
  src/validation_error.rs: error_types
  src/graph/mod.rs: semantic_graph
  Cargo.toml: manifest_with_features
```

---

## 11. Development Workflow

```yaml
prerequisite:
  - rust_toolchain: stable
  - node_version: ">=18"
  - pnpm: installed

local_development:
  1_build_server:
    cwd: domainforge-lsp
    command: "cargo build"
  2_build_client:
    cwd: domainforge-vsc-extension
    command: "pnpm install && pnpm run compile"
  3_launch_extension:
    method: vscode_f5_debug
    config: .vscode/launch.json

testing:
  server_unit_tests:
    command: "cargo test -p domainforge-lsp"
  client_tests:
    command: "pnpm test"
  integration_tests:
    method: vscode_test_runner
    config: .vscode-test.mjs

ci_pipeline:
  steps:
    - lint: "cargo clippy && pnpm lint"
    - test: "cargo test && pnpm test"
    - build_binaries: cross_compile_all_targets
    - package_vsix: "pnpm run package"
    - publish: marketplace_upload
```

---

## 12. Model References

```yaml
architecture_models:
  rust_analyzer:
    relevance: server_design
    patterns:
      - incremental_parsing
      - salsa_caching
      - lsp_capability_negotiation

  typescript_language_features:
    relevance: client_design
    patterns:
      - minimal_activation
      - server_lifecycle_management
      - configuration_forwarding
```
