//! Typed expectations manifest linking each generated validation fixture to
//! its spec's code, its CLAIM, and its implementation status. This is the only
//! contract between the spec generator and the data-driven runner; it is
//! serialized to the corpus dir as `manifest.json`.

use serde::{Deserialize, Serialize};

use super::metadata::SpecErrorCode;
use super::metadata::Status;
use crate::repo_paths::RepoRelativePath;

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
    /// The spec's own code, which the claim is ABOUT.
    ///
    /// With `claim` it replaces the pre-R2 `expected_codes` list, which mixed
    /// the normative assertion with incidental observations and could not
    /// express an absence at all.
    pub code: SpecErrorCode,
    /// What this fixture asserts; the runner enforces both halves
    /// (`subsumed_by` and `legal` carry negative assertions).
    pub claim: talkbank_spec_vocabulary::frontmatter::Claim,
    /// Implementation status carried from the source spec; the runner skips
    /// anything that is not `Implemented`.
    pub status: Status,
    /// Source spec path, for diagnostics. Repo-relative by construction, so a
    /// caller cannot record an absolute one.
    pub source_spec: RepoRelativePath,
}

/// Top-level manifest written to the corpus dir as `manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ValidationManifest {
    pub fixtures: Vec<ValidationFixtureEntry>,
    /// Implemented CODES with no example in ANY spec that claims them.
    ///
    /// Per-code, not per-spec, as of R4: a code may be claimed by several spec
    /// files (the duplicate pairs), and the obligation "a rule owes a
    /// triggering example" belongs to the RULE. A no-example spec whose code
    /// is demonstrated by its sibling is documentation, not a gap; the
    /// per-spec version of this list reported exactly that false positive the
    /// moment the corpus became total (`E502_wor_cascade_regression.md`, a
    /// false-positive regression record whose code E502 is demonstrated by
    /// `E502_auto.md`).
    #[serde(default)]
    pub implemented_codes_without_examples: Vec<SpecErrorCode>,

    /// Specs marked `unreachable_from_chat` that nonetheless carry an example.
    ///
    /// An example means some CHAT input reaches the rule, so the status is
    /// wrong. Without this, the new status would be a way to opt any rule out
    /// of its fixture obligation.
    #[serde(default)]
    pub unreachable_specs_with_examples: Vec<RepoRelativePath>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_json() {
        let m = ValidationManifest {
            fixtures: vec![ValidationFixtureEntry {
                fixture: FixtureName::new("E370_retrace.cha"),
                code: SpecErrorCode::parse("E370").expect("valid code"),
                claim: talkbank_spec_vocabulary::frontmatter::Claim::Violates,
                status: Status::Implemented,
                source_spec: RepoRelativePath::new(
                    std::path::Path::new("/checkout"),
                    "/checkout/spec/errors/E370_retrace_missing_content.md",
                ),
            }],
            implemented_codes_without_examples: Vec::new(),
            unreachable_specs_with_examples: Vec::new(),
        };
        let json = serde_json::to_string_pretty(&m).expect("serialize");
        // Codes, status and the claim serialize as their written forms.
        assert!(json.contains("\"E370\""));
        assert!(json.contains("\"implemented\""));
        assert!(json.contains("\"violates\""));
        // The newtype is `serde(transparent)`, so the wire format is unchanged.
        assert!(json.contains("\"spec/errors/E370_retrace_missing_content.md\""));
        let back: ValidationManifest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(m, back);
    }
}
