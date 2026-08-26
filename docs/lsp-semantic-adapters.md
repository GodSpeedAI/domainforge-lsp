# LSP Semantic Adapters

The DomainForge LSP server can load **semantic packs** that provide domain-specific vocabulary, validation rules, and enrichment metadata for `.sea` files. When a semantic pack is active, the LSP publishes semantic diagnostics alongside standard parse diagnostics, provides pack-driven completion suggestions, and enriches hover responses with pack metadata.

All semantic validation logic lives in `sea-core::semantic_pack`. The LSP acts as a protocol adapter: it loads packs from configuration, translates `sea-core` diagnostic types into LSP `Diagnostic` objects, and feeds pack concepts into completion and hover providers.

---

## 1. Overview

The semantic adapter pipeline works as follows:

1. The LSP reads semantic pack paths from the workspace configuration (`domainforge.semantic.packs`).
2. On `initialize` and every `workspace/didChangeConfiguration`, the server loads, validates, and merges those packs into a `PackSet`.
3. On each document validation cycle, the server first runs the standard parse. If the parse succeeds and a pack is available, the server calls `validate_graph_with_pack` to produce semantic diagnostics.
4. Semantic diagnostics are converted to LSP `Diagnostic` objects and published together with any parse diagnostics.
5. Pack concepts feed into completion and hover providers, offering domain-aware suggestions and enriched metadata.

The key design invariant is **zero logic duplication**: the LSP never interprets pack content itself. It delegates to `sea-core::semantic_pack` for loading, merging, hashing, signature verification, and graph validation.

---

## 2. Configuration

Semantic packs are configured under the `semantic` section of `DomainForgeConfig`. This section is synced from the VS Code extension via `workspace/didChangeConfiguration`.

```json
{
  "domainforge": {
    "semantic": {
      "enabled": true,
      "validationMode": "warn",
      "unknownConceptPolicy": "warning",
      "deprecatedPolicy": "warn",
      "requireSignature": false,
      "packs": [
        {
          "path": ".domainforge/packs/acme.procurement.semantic-pack.json",
          "priority": 10,
          "expectedHash": "sha256:a1b2c3d4..."
        }
      ]
    }
  }
}
```

### Field Reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | `bool` | `true` | Master switch for the entire semantic subsystem. When `false`, no packs are loaded and no semantic diagnostics, completions, or hover enrichment are produced. |
| `validationMode` | `"warn"` \| `"strict"` | `"warn"` | Governs how pack-load failures are treated. In `"warn"` mode, hash mismatches and version mismatches are logged but the pack is still used. In `"strict"` mode, these conditions reject the pack entirely. |
| `unknownConceptPolicy` | `"warning"` \| `"error"` \| `"ignore"` | `"warning"` | What diagnostic severity to assign when a `.sea` file references a concept not found in any loaded pack. |
| `deprecatedPolicy` | `"warn"` \| `"error"` \| `"ignore"` | `"warn"` | What diagnostic severity to assign when a `.sea` file uses a concept marked `deprecated` in the pack. |
| `requireSignature` | `bool` | `false` | When `true`, every pack must carry a valid cryptographic signature. Unsigned packs are rejected with a `PackUnsigned` diagnostic. |
| `packs` | `SemanticPackConfig[]` | `[]` | Array of pack descriptors to load. The array may be empty, in which case no semantic validation occurs. |

### SemanticPackConfig

Each entry in `packs` describes one semantic pack file:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `path` | `string` | Yes | Filesystem path to the semantic pack JSON file. May be relative to the workspace root. |
| `priority` | `i32` | No | Merge priority. Higher values take precedence during concept conflict resolution. Defaults to `0`. |
| `expectedHash` | `string` | No | Content hash pin in the format `sha256:<hex>`. When present, the pack's computed hash must match exactly. This is a tamper-protection pin: it ensures the pack file has not been modified since the hash was recorded. If the hash does not match, the behavior depends on `validationMode`. In `"strict"` mode the pack is rejected. In `"warn"` mode a warning is logged and the pack is still used. |

### Tamper-Protection with `expectedHash`

The `expectedHash` field acts as a content integrity pin. When a team commits the configuration with a known-good hash, any unauthorized change to the pack file on disk will be detected at load time. The hash is computed by `sea_core::semantic_pack::compute_pack_content_hash`, which deterministically serialises the pack's canonical fields and produces a `sha256` digest.

---

## 3. Backend State

The `Backend` struct (`src/backend.rs`) holds two fields related to semantic packs:

```rust
pub struct Backend {
    // ... other fields ...

    semantic_pack_set: RwLock<Option<PackSet>>,
    semantic_pack_errors: RwLock<Vec<SemanticDiagnostic>>,
}
```

### `semantic_pack_set`

An `RwLock<Option<PackSet>>` that holds the successfully loaded and merged pack set. The value is `None` when:

- Semantic validation is disabled (`enabled: false`).
- No packs are configured.
- All packs failed to load.

When the value is `Some(PackSet)`, the pack set is available for validation, completion, and hover enrichment.

### `semantic_pack_errors`

An `RwLock<Vec<SemanticDiagnostic>>` that accumulates errors from the most recent load attempt. These errors are converted into file-level LSP diagnostics (range `0:0` to `0:0`) and attached to every open `.sea` document so the user is aware that semantic validation is degraded.

### Loading Sequence

Packs are loaded at two points:

1. **`initialize`** — The server calls `self.load_semantic_packs().await` during the `initialize` handshake, before reporting capabilities.
2. **`didChangeConfiguration`** — After updating the config, the server reloads packs and then re-validates all open documents.

The `load_semantic_packs` method:

1. Reads `config.semantic`. If `enabled` is `false` or `packs` is empty, clears both `semantic_pack_set` and `semantic_pack_errors`.
2. Builds `ValidationOptions` from the config via `config.semantic.to_validation_options()`.
3. Calls `semantic_pack_loader::load_pack_set(&pack_configs, &options)`.
4. On success, stores the `PackSet` and clears errors.
5. On failure, sets `semantic_pack_set` to `None` and stores the error list.

---

## 4. Validation Pipeline

Document validation follows a strict layered order:

```
┌─────────────────────────────────┐
│  1. Parse (sea-core parser)     │
│     If parse fails → stop here  │
└──────────────┬──────────────────┘
               │ parse OK
┌──────────────▼──────────────────┐
│  2. Check semantic enabled?     │
│     If no → publish empty diag  │
└──────────────┬──────────────────┘
               │ yes
┌──────────────▼──────────────────┐
│  3. Check pack_errors non-empty?│
│     If yes → emit pack diags   │
└──────────────┬──────────────────┘
               │ no errors
┌──────────────▼──────────────────┐
│  4. Check PackSet available?    │
│     If no → skip (nothing to   │
│     validate against)           │
└──────────────┬──────────────────┘
               │ pack available
┌──────────────▼──────────────────┐
│  5. validate_graph_with_pack()  │
│     Convert SemanticDiagnostic  │
│     → LSP Diagnostic            │
└─────────────────────────────────┘
```

### Key Invariants

- **Parse-first**: Semantic validation never runs on a document with parse errors. The user must fix syntax before semantic issues are visible.
- **Graceful degradation**: If the pack is unavailable (load failed, disabled, or not configured), the server simply omits semantic diagnostics. No false positives are produced.
- **Pack errors as diagnostics**: When `semantic_pack_errors` is non-empty, each error is converted to a file-level warning diagnostic so the user knows semantic validation is not functioning.

The validation entry point is `Backend::validate_document`, which assembles both parse and semantic diagnostics into a single `publishDiagnostics` notification.

---

## 5. Diagnostics

### Conversion: SemanticDiagnostic → LSP Diagnostic

The function `semantic_diagnostic_to_lsp` (`src/semantic_diagnostics.rs`) converts each `sea_core::semantic_pack::SemanticDiagnostic` into an `lsp_types::Diagnostic`:

| `SemanticDiagnostic` field | LSP `Diagnostic` field |
|---|---|
| `source_ref` (byte range) | `range` (converted via `LineIndex::position_of`) |
| `severity` (Error/Warning/Info/Hint) | `severity` (mapped 1:1 to `DiagnosticSeverity`) |
| `code` (e.g. `unknown_concept`) | `code` prefixed with `S`: `"Sunknown_concept"` |
| `message` | `message` |
| `semantic_truth` | `data` (serialised as JSON string) |

The `source` field is set to `"domainforge-semantic"` to distinguish semantic diagnostics from parse diagnostics (which use `"domainforge"`).

### Diagnostic Data Payload

Each semantic diagnostic includes a `data` field carrying structured metadata:

```json
{
  "domainforge": {
    "semantic_code": "unknown_concept",
    "semantic_truth": "unknown",
    "pack_id": "acme/procurement/0.1.0",
    "pack_hash": "sha256:a1b2c3d4...",
    "merged_pack_hash": "sha256:e5f6g7h8..."
  }
}
```

| Field | Description |
|---|---|
| `semantic_code` | The specific semantic rule that was violated (e.g. `unknown_concept`, `deprecated_concept`, `type_mismatch`). |
| `semantic_truth` | The truth state of the finding: `"unknown"` for uncertain findings, `"valid"` for confirmed correct usage, `"invalid"` for confirmed violations. |
| `pack_id` | The identifier of the pack that produced this diagnostic, in `<org>/<domain>/<version>` format. |
| `pack_hash` | Content hash of the individual pack that generated the diagnostic. |
| `merged_pack_hash` | Content hash of the merged `PackSet`, allowing the client to correlate diagnostics with a specific pack configuration snapshot. |

### Pack-Level Diagnostics

When the pack loading itself fails, `create_pack_diagnostic` generates a file-level diagnostic with range `0:0`–`0:0` and severity `Warning`. These diagnostics inform the user that semantic validation is degraded without pointing at any specific location in the source file.

### Severity Mapping

```rust
fn map_severity(severity: SeaDiagnosticSeverity) -> DiagnosticSeverity {
    match severity {
        SeaDiagnosticSeverity::Error   => DiagnosticSeverity::ERROR,
        SeaDiagnosticSeverity::Warning => DiagnosticSeverity::WARNING,
        SeaDiagnosticSeverity::Info    => DiagnosticSeverity::INFORMATION,
        SeaDiagnosticSeverity::Hint    => DiagnosticSeverity::HINT,
    }
}
```

---

## 6. Completion

When semantic packs are loaded and error-free, the LSP augments its standard completion results with pack-driven suggestions via `src/semantic_completion.rs`.

### How It Works

1. The standard completion provider (`src/completion.rs`) generates results from the parsed graph and semantic index.
2. If `semantic.enabled` is `true` and `semantic_pack_errors` is empty, the backend loads the first pack from the `PackSet`.
3. It extracts the prefix at the cursor position and calls `get_semantic_completions(&pack, &prefix, include_deprecated)`.
4. The returned `CompletionItem` list is appended to the standard results.

### Concept Filtering

Only concepts with status `Active` are included by default. The filtering logic:

| Concept Status | Included | Notes |
|---|---|---|
| `Active` | Yes | Always included in completion results. |
| `Deprecated` | Only if `include_deprecated=true` | Currently hardcoded to `false` in the backend. Deprecated concepts are excluded by default to steer users toward active vocabulary. |
| `Proposed` | No | Proposed concepts are not yet ratified and are excluded. |
| `Rejected` | No | Rejected concepts are never offered. |
| `ExternalOnly` | No | External-only concepts are not applicable in `.sea` source files. |

### Completion Item Details

Each semantic completion item includes:

- **`label`**: The matching alias (if the user typed an alias prefix) or the `canonical_name`.
- **`insertText`**: Always the `canonical_name`, ensuring inserted text is canonical regardless of how the user discovered it.
- **`kind`**: Mapped from the concept's `ConceptKind`:
  - `Entity` → `CompletionItemKind::CLASS`
  - `Resource` → `CompletionItemKind::CONSTANT`
  - `Role` → `CompletionItemKind::ENUM_MEMBER`
  - `Flow` → `CompletionItemKind::EVENT`
  - `Policy` → `CompletionItemKind::ENUM`
  - `Metric` → `CompletionItemKind::FIELD`
  - `Dimension` → `CompletionItemKind::STRUCT`
  - `Unit` → `CompletionItemKind::UNIT`
  - `External` → `CompletionItemKind::INTERFACE`
- **`detail`**: `"<canonical_name> (<status>)"` — e.g. `"Purchase Order (active)"`.
- **`documentation`**: The concept's `definition.text`, if non-empty.
- **`deprecated`**: `true` when the concept status is `Deprecated`, causing the editor to render it with a strikethrough.
- **`filterText`**: Concatenation of `canonical_name`, `id`, and all aliases, enabling fuzzy matching across multiple identifiers.
- **`sortText`**: Status-rank-prefixed name, ensuring active concepts sort above deprecated ones.

### Sorting

Results are sorted first by `sortText` (which embeds status rank: Active=0, Proposed=1, Deprecated=2, External=3, Rejected=4) and then alphabetically by label.

---

## 7. Hover

Pack concepts enrich hover responses with domain metadata. When the hover provider resolves a symbol that matches a concept in the active pack, the hover model includes additional facts drawn from the pack.

### Pack Concept Hover Content

For a symbol that resolves to a pack concept, the hover response shows:

| Field | Source | Description |
|---|---|---|
| `canonical_name` | `Concept.canonical_name` | The authoritative name of the concept. |
| `kind` | `Concept.kind` | The concept type (Entity, Resource, Role, etc.). |
| `status` | `Concept.status` | Lifecycle state: active, proposed, deprecated, rejected, or external. |
| `definition text` | `Concept.definition.text` | Human-readable definition from the pack. Displayed as the hover documentation body. |
| `owner` | `Concept.owner` | The team or individual responsible for the concept. |
| `pack_id` | `Pack.pack_id` | The identifier of the originating pack. |
| `meaning_version` | `Concept.meaning_version` | The version of the concept's semantic definition, enabling version-aware hover. |

### Hover Rendering

The hover model is built by `build_hover_model` in `src/hover/symbol_resolver.rs`. For graph-resolved symbols (Entity, Resource, Flow, etc.), the model includes:

- A **header** with display name, kind label, and qualified path.
- A **signature** showing the DSL syntax for the symbol.
- **Facts** (key-value pairs) covering namespace, version, flow counts, unit symbols, policy metadata, etc.
- **Related symbols** ranked by co-occurrence frequency.

When a semantic pack is active, concept metadata can be overlaid onto these facts, providing the user with both structural (graph-derived) and semantic (pack-derived) context in a single hover card.

---

## 8. Last-Known-Good Pack Cache

When a pack reload fails, the LSP retains the previously valid pack set in `semantic_pack_set` and surfaces the new errors through `semantic_pack_errors`.

### Behavior

| Scenario | `semantic_pack_set` | `semantic_pack_errors` | Result |
|---|---|---|---|
| Initial load succeeds | `Some(PackSet)` | `[]` | Normal operation. |
| Config change, reload succeeds | `Some(new PackSet)` | `[]` | Updated pack active. |
| Config change, reload fails | `None` | `[errors...]` | Previous pack is **lost**; semantic validation is blocked. Pack-error diagnostics are emitted. |
| Packs disabled (`enabled: false`) | `None` | `[]` | Semantic subsystem turned off cleanly. |

### Impact on Validation

When `semantic_pack_errors` is non-empty:

- The server emits file-level warning diagnostics for every open document, informing the user that semantic packs failed to load.
- The `validate_document` method does not call `validate_graph_with_pack`, because the condition `pack_errors.is_empty()` guards the pack-validation branch.
- The `SemanticValidationResult.status` is effectively **blocked** when no valid pack exists — no semantic diagnostics are produced, and the user sees only pack-load errors.

This design ensures that a broken pack never produces false-positive or false-negative semantic findings. The user is always explicitly informed when the semantic subsystem is degraded.

---

## 9. Multi-Pack Support

The LSP supports loading multiple semantic packs simultaneously. Packs are merged into a single `PackSet` by `sea_core::semantic_pack::merge_packs`.

### Merge Process

1. Each `SemanticPackConfig` entry is loaded independently via `load_pack_from_path`.
2. Hash verification, signature checks, and schema version checks are applied per-pack.
3. Valid packs and their priorities are passed to `merge_packs(&packs, &priorities)`.
4. `merge_packs` produces a `PackSet` containing:
   - `packs`: The merged list of `PackRef` entries.
   - `merged_pack_hash`: A deterministic hash of the combined pack content.
   - A unified concept namespace with conflict detection.

### Conflict Detection

When two packs define the same concept (by `id`), the merge detects the conflict and returns an error:

```
Pack set conflict (duplicate concept): concept_id "purchase_order"
```

Each conflict produces a `SemanticDiagnostic` with code `PackSetConflict`, describing the conflicting key and the two pack identifiers involved.

### Precedence Rules

When multiple packs define overlapping concepts and the conflict is resolvable by priority:

- **Higher `priority` wins**: The concept from the pack with the higher `priority` value takes precedence.
- **Equal priority**: If two packs have the same priority and define the same concept, this is treated as a conflict and the merge fails.
- **No priority specified**: Defaults to `0`.

### Practical Multi-Pack Example

```json
{
  "semantic": {
    "enabled": true,
    "packs": [
      {
        "path": ".domainforge/packs/acme.foundation.semantic-pack.json",
        "priority": 5
      },
      {
        "path": ".domainforge/packs/acme.procurement.semantic-pack.json",
        "priority": 10
      }
    ]
  }
}
```

In this configuration, the procurement pack (priority 10) takes precedence over the foundation pack (priority 5) for any overlapping concepts. If both packs define the same concept with the same priority, the merge fails and both packs are rejected with `PackSetConflict` diagnostics.

### Module Reference

| Module | File | Responsibility |
|--------|------|----------------|
| `semantic_config` | `src/semantic_config.rs` | Configuration types, defaults, conversion to `ValidationOptions`. |
| `semantic_pack_loader` | `src/semantic_pack_loader.rs` | Pack loading, hash verification, signature checks, `PackSet` assembly. |
| `semantic_diagnostics` | `src/semantic_diagnostics.rs` | Conversion from `SemanticDiagnostic` to LSP `Diagnostic`. |
| `semantic_completion` | `src/semantic_completion.rs` | Pack-driven completion item generation. |
| `semantic_index` | `src/semantic_index.rs` | AST-level symbol index for navigation, hover, and completion. |
