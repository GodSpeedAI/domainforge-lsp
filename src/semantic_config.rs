use serde::{Deserialize, Serialize};

use sea_core::semantic_pack::diagnostics::{DeprecatedPolicy, UnknownConceptPolicy, ValidationMode};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub packs: Vec<SemanticPackConfig>,
    #[serde(default = "default_validation_mode")]
    pub validation_mode: ValidationMode,
    #[serde(default = "default_unknown_concept_policy")]
    pub unknown_concept_policy: UnknownConceptPolicy,
    #[serde(default = "default_deprecated_policy")]
    pub deprecated_policy: DeprecatedPolicy,
    #[serde(default)]
    pub require_signature: bool,
}

impl Default for SemanticConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            packs: vec![],
            validation_mode: default_validation_mode(),
            unknown_concept_policy: default_unknown_concept_policy(),
            deprecated_policy: default_deprecated_policy(),
            require_signature: false,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_validation_mode() -> ValidationMode {
    ValidationMode::Warn
}

fn default_unknown_concept_policy() -> UnknownConceptPolicy {
    UnknownConceptPolicy::Warning
}

fn default_deprecated_policy() -> DeprecatedPolicy {
    DeprecatedPolicy::Warn
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticPackConfig {
    pub path: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_hash: Option<String>,
}

impl SemanticConfig {
    pub fn to_validation_options(&self) -> sea_core::semantic_pack::ValidationOptions {
        sea_core::semantic_pack::ValidationOptions {
            mode: self.validation_mode,
            unknown_concept_policy: self.unknown_concept_policy,
            deprecated_policy: self.deprecated_policy,
            require_signed_pack: self.require_signature,
            allow_unsigned_test_fixtures: false,
        }
    }
}
