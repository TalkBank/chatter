//! Persistent record of speaker-id adjudications.
//!
//! The override file is a TOML document keyed by session ID,
//! recording per-session decisions made by `chatter speaker-id` (or
//! its adjudication UI). It serves three purposes:
//!
//! 1. **Persistence**: replay prior adjudications without
//!    re-prompting the operator.
//! 2. **Audit trail**: record who decided what, when, on the
//!    basis of which Jaccard scores.
//! 3. **Interchange**: UI tools (CLI, VS Code, future web app)
//!    share the on-disk contract.
//!
//! Authoritative format spec:
//! `book/src/chatter/integrating/merge-overrides.md`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use talkbank_model::{ParticipantRole, SpeakerCode};

use super::error::SpeakerIdError;
use super::identify::DonorMatchReport;
use super::mapping::{MappingSpec, SpeakerAssignment};
use super::provenance::{DecisionEngine, JudgmentProvenance};

/// Current schema version supported by this binary. Readers refuse
/// files with any other value; there is no implicit version, no
/// fallback, no auto-migration. See the format spec §6. Bumped from 1
/// to 2 for the per-speaker `adult_roles` map (was `inserted_role`, a
/// single shared field).
pub const CURRENT_SCHEMA_VERSION: u32 = 2;

/// Errors arising from override-file I/O or parsing.
#[derive(Debug, thiserror::Error)]
pub enum OverrideFileError {
    /// The file's `schema_version` is missing or not equal to
    /// [`CURRENT_SCHEMA_VERSION`]. The binary refuses to interpret
    /// unknown versions rather than risk silent misreads.
    #[error("unsupported override-file schema_version {found:?}; this binary supports {supported}")]
    UnsupportedSchemaVersion {
        /// The schema version as it was read from the file (None if
        /// the field was absent entirely).
        found: Option<u32>,
        /// The schema version this binary supports.
        supported: u32,
    },

    /// I/O error reading or writing the file.
    #[error("override-file I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// TOML parse / serialize error.
    #[error("override-file TOML error: {0}")]
    Toml(String),
}

/// How a speaker-id decision was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverrideMode {
    /// Reference-mode auto-decide above confidence threshold.
    Auto,
    /// Operator-supplied `--mapping` (typically after a low-confidence
    /// reference-mode attempt).
    Explicit,
    /// Replay of a prior decision read from another override file.
    Override,
}

/// Action applied to one speaker in the input file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpeakerAction {
    /// Rename the speaker per its own entry in `adult_roles`.
    Rename,
    /// Drop the speaker's utterances and header rows entirely.
    Drop,
}

/// Inline-table form of the inserted-role spec recorded in each
/// override entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InsertedRoleSpec {
    /// CHAT speaker code (e.g. `INV`, or `INV1` when disambiguated from
    /// a same-role collision).
    pub code: String,
    /// CHAT standard role tag (e.g. `Investigator`).
    pub tag: String,
    /// Specific-role label for `@Participants`' name/specific-role slot
    /// (e.g. `First_Investigator`), set only when two adults in the same
    /// judgment share `tag` and need the CHAT manual's `CHI1`/`CHI2`-style
    /// disambiguation. `None` for the ordinary single-adult-per-role case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specific_role: Option<String>,
}

impl InsertedRoleSpec {
    /// Build from the typed CHAT primitives, with no specific-role label.
    pub fn new(code: &SpeakerCode, tag: &ParticipantRole) -> Self {
        Self {
            code: code.as_str().to_string(),
            tag: tag.as_str().to_string(),
            specific_role: None,
        }
    }
}

impl MergeOverride {
    /// Build an `Auto`-mode entry from a successful reference-mode
    /// run. Collapses the four conversions (mapping → on-disk action
    /// map, typed scores → BTreeMap, typed margin → `Option<f64>`, plus
    /// constants for mode/note/flags) into one constructor so callers
    /// don't have to thread the data-model details through their
    /// glue code.
    pub fn auto_decision(
        mapping: &MappingSpec,
        report: &DonorMatchReport,
        adult_roles: BTreeMap<String, InsertedRoleSpec>,
        operator: String,
        decided_at: DateTime<Utc>,
    ) -> Self {
        Self {
            mode: OverrideMode::Auto,
            adult_roles,
            mapping: mapping_to_serializable(mapping),
            scores: report.scores_to_serializable(),
            margin: report.margin_to_serializable(),
            operator,
            decided_at,
            note: None,
            flags: Vec::new(),
            engine: DecisionEngine::Deterministic,
            judgment: None,
        }
    }

    /// Build an `Explicit`-mode entry from an operator-driven
    /// adjudication. Centralizes the constants (mode, decided_at
    /// origin, empty flags) so the three `apply_decision` arms in
    /// the adjudication core don't drift on the shared header.
    pub fn operator_decision(
        mapping: BTreeMap<String, SpeakerAction>,
        adult_roles: BTreeMap<String, InsertedRoleSpec>,
        scores: BTreeMap<String, f64>,
        margin: Option<f64>,
        operator: String,
        decided_at: DateTime<Utc>,
        note: Option<String>,
    ) -> Self {
        Self {
            mode: OverrideMode::Explicit,
            adult_roles,
            mapping,
            scores,
            margin,
            operator,
            decided_at,
            note,
            flags: Vec::new(),
            engine: DecisionEngine::Deterministic,
            judgment: None,
        }
    }

    /// Translate this entry's recorded decision into the in-memory
    /// [`MappingSpec`] consumed by `apply_mapping`. Every recorded
    /// `Rename` action looks up its OWN entry in `adult_roles`; every
    /// `Drop` becomes `SpeakerAssignment::Drop`.
    ///
    /// Returns [`SpeakerIdError::OverrideRenameMissingRole`] if a
    /// `Rename` action has no matching `adult_roles` entry. The
    /// sanctioned constructors (`auto_decision`, `operator_decision`)
    /// and every writer path maintain that covering invariant, so this
    /// only fires on a hand-corrupted override file; it is surfaced as a
    /// typed error rather than a panic (this crate forbids production
    /// panics).
    pub fn to_mapping_spec(&self) -> Result<MappingSpec, SpeakerIdError> {
        self.mapping
            .iter()
            .map(|(spk, action)| {
                let speaker = SpeakerCode::new(spk);
                let assignment = match action {
                    SpeakerAction::Drop => SpeakerAssignment::Drop,
                    SpeakerAction::Rename => {
                        let role_spec = self.adult_roles.get(spk).ok_or_else(|| {
                            SpeakerIdError::OverrideRenameMissingRole {
                                speaker: SpeakerCode::new(spk),
                            }
                        })?;
                        SpeakerAssignment::Rename {
                            code: SpeakerCode::new(&role_spec.code),
                            role: ParticipantRole::new(&role_spec.tag),
                            specific_role: role_spec
                                .specific_role
                                .as_deref()
                                .map(talkbank_model::ParticipantName::new),
                        }
                    }
                };
                Ok((speaker, assignment))
            })
            .collect()
    }
}

/// Convert a [`MappingSpec`] (in-memory typed) into the on-disk
/// `BTreeMap<String, SpeakerAction>` shape recorded in override-file
/// entries. The action's payload (rename target code/role) is
/// captured separately in the entry's `adult_roles` map, here
/// we only need the action discriminant.
fn mapping_to_serializable(mapping: &MappingSpec) -> BTreeMap<String, SpeakerAction> {
    mapping
        .iter()
        .map(|(spk, action)| {
            let act = match action {
                SpeakerAssignment::Drop => SpeakerAction::Drop,
                SpeakerAssignment::Rename { .. } => SpeakerAction::Rename,
            };
            (spk.as_str().to_string(), act)
        })
        .collect()
}

/// A single override-file entry: the operator decision for one
/// session. See `merge-overrides.md` for field semantics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeOverride {
    /// How the decision was made.
    pub mode: OverrideMode,

    /// Per-donor-speaker-code role assignment, for every speaker whose
    /// `mapping` action is `Rename`. Invariant: every `Rename` key in
    /// `mapping` has a matching entry here (enforced by the sanctioned
    /// constructors; [`Self::to_mapping_spec`] returns an error rather
    /// than panicking if a hand-edited file violates it).
    pub adult_roles: BTreeMap<String, InsertedRoleSpec>,

    /// Map from input speaker codes to actions. Every speaker that
    /// exists in the input must appear here.
    pub mapping: BTreeMap<String, SpeakerAction>,

    /// Per-speaker Jaccard scores recorded at decision time.
    /// Present for `Auto` (and `Explicit` decisions that followed a
    /// low-confidence reference-mode attempt).
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub scores: BTreeMap<String, f64>,

    /// Winner-score / runner-up-score margin. Serialized as a
    /// number; the divide-by-zero case is recorded as `f64::INFINITY`
    /// (the spec also permits the string `"unbounded"` for that
    /// case; this implementation uses the numeric form for now).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub margin: Option<f64>,

    /// Free-form identifier of the operator who made the decision.
    pub operator: String,

    /// When the decision was made (RFC 3339).
    pub decided_at: DateTime<Utc>,

    /// Free-text operator note. Strongly recommended for `Explicit`
    /// and `Override` modes. `None` and `Some("")` are
    /// preserved-distinct on the in-memory side (the boundary code
    /// that builds the override entry shouldn't collapse them); on
    /// disk, `None` is omitted entirely and `Some("")` serializes as
    /// `note = ""`. Operators reading the file see absence vs
    /// empty-string as the same "no note recorded", but the typed
    /// boundary preserves the distinction in case future tooling
    /// (`chatter audit`?) cares.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,

    /// Operator-supplied audit flags (e.g. `"diarization-mixed"`,
    /// `"best-guess"`).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub flags: Vec<String>,

    /// Which engine produced this decision. Absent in pre-provenance
    /// files, which deserialize as `Deterministic`.
    #[serde(default)]
    pub engine: DecisionEngine,

    /// LLM audit trail; present only for `engine = Llm` decisions.
    /// Invariant: `Some` if and only if `engine == Llm`; this coupling
    /// is not yet type-enforced (a future change may fold the two into
    /// one enum).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub judgment: Option<JudgmentProvenance>,
}

/// The full override-file document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverrideFile {
    /// Schema version. Always [`CURRENT_SCHEMA_VERSION`] when this
    /// binary writes; readers reject other values.
    pub schema_version: u32,

    /// Per-session entries, alphabetically ordered by session ID via
    /// the `BTreeMap` default.
    #[serde(flatten)]
    pub entries: BTreeMap<String, MergeOverride>,
}

impl Default for OverrideFile {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            entries: BTreeMap::new(),
        }
    }
}

impl OverrideFile {
    /// Read an override file from disk, or return an empty default
    /// (with the current schema version) if the path does not exist.
    /// Used by `--write-override` so an operator can run a batch
    /// without pre-creating the file. Operating directly on the read
    /// and matching on `NotFound` avoids the TOCTOU race between an
    /// `exists()` check and the actual open.
    pub fn read_or_default(path: &Path) -> Result<Self, OverrideFileError> {
        let bytes = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(OverrideFileError::Io(e)),
        };
        let file: OverrideFile =
            toml::from_str(&bytes).map_err(|e| OverrideFileError::Toml(e.to_string()))?;
        if file.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(OverrideFileError::UnsupportedSchemaVersion {
                found: Some(file.schema_version),
                supported: CURRENT_SCHEMA_VERSION,
            });
        }
        Ok(file)
    }

    /// Serialize to TOML and write to disk via a `.tmp` + rename so
    /// a crash mid-write leaves the prior file intact rather than
    /// truncated.
    pub fn write(&self, path: &Path) -> Result<(), OverrideFileError> {
        let serialized =
            toml::to_string_pretty(self).map_err(|e| OverrideFileError::Toml(e.to_string()))?;
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, serialized)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Insert (or replace) the entry for `session_id`.
    pub fn upsert(&mut self, session_id: String, entry: MergeOverride) {
        self.entries.insert(session_id, entry);
    }

    /// Look up the entry for `session_id`, returning `None` if
    /// absent. The replay path treats absence as an exit-2 precondition
    /// violation; tooling that wants to fall back may inspect the
    /// `None` directly.
    pub fn get(&self, session_id: &str) -> Option<&MergeOverride> {
        self.entries.get(session_id)
    }

    /// Iterate session IDs in deterministic order (alphabetical via
    /// `BTreeMap`). Useful for diagnostic listings when an operator
    /// passes an unknown session ID.
    pub fn session_ids(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// Iterate `(session_id, entry)` pairs whose `mode` is
    /// [`OverrideMode::Auto`]. Used by the post-merge sanity scan,
    /// which only flags algorithm-decided sessions, explicit-mode
    /// entries mean the operator already signed off, so flagging
    /// them again would be noise.
    pub fn auto_entries(&self) -> impl Iterator<Item = (&str, &MergeOverride)> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.mode == OverrideMode::Auto)
            .map(|(session_id, entry)| (session_id.as_str(), entry))
    }

    /// Iterate `(session_id, entry)` pairs whose decision was produced by
    /// an LLM ([`DecisionEngine::Llm`]). Used by audits that need to
    /// report which speaker assignments were model-made versus
    /// deterministic.
    pub fn llm_entries(&self) -> impl Iterator<Item = (&str, &MergeOverride)> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.engine == DecisionEngine::Llm)
            .map(|(session_id, entry)| (session_id.as_str(), entry))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `specific_role` must round-trip through TOML and default to `None`
    /// when absent (so pre-existing on-disk entries with just `code`/`tag`
    /// still parse once this field is added, since the schema bump in
    /// Task 8 covers the `adult_roles` shape, not this field).
    #[test]
    fn inserted_role_spec_specific_role_defaults_to_none_and_round_trips() {
        let without: InsertedRoleSpec = toml::from_str(
            r#"code = "INV"
tag = "Investigator""#,
        )
        .expect("must parse without specific_role");
        assert_eq!(without.specific_role, None);

        let with = InsertedRoleSpec {
            code: "INV1".to_string(),
            tag: "Investigator".to_string(),
            specific_role: Some("First_Investigator".to_string()),
        };
        let toml_str = toml::to_string(&with).expect("must serialize");
        let back: InsertedRoleSpec = toml::from_str(&toml_str).expect("must parse back");
        assert_eq!(back, with);
    }

    /// A `MergeOverride` with two Rename actions, each looked up in its
    /// own `adult_roles` entry, must produce a `MappingSpec` where each
    /// speaker gets its OWN code/role/specific_role, not one shared value.
    #[test]
    fn to_mapping_spec_applies_distinct_roles_per_speaker() {
        let mut mapping = BTreeMap::new();
        mapping.insert("PAR0".to_string(), SpeakerAction::Rename);
        mapping.insert("PAR1".to_string(), SpeakerAction::Rename);
        let mut adult_roles = BTreeMap::new();
        adult_roles.insert(
            "PAR0".to_string(),
            InsertedRoleSpec {
                code: "INV".to_string(),
                tag: "Investigator".to_string(),
                specific_role: None,
            },
        );
        adult_roles.insert(
            "PAR1".to_string(),
            InsertedRoleSpec {
                code: "FAT".to_string(),
                tag: "Father".to_string(),
                specific_role: None,
            },
        );
        let entry = MergeOverride {
            mode: OverrideMode::Explicit,
            adult_roles,
            mapping,
            scores: BTreeMap::new(),
            margin: None,
            operator: "test".to_string(),
            decided_at: Utc::now(),
            note: None,
            flags: Vec::new(),
            engine: DecisionEngine::Deterministic,
            judgment: None,
        };

        let spec = entry
            .to_mapping_spec()
            .expect("well-formed entry (every Rename has a role) must build a MappingSpec");
        match spec.get(&SpeakerCode::new("PAR0")) {
            Some(SpeakerAssignment::Rename { code, role, .. }) => {
                assert_eq!(code.as_str(), "INV");
                assert_eq!(role.as_str(), "Investigator");
            }
            other => panic!("expected Rename for PAR0; got {other:?}"),
        }
        match spec.get(&SpeakerCode::new("PAR1")) {
            Some(SpeakerAssignment::Rename { code, role, .. }) => {
                assert_eq!(code.as_str(), "FAT");
                assert_eq!(role.as_str(), "Father");
            }
            other => panic!("expected Rename for PAR1; got {other:?}"),
        }
    }
}
