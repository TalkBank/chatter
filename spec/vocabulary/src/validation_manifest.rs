//! Typed expectations manifest linking each generated validation fixture to
//! its spec's code, its CLAIM, the rules it runs under, and its
//! implementation status. This is the only contract between the spec
//! generator and the data-driven runner; it is serialized to the corpus dir
//! as `manifest.json`.
//!
//! # Why it lives in the vocabulary crate
//!
//! The generator is in the `spec/` workspace and the runner is in the root
//! one, and the runner cannot depend on the generator. So it MIRRORED this
//! struct by field name, with a comment saying so, and the mirror had drifted
//! at least once: an earlier local status enum carried three variants where
//! the generator had four. That copy would have failed to deserialize rather
//! than mis-judging anything, and no `unreachable_from_chat` entry ever
//! existed while it stood, so the drift was latent. Latent is the point: a
//! wire format read from two workspaces belongs in the crate that exists so
//! both workspaces read one definition, and the mirror is deleted rather than
//! re-synced. Adding `rules` to it would have been the third field written
//! twice.
//!
//! The two gate lists are deliberately NOT `#[serde(default)]`. The
//! generator always writes both, so a missing or renamed field must be a
//! loud deserialization failure rather than an empty gate that passes.

use serde::{Deserialize, Serialize};

use crate::frontmatter::{Claim, RuleProfile};
use crate::paths::RepoRelativePath;
use crate::{SpecErrorCode, Status};

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

impl std::fmt::Display for FixtureName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<std::path::Path> for FixtureName {
    /// A fixture name IS a path component, so the runner joins it onto the
    /// corpus directory directly. Reaching for `as_str()` at each join site is
    /// what let the runner's deleted mirror hold this field as a bare
    /// `String`.
    fn as_ref(&self) -> &std::path::Path {
        std::path::Path::new(&self.0)
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
    pub claim: Claim,
    /// The validation rules the fixture must be run under.
    ///
    /// Absent from the JSON when it is the default, which is 430 of the 441
    /// entries. `serde(default)` alone governs only READING, so the first
    /// regeneration wrote `"rules": "default"` four hundred and thirty times
    /// while the doc here claimed it would not; `skip_serializing_if` is what
    /// makes the sentence true.
    #[serde(default, skip_serializing_if = "RuleProfile::is_default")]
    pub rules: RuleProfile,
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
    pub implemented_codes_without_examples: Vec<SpecErrorCode>,

    /// Specs marked `unreachable_from_chat` that nonetheless carry an example.
    ///
    /// An example means some CHAT input reaches the rule, so the status is
    /// wrong. Without this, the new status would be a way to opt any rule out
    /// of its fixture obligation.
    pub unreachable_specs_with_examples: Vec<RepoRelativePath>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The manifest survives a JSON round trip with its written forms intact.
    ///
    /// SURVIVES a type change, and says which category: this is a WIRE
    /// FORMAT. The generator writes this file and a runner in the OTHER cargo
    /// workspace reads it, so the bytes are the contract and no signature
    /// spans the two processes.
    #[test]
    fn round_trips_json() {
        let m = ValidationManifest {
            fixtures: vec![ValidationFixtureEntry {
                fixture: FixtureName::new("E370_retrace.cha"),
                code: SpecErrorCode::parse("E370").expect("valid code"),
                claim: Claim::Violates,
                rules: RuleProfile::Default,
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

    /// An opt-in rules profile reaches the far side of the wire.
    ///
    /// The default is `#[serde(default)]` and so absent from the JSON, which
    /// is what keeps four hundred entries from repeating it; that makes the
    /// NON-default case the one worth pinning, since a profile that silently
    /// failed to serialize would run every opt-in fixture under the default
    /// rules and report that its rule does not fire.
    #[test]
    fn an_opt_in_rules_profile_survives_the_wire() {
        let entry = ValidationFixtureEntry {
            fixture: FixtureName::new("E351_0.cha"),
            code: SpecErrorCode::parse("E351").expect("valid code"),
            claim: Claim::Violates,
            rules: RuleProfile::StrictLinkers,
            status: Status::Implemented,
            source_spec: RepoRelativePath::new(
                std::path::Path::new("/checkout"),
                "/checkout/spec/errors/E351.md",
            ),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(
            json.contains("\"strict_linkers\""),
            "the profile must be on the wire: {json}"
        );
        let back: ValidationFixtureEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.rules, RuleProfile::StrictLinkers);
    }

    /// An entry with no `rules` key reads as the default rule set.
    ///
    /// The absence has to MEAN something, and it means what a bare
    /// `chatter validate` runs. Pinned because the alternative reading, that
    /// an entry written before the field existed is unreadable, would make
    /// every regeneration a flag day.
    #[test]
    fn an_absent_rules_key_is_the_default_profile() {
        let json = r#"{
            "fixture": "E370_retrace.cha",
            "code": "E370",
            "claim": "violates",
            "status": "implemented",
            "source_spec": "spec/errors/E370.md"
        }"#;
        let entry: ValidationFixtureEntry = serde_json::from_str(json).expect("deserialize");
        assert_eq!(entry.rules, RuleProfile::Default);
    }
}
