use std::path::Path;

use sea_core::semantic_pack::{
    compute_pack_content_hash, merge_packs, SemanticDiagnostic, SemanticDiagnosticCode,
    SemanticPack, PackSet, ValidationOptions,
};
use sea_core::semantic_pack::schema::SourceRef;

use crate::semantic_config::SemanticPackConfig;

pub fn load_pack_from_path(path: &str) -> Result<SemanticPack, SemanticDiagnostic> {
    let file_path = Path::new(path);

    let content = std::fs::read_to_string(file_path).map_err(|e| SemanticDiagnostic {
        code: SemanticDiagnosticCode::PackUnavailable,
        severity: sea_core::semantic_pack::DiagnosticSeverity::Error,
        semantic_truth: sea_core::semantic_pack::SemanticTruth::Invalid,
        message: format!("Failed to read pack file '{}': {}", path, e),
        source_ref: SourceRef::synthetic(path),
        pack_ref: sea_core::semantic_pack::schema::PackRef {
            pack_id: String::new(),
            pack_content_hash: String::new(),
            path_or_uri: path.to_string(),
            priority: 0,
        },
        suggestions: vec![],
        recoverability_hint: "Check that the pack file path is correct and accessible".to_string(),
    })?;

    serde_json::from_str::<SemanticPack>(&content).map_err(|e| SemanticDiagnostic {
        code: SemanticDiagnosticCode::PackSchemaMismatch,
        severity: sea_core::semantic_pack::DiagnosticSeverity::Error,
        semantic_truth: sea_core::semantic_pack::SemanticTruth::Invalid,
        message: format!("Failed to parse pack JSON '{}': {}", path, e),
        source_ref: SourceRef::synthetic(path),
        pack_ref: sea_core::semantic_pack::schema::PackRef {
            pack_id: String::new(),
            pack_content_hash: String::new(),
            path_or_uri: path.to_string(),
            priority: 0,
        },
        suggestions: vec![],
        recoverability_hint: "Verify the pack file is valid JSON conforming to the semantic pack schema".to_string(),
    })
}

pub fn load_pack_set(
    configs: &[SemanticPackConfig],
    options: &ValidationOptions,
) -> Result<PackSet, Vec<SemanticDiagnostic>> {
    if configs.is_empty() {
        return merge_packs(&[], &[]).map_err(|_| vec![]);
    }

    let mut packs: Vec<SemanticPack> = Vec::with_capacity(configs.len());
    let mut priorities: Vec<i32> = Vec::with_capacity(configs.len());
    let mut errors: Vec<SemanticDiagnostic> = Vec::new();

    for config in configs {
        match load_pack_from_path(&config.path) {
            Ok(pack) => {
                if let Some(ref expected_hash) = config.expected_hash {
                    if let Err(diag) = verify_expected_hash(&pack, expected_hash) {
                        if options.mode == sea_core::semantic_pack::ValidationMode::Strict {
                            errors.push(diag);
                            continue;
                        } else {
                            log::warn!(
                                "Pack hash mismatch for '{}': expected {}",
                                pack.pack_id,
                                expected_hash
                            );
                        }
                    }
                }

                if options.require_signed_pack
                    && pack.trust.signature_state
                        != sea_core::semantic_pack::schema::SignatureState::Signed
                {
                    let diag = SemanticDiagnostic {
                        code: SemanticDiagnosticCode::PackUnsigned,
                        severity: sea_core::semantic_pack::DiagnosticSeverity::Error,
                        semantic_truth: sea_core::semantic_pack::SemanticTruth::Invalid,
                        message: format!(
                            "Pack '{}' is unsigned but signatures are required",
                            pack.pack_id
                        ),
                        source_ref: SourceRef::synthetic(&config.path),
                        pack_ref: sea_core::semantic_pack::schema::PackRef {
                            pack_id: pack.pack_id.clone(),
                            pack_content_hash: compute_pack_content_hash(&pack),
                            path_or_uri: config.path.clone(),
                            priority: config.priority,
                        },
                        suggestions: vec![],
                        recoverability_hint: "Sign the pack or disable require_signature".to_string(),
                    };
                    errors.push(diag);
                    continue;
                }

                if pack.schema_version != "0.3" {
                    let diag = SemanticDiagnostic {
                        code: SemanticDiagnosticCode::PackVersionMismatch,
                        severity: sea_core::semantic_pack::DiagnosticSeverity::Warning,
                        semantic_truth: sea_core::semantic_pack::SemanticTruth::Unknown,
                        message: format!(
                            "Pack '{}' has schema version '{}' (expected '0.3')",
                            pack.pack_id, pack.schema_version
                        ),
                        source_ref: SourceRef::synthetic(&config.path),
                        pack_ref: sea_core::semantic_pack::schema::PackRef {
                            pack_id: pack.pack_id.clone(),
                            pack_content_hash: compute_pack_content_hash(&pack),
                            path_or_uri: config.path.clone(),
                            priority: config.priority,
                        },
                        suggestions: vec![],
                        recoverability_hint: "Update the pack to schema version 0.3".to_string(),
                    };
                    if options.mode == sea_core::semantic_pack::ValidationMode::Strict {
                        errors.push(diag);
                        continue;
                    } else {
                        log::warn!("{}", diag.message);
                    }
                }

                packs.push(pack);
                priorities.push(config.priority);
            }
            Err(diag) => {
                errors.push(diag);
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    merge_packs(&packs, &priorities).map_err(|conflicts| {
        conflicts
            .into_iter()
            .map(|c| SemanticDiagnostic {
                code: SemanticDiagnosticCode::PackSetConflict,
                severity: sea_core::semantic_pack::DiagnosticSeverity::Error,
                semantic_truth: sea_core::semantic_pack::SemanticTruth::Invalid,
                message: format!(
                    "Pack set conflict ({}): {}",
                    c.detail,
                    c.key
                ),
                source_ref: SourceRef::workspace_root(),
                pack_ref: sea_core::semantic_pack::schema::PackRef {
                    pack_id: c.pack_a_id.clone(),
                    pack_content_hash: String::new(),
                    path_or_uri: String::new(),
                    priority: 0,
                },
                suggestions: vec![],
                recoverability_hint: "Resolve the conflicting pack definitions or adjust priorities".to_string(),
            })
            .collect()
    })
}

pub fn verify_expected_hash(
    pack: &SemanticPack,
    expected: &str,
) -> Result<(), SemanticDiagnostic> {
    let actual = compute_pack_content_hash(pack);
    if actual == expected {
        Ok(())
    } else {
        Err(SemanticDiagnostic {
            code: SemanticDiagnosticCode::PackHashMismatch,
            severity: sea_core::semantic_pack::DiagnosticSeverity::Error,
            semantic_truth: sea_core::semantic_pack::SemanticTruth::Invalid,
            message: format!(
                "Pack '{}' hash mismatch: expected '{}', got '{}'",
                pack.pack_id, expected, actual
            ),
            source_ref: SourceRef::pack_uri(&pack.pack_id),
            pack_ref: sea_core::semantic_pack::schema::PackRef {
                pack_id: pack.pack_id.clone(),
                pack_content_hash: actual.clone(),
                path_or_uri: format!("pack://{}", pack.pack_id),
                priority: 0,
            },
            suggestions: vec![],
            recoverability_hint: "Update the expected_hash in configuration or regenerate the pack".to_string(),
        })
    }
}
