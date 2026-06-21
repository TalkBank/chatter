//! Typed expectations manifest linking each generated validation fixture to the
//! error codes it must produce and its implementation status. This is the only
//! contract between the spec generator and the data-driven runner; it is
//! serialized to the corpus dir as `manifest.json`.

use serde::{Deserialize, Serialize};

use super::error_corpus::{SpecErrorCode, Status};

/// A generated fixture's filename within the `validation_errors` corpus dir.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FixtureName(String);

impl FixtureName {
    /// Wrap a fixture filename.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
    /// The filename text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One generated fixture and what the runner must assert about it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationFixtureEntry {
    /// Fixture filename, relative to the validation_errors corpus dir.
    pub fixture: FixtureName,
    /// All error codes the fixture must produce (parse + validation).
    pub expected_codes: Vec<SpecErrorCode>,
    /// Implementation status carried from the source spec; the runner skips
    /// anything that is not `Implemented`.
    pub status: Status,
    /// Source spec path, for diagnostics.
    pub source_spec: String,
}

/// Top-level manifest written to the corpus dir as `manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ValidationManifest {
    pub fixtures: Vec<ValidationFixtureEntry>,
    /// Implemented validation specs that produced no example/fixture. Populated
    /// by the generator; consumed by the runner's coverage gate.
    #[serde(default)]
    pub implemented_specs_without_examples: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_json() {
        let m = ValidationManifest {
            fixtures: vec![ValidationFixtureEntry {
                fixture: FixtureName::new("E370_retrace.cha"),
                expected_codes: vec![SpecErrorCode::parse("E370").expect("valid code")],
                status: Status::Implemented,
                source_spec: "spec/errors/E370_retrace_missing_content.md".to_string(),
            }],
            implemented_specs_without_examples: Vec::new(),
        };
        let json = serde_json::to_string_pretty(&m).expect("serialize");
        // Codes and status serialize transparently as JSON strings.
        assert!(json.contains("\"E370\""));
        assert!(json.contains("\"implemented\""));
        let back: ValidationManifest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(m, back);
    }
}
