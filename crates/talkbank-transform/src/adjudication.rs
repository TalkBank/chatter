//! Adjudication core: drive operator decisions over pending
//! adjudications, write resolved entries to the override file.
//!
//! This module is the testability-seam-bearing substrate for the
//! `chatter adjudicate` CLI subcommand. The CLI shim is a thin
//! wrapper around [`run_adjudication`]; the L4 test harness drives
//! the same core with a [`ScriptedPrompter`] standing in for human
//! input.
//!
//! Authoritative design: `book/src/architecture/adjudication-workflow.md`.
//! Currently supports the `speaker-id-low-confidence` adjudication
//! kind with `AcceptSuggested` decisions. Other kinds and decision
//! variants are scaffolded by the enum shapes but not yet
//! implemented.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::speaker_id::{
    DecisionEngine, InsertedRoleSpec, JudgmentProvenance, MergeOverride, OverrideFile,
    SpeakerAction,
};

/// Errors arising from the adjudication core.
#[derive(Debug, thiserror::Error)]
pub enum AdjudicationError {
    /// The prompter failed to produce a decision for the given
    /// session. For `ScriptedPrompter` this means the scripted
    /// decision sheet had no matching `(session_id, kind)` pair;
    /// for `TerminalPrompter` it means the operator entered an
    /// unparseable response.
    #[error("prompter could not produce a decision for session {session_id:?}: {detail}")]
    PrompterFailed {
        /// The session for which no decision was produced.
        session_id: String,
        /// Operator-facing detail from the prompter implementation.
        detail: String,
    },

    /// The operator's decision shape does not match the adjudication
    /// `kind` of the pending entry. Example: `AcceptSuggested` on a
    /// kind that has no `suggested` field, or `OverrideMapping` on a
    /// non-speaker-id kind.
    #[error(
        "decision kind {decision:?} does not match pending kind {pending:?} for session {session_id:?}"
    )]
    DecisionKindMismatch {
        /// The session being adjudicated.
        session_id: String,
        /// The pending entry's `kind` field.
        pending: AdjudicationKind,
        /// A short description of the decision shape supplied.
        decision: String,
    },

    /// I/O error reading or writing one of the adjudication files
    /// (pending-adjudications or scripted-decisions). The `path`
    /// field makes the error self-diagnosing without depending on
    /// the CLI shim to re-prefix.
    #[error("adjudication file I/O error at {}: {source}", path.display())]
    FileIo {
        /// The file path that failed.
        path: PathBuf,
        /// Underlying OS error.
        #[source]
        source: std::io::Error,
    },

    /// I/O error on stdin/stdout used by interactive prompters.
    /// No path because the failure is on the terminal stream itself.
    #[error("adjudication terminal I/O error: {0}")]
    TerminalIo(#[from] std::io::Error),

    /// TOML parse / serialize error.
    #[error("adjudication TOML error: {0}")]
    Toml(String),
}

/// Discriminator over the adjudication points the pipeline has.
/// See `adjudication-workflow.md` §"The known adjudication points"
/// for the full list of intended kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdjudicationKind {
    /// `chatter speaker-id` reference-mode Jaccard margin below
    /// threshold. The operator supplies a per-speaker mapping +
    /// inserted role.
    SpeakerIdLowConfidence,
    /// Parent-sample session needs a role tag (`MOT` vs `FAT` etc.)
    /// for an already-identified parent speaker. No Jaccard work; the
    /// operator picks based on external evidence (contributor data
    /// sheet, audio).
    ParentRoleLookup,
    /// A post-merge sanity scan detected evidence that the pass-1
    /// auto-decision was likely wrong despite passing the confidence
    /// threshold (e.g., utterance-length asymmetry inverted between
    /// anchor and inserted speakers). Carries the scan's corrected
    /// mapping suggestion + a diagnostic reason string.
    SanityScanMisclassification,
}

/// The algorithm's "would-have-chosen" mapping for a
/// speaker-id-low-confidence pending entry. The operator can accept
/// this verbatim via [`OperatorDecision::AcceptSuggested`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedSpeakerIdMapping {
    /// Per-speaker action map the algorithm would have applied had
    /// the confidence threshold been lower.
    pub mapping: BTreeMap<String, SpeakerAction>,
    /// The inserted-role spec the algorithm would have paired with
    /// the mapping (typically from the CLI's `--inserted-role`).
    pub inserted_role: InsertedRoleSpec,
}

/// One pending adjudication, carrying the inputs + evidence the
/// adjudication tool needs to prompt the operator.
///
/// Kind-specific data lives inside [`PendingKindData`]; the
/// serde-flattened tag puts the `kind = "..."` discriminator at the
/// entry's top level in the TOML wire format, matching the
/// authoritative format spec at
/// `book/src/architecture/adjudication-workflow.md`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingEntry {
    /// The session ID this adjudication is for. Must match the
    /// session ID under which the resolved decision will be written
    /// in the override file.
    pub session_id: String,
    /// When the pending entry was created (by the orchestrator's
    /// pass 1).
    pub created_at: DateTime<Utc>,
    /// Kind-specific payload. The kind itself is encoded as a serde
    /// tag on the enum, flattened into the entry-level TOML.
    #[serde(flatten)]
    pub data: PendingKindData,
    /// Per-speaker Jaccard scores recorded at decision time. Empty
    /// when scores are not applicable to the kind.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub scores: BTreeMap<String, f64>,
    /// Winner→runner-up margin. `None` when not applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin: Option<f64>,
    /// The confidence threshold the orchestrator used. `None` when
    /// the kind has no threshold (e.g., parent-role-lookup).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold_used: Option<f64>,
    /// Which engine produced the suggestion carried by this entry.
    /// Absent (defaulting to deterministic) in pre-provenance pending
    /// files. Skipped on serialize when deterministic so existing pending
    /// files stay byte-identical; LLM entries write `engine = "llm"`.
    #[serde(default, skip_serializing_if = "DecisionEngine::is_deterministic")]
    pub engine: DecisionEngine,
    /// LLM audit trail; present only when `engine = Llm`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub judgment: Option<JudgmentProvenance>,
}

impl PendingEntry {
    /// Discriminator of the kind-specific payload, useful for
    /// log messages and audit-trail summaries that don't need to
    /// destructure the data.
    pub fn kind(&self) -> AdjudicationKind {
        self.data.kind()
    }
}

/// Per-kind payload carried by [`PendingEntry`]. Each variant's
/// fields are the inputs + algorithmically-derived defaults the
/// operator needs to make the decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PendingKindData {
    /// `chatter speaker-id` reference-mode Jaccard margin below
    /// threshold. The operator either accepts the algorithm's
    /// suggested mapping or supplies an override.
    SpeakerIdLowConfidence {
        /// The algorithm's would-have-applied choice, operator
        /// can `AcceptSuggested` to apply verbatim.
        suggested: SuggestedSpeakerIdMapping,
    },
    /// Parent-sample session needs a role tag (`MOT` vs `FAT` etc.)
    /// for an already-identified parent speaker. No Jaccard
    /// inference; the operator picks the role based on external
    /// evidence (contributor data sheet, audio review).
    ParentRoleLookup {
        /// The donor speaker code that needs role assignment.
        /// Captured so the operator can see which speaker the
        /// decision applies to.
        donor_speaker: String,
        /// Pre-recorded per-speaker mapping for the eventual
        /// override entry. The operator's chosen role gets
        /// paired with this mapping; the mapping itself is
        /// already determined by the orchestrator.
        speaker_mapping: BTreeMap<String, SpeakerAction>,
    },
    /// A post-merge sanity scan detected evidence that the pass-1
    /// auto-decision was likely wrong despite passing the confidence
    /// threshold. The operator either accepts the scan's corrected
    /// mapping (`AcceptSuggested`) or supplies their own
    /// (`OverrideMapping`); accepting yields a new override entry
    /// that, on pass-2 re-merge with `--override-file`, produces a
    /// corrected merged file.
    SanityScanMisclassification {
        /// The mapping the scan thinks is correct (the swap, etc.).
        /// Mirrors the speaker-id-low-confidence `suggested` shape so
        /// the adjudication apply-path can be shared.
        suggested: SuggestedSpeakerIdMapping,
        /// Diagnostic text from the scan explaining what triggered
        /// the flag. Surfaced to the operator (terminal prompt, audit
        /// trail) so they can decide whether to trust the scan.
        reason: String,
    },
}

impl PendingKindData {
    /// The [`AdjudicationKind`] discriminator for this payload.
    pub fn kind(&self) -> AdjudicationKind {
        match self {
            Self::SpeakerIdLowConfidence { .. } => AdjudicationKind::SpeakerIdLowConfidence,
            Self::ParentRoleLookup { .. } => AdjudicationKind::ParentRoleLookup,
            Self::SanityScanMisclassification { .. } => {
                AdjudicationKind::SanityScanMisclassification
            }
        }
    }
}

/// The top-level pending-adjudications document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAdjudications {
    /// Schema version of the pending-adjudications file format.
    pub schema_version: u32,
    /// In-flight entries; ordered, may be empty.
    pub entries: Vec<PendingEntry>,
}

impl Default for PendingAdjudications {
    fn default() -> Self {
        Self {
            schema_version: 1,
            entries: Vec::new(),
        }
    }
}

impl PendingAdjudications {
    /// Read a pending-adjudications file from disk. Refuses
    /// unknown schema versions; uses `default()` only when the
    /// caller supplies a path that does not exist via
    /// [`Self::read_or_default`].
    pub fn read(path: &Path) -> Result<Self, AdjudicationError> {
        let bytes = fs::read_to_string(path).map_err(|e| AdjudicationError::FileIo {
            path: path.to_path_buf(),
            source: e,
        })?;
        let parsed: PendingAdjudications =
            toml::from_str(&bytes).map_err(|e| AdjudicationError::Toml(e.to_string()))?;
        Ok(parsed)
    }

    /// Read the file or return an empty default if the path doesn't
    /// exist. Matches the `OverrideFile::read_or_default` ergonomics
    /// so first-run batches don't need a pre-created file.
    pub fn read_or_default(path: &Path) -> Result<Self, AdjudicationError> {
        match fs::read_to_string(path) {
            Ok(s) => {
                let parsed: PendingAdjudications =
                    toml::from_str(&s).map_err(|e| AdjudicationError::Toml(e.to_string()))?;
                Ok(parsed)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(AdjudicationError::FileIo {
                path: path.to_path_buf(),
                source: e,
            }),
        }
    }

    /// Serialize to TOML and write to disk via `.tmp` + rename so a
    /// crash mid-write leaves the prior file intact.
    pub fn write(&self, path: &Path) -> Result<(), AdjudicationError> {
        let serialized =
            toml::to_string_pretty(self).map_err(|e| AdjudicationError::Toml(e.to_string()))?;
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, serialized).map_err(|e| AdjudicationError::FileIo {
            path: tmp.clone(),
            source: e,
        })?;
        fs::rename(&tmp, path).map_err(|e| AdjudicationError::FileIo {
            path: path.to_path_buf(),
            source: e,
        })?;
        Ok(())
    }
}

/// One operator decision on a pending entry. The variant determines
/// the apply-logic in `apply_decision`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorDecision {
    /// Accept the algorithm's suggested choice verbatim. The pending
    /// entry's `suggested` field becomes the override entry's
    /// `mapping` + `inserted_role`.
    AcceptSuggested {
        /// Optional operator note recorded in the override entry.
        note: Option<String>,
    },
    /// Override the algorithm's suggestion with an operator-supplied
    /// mapping. Used when the operator has external evidence (audio
    /// review, contributor data sheet) that contradicts the
    /// algorithm's per-speaker scoring. Both `mapping` and
    /// `inserted_role` come from the operator; the pending entry's
    /// `suggested` field is ignored at apply time but preserved in
    /// the pending file's audit trail.
    OverrideMapping {
        /// Operator-supplied per-speaker actions. Must cover every
        /// speaker the merge stage will see in the donor file (same
        /// rule as the algorithm's mapping).
        mapping: BTreeMap<String, SpeakerAction>,
        /// Operator-supplied inserted-role spec. May differ from the
        /// pending entry's suggested inserted role (e.g., MOT instead
        /// of INV after the operator confirms it's a parent sample).
        inserted_role: InsertedRoleSpec,
        /// Operator note explaining the override. Strongly
        /// recommended on this path, captures the *why* a future
        /// reader would want.
        note: Option<String>,
    },
    /// Parent-role-lookup decision: operator picks a role for an
    /// already-identified parent speaker. The mapping comes from the
    /// pending entry's `speaker_mapping` field; only the
    /// `inserted_role` is operator-supplied.
    ChooseRole {
        /// Operator-supplied role spec (e.g. `{ code: "MOT", tag:
        /// "Mother" }`). Becomes the override entry's
        /// `inserted_role`.
        inserted_role: InsertedRoleSpec,
        /// Operator note explaining the choice. Recommended when
        /// the source isn't obvious (e.g., audio-based judgment vs
        /// contributor data sheet).
        note: Option<String>,
    },
}

/// Operator-supplied input to the adjudication tool. Implementations:
/// `ScriptedPrompter` for tests and scripted operator workflows;
/// `TerminalPrompter` (not yet implemented) for interactive use.
pub trait Prompter {
    /// Prompt the operator for a decision on `entry`. Implementations
    /// should produce a decision whose shape is compatible with
    /// `entry.kind`; mismatches are surfaced as
    /// [`AdjudicationError::DecisionKindMismatch`] at apply time.
    fn ask(&mut self, entry: &PendingEntry) -> Result<OperatorDecision, AdjudicationError>;
}

/// Returns scripted operator decisions in `(session_id, decision)`
/// order. Used by L4 tests and by the CLI's `--scripted` mode.
pub struct ScriptedPrompter {
    decisions: Vec<(String, OperatorDecision)>,
    cursor: usize,
}

impl ScriptedPrompter {
    /// Construct from an ordered list of `(session_id, decision)`
    /// pairs. The prompter answers `ask()` calls in order; each call
    /// matches its session ID against the next scripted entry.
    pub fn from_decisions(decisions: Vec<(String, OperatorDecision)>) -> Self {
        Self {
            decisions,
            cursor: 0,
        }
    }

    /// Read scripted decisions from a TOML file matching the format
    /// in `book/src/architecture/adjudication-workflow.md` §"The
    /// `--scripted` mode". Used by `chatter adjudicate --scripted`
    /// and any test that wants to share the on-disk fixture format
    /// with L3 subprocess tests.
    pub fn read_toml(path: &Path) -> Result<Self, AdjudicationError> {
        let bytes = fs::read_to_string(path).map_err(|e| AdjudicationError::FileIo {
            path: path.to_path_buf(),
            source: e,
        })?;
        let parsed: ScriptedDecisions =
            toml::from_str(&bytes).map_err(|e| AdjudicationError::Toml(e.to_string()))?;
        let decisions = parsed
            .decisions
            .into_iter()
            .map(|d| (d.session_id, d.choice.into()))
            .collect();
        Ok(Self {
            decisions,
            cursor: 0,
        })
    }
}

/// Top-level scripted-decisions TOML document. See
/// `adjudication-workflow.md` §"The `--scripted` mode" for the
/// authoritative format.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScriptedDecisions {
    /// Schema version of the scripted-decisions file format.
    // Deserialized from the scripted-decisions file format; not read in logic.
    #[allow(dead_code)]
    schema_version: u32,
    /// Operator decisions to apply, matched to pending entries by
    /// session_id in document order.
    decisions: Vec<ScriptedDecisionEntry>,
}

/// One scripted decision entry. `kind` documents which adjudication
/// kind the operator was responding to; the apply-logic dispatches
/// on the corresponding [`PendingEntry::kind`], so `kind` here is
/// audit-trail metadata rather than a discriminator.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScriptedDecisionEntry {
    session_id: String,
    // Deserialized from the scripted-decisions file format; not read in logic.
    #[allow(dead_code)]
    kind: AdjudicationKind,
    choice: ScriptedChoice,
}

/// Variant-tagged TOML representation of [`OperatorDecision`]. The
/// `kind` field discriminates; other fields are variant-specific.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum ScriptedChoice {
    /// `choice = { kind = "accept-suggested", note = "..." }`
    AcceptSuggested {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    /// `choice = { kind = "override-mapping", mapping = { … }, inserted_role = { … }, note = "..." }`
    OverrideMapping {
        mapping: BTreeMap<String, SpeakerAction>,
        inserted_role: InsertedRoleSpec,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    /// `choice = { kind = "choose-role", inserted_role = { … }, note = "..." }`
    ChooseRole {
        inserted_role: InsertedRoleSpec,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
}

impl From<ScriptedChoice> for OperatorDecision {
    fn from(choice: ScriptedChoice) -> Self {
        match choice {
            ScriptedChoice::AcceptSuggested { note } => OperatorDecision::AcceptSuggested { note },
            ScriptedChoice::OverrideMapping {
                mapping,
                inserted_role,
                note,
            } => OperatorDecision::OverrideMapping {
                mapping,
                inserted_role,
                note,
            },
            ScriptedChoice::ChooseRole {
                inserted_role,
                note,
            } => OperatorDecision::ChooseRole {
                inserted_role,
                note,
            },
        }
    }
}

impl Prompter for ScriptedPrompter {
    fn ask(&mut self, entry: &PendingEntry) -> Result<OperatorDecision, AdjudicationError> {
        let Some((expected_session, decision)) = self.decisions.get(self.cursor) else {
            return Err(AdjudicationError::PrompterFailed {
                session_id: entry.session_id.clone(),
                detail: format!(
                    "scripted prompter exhausted after {} decision(s); pending entry asks for one more",
                    self.cursor
                ),
            });
        };
        if expected_session != &entry.session_id {
            return Err(AdjudicationError::PrompterFailed {
                session_id: entry.session_id.clone(),
                detail: format!(
                    "scripted prompter next decision is for session {expected_session:?}, \
                     but pending entry is for {:?}",
                    entry.session_id
                ),
            });
        }
        let result = decision.clone();
        self.cursor += 1;
        Ok(result)
    }
}

/// Result of running adjudication over a pending-adjudications set.
#[derive(Debug, Clone)]
pub struct AdjudicationOutcome {
    /// Session IDs of resolved entries, in the order they were
    /// resolved.
    pub resolved: Vec<String>,
}

impl AdjudicationOutcome {
    /// Number of pending entries that were resolved into override
    /// entries on this run.
    pub fn resolved_count(&self) -> usize {
        self.resolved.len()
    }
}

/// Walk every pending entry, ask the prompter for an operator
/// decision, apply each decision to the override file. Resolved
/// pending entries are removed in place; the returned outcome lists
/// the resolved session IDs.
///
/// Stops at the first unrecoverable error (mismatched kind,
/// exhausted prompter, etc.); already-applied decisions remain in
/// the override file so a re-run can pick up where this one left off.
pub fn run_adjudication(
    pending: &mut PendingAdjudications,
    overrides: &mut OverrideFile,
    prompter: &mut dyn Prompter,
    operator: String,
) -> Result<AdjudicationOutcome, AdjudicationError> {
    let mut resolved: Vec<String> = Vec::new();
    let mut remaining: Vec<PendingEntry> = Vec::new();
    // Process in document order; clone into the working vec so we
    // can rebuild `pending.entries` from `remaining` after the loop.
    let drained: Vec<PendingEntry> = pending.entries.drain(..).collect();
    for entry in drained {
        let decision = prompter.ask(&entry)?;
        match apply_decision(&entry, &decision, &operator, overrides) {
            Ok(()) => resolved.push(entry.session_id.clone()),
            Err(e) => {
                // Restore the not-yet-processed entry plus the
                // failing one so re-running picks up state cleanly.
                remaining.push(entry);
                pending.entries.append(&mut remaining);
                return Err(e);
            }
        }
    }
    pending.entries = remaining;
    Ok(AdjudicationOutcome { resolved })
}

/// Apply one operator decision to the override file. Dispatches on
/// the pending entry's `kind` and the decision's variant; mismatches
/// are surfaced as
/// [`AdjudicationError::DecisionKindMismatch`].
fn apply_decision(
    entry: &PendingEntry,
    decision: &OperatorDecision,
    operator: &str,
    overrides: &mut OverrideFile,
) -> Result<(), AdjudicationError> {
    let now = Utc::now();
    let merge_override = match (&entry.data, decision) {
        (
            PendingKindData::SpeakerIdLowConfidence { suggested },
            OperatorDecision::AcceptSuggested { note },
        ) => MergeOverride::operator_decision(
            suggested.mapping.clone(),
            suggested.inserted_role.clone(),
            entry.scores.clone(),
            entry.margin,
            operator.to_string(),
            now,
            note.clone(),
        ),
        (
            PendingKindData::SpeakerIdLowConfidence { .. },
            OperatorDecision::OverrideMapping {
                mapping,
                inserted_role,
                note,
            },
        ) => MergeOverride::operator_decision(
            // Operator's mapping replaces the algorithm's suggestion;
            // the pending entry's scores+margin ride through for audit.
            mapping.clone(),
            inserted_role.clone(),
            entry.scores.clone(),
            entry.margin,
            operator.to_string(),
            now,
            note.clone(),
        ),
        (
            PendingKindData::ParentRoleLookup {
                speaker_mapping, ..
            },
            OperatorDecision::ChooseRole {
                inserted_role,
                note,
            },
        ) => MergeOverride::operator_decision(
            // Parent-role-lookup has no Jaccard inputs, scores+margin
            // intentionally empty/None.
            speaker_mapping.clone(),
            inserted_role.clone(),
            BTreeMap::new(),
            None,
            operator.to_string(),
            now,
            note.clone(),
        ),
        (
            PendingKindData::SanityScanMisclassification { suggested, .. },
            OperatorDecision::AcceptSuggested { note },
        ) => MergeOverride::operator_decision(
            // The scan's suggested mapping rides through verbatim. The
            // diagnostic `reason` string lives in the pending entry; it
            // is intentionally NOT copied into the override entry,
            // the override is a forward-looking decision, not a log of
            // why the decision was needed.
            suggested.mapping.clone(),
            suggested.inserted_role.clone(),
            entry.scores.clone(),
            entry.margin,
            operator.to_string(),
            now,
            note.clone(),
        ),
        (
            PendingKindData::SanityScanMisclassification { .. },
            OperatorDecision::OverrideMapping {
                mapping,
                inserted_role,
                note,
            },
        ) => MergeOverride::operator_decision(
            mapping.clone(),
            inserted_role.clone(),
            entry.scores.clone(),
            entry.margin,
            operator.to_string(),
            now,
            note.clone(),
        ),
        (kind_data, decision) => {
            return Err(AdjudicationError::DecisionKindMismatch {
                session_id: entry.session_id.clone(),
                pending: kind_data.kind(),
                decision: format!("{decision:?}"),
            });
        }
    };
    overrides.upsert(entry.session_id.clone(), merge_override);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pre-provenance pending TOML that has no `engine` field must parse
    /// with `engine == Deterministic` and `judgment == None`. This verifies
    /// that adding the new fields with `serde(default)` did not break
    /// backward compatibility with files written by earlier binaries.
    #[test]
    fn legacy_pending_toml_without_engine_defaults_to_deterministic() {
        // Minimal valid pending TOML in the pre-provenance format: no
        // `engine` or `judgment` keys at all. The `inserted_role` is an
        // inline table; the speaker-id-low-confidence kind tag must be
        // present for the flattened enum to parse.
        let toml = r#"
schema_version = 1

[[entries]]
session_id = "legacy-session-1"
created_at = "2026-01-01T00:00:00Z"
kind = "speaker-id-low-confidence"

[entries.suggested]
inserted_role = { code = "INV", tag = "Investigator" }

[entries.suggested.mapping]
PAR0 = "drop"
PAR1 = "rename"
"#;

        let parsed: PendingAdjudications =
            toml::from_str(toml).expect("legacy pending TOML must parse");

        assert_eq!(parsed.entries.len(), 1, "must have exactly one entry");

        let entry = &parsed.entries[0];

        assert_eq!(
            entry.engine,
            DecisionEngine::Deterministic,
            "missing engine field must default to Deterministic"
        );
        assert!(
            entry.judgment.is_none(),
            "missing judgment field must default to None"
        );
        assert_eq!(entry.session_id, "legacy-session-1");
    }

    /// Confirm that a `Deterministic` entry does NOT serialize the `engine`
    /// field, keeping pre-provenance files byte-identical on write.
    #[test]
    fn deterministic_entry_omits_engine_field_on_serialize() {
        let entry = PendingEntry {
            session_id: "sess-check".to_string(),
            created_at: chrono::Utc::now(),
            data: PendingKindData::SpeakerIdLowConfidence {
                suggested: SuggestedSpeakerIdMapping {
                    mapping: {
                        let mut m = std::collections::BTreeMap::new();
                        m.insert("PAR0".to_string(), SpeakerAction::Drop);
                        m
                    },
                    inserted_role: InsertedRoleSpec {
                        code: "INV".to_string(),
                        tag: "Investigator".to_string(),
                    },
                },
            },
            scores: std::collections::BTreeMap::new(),
            margin: None,
            threshold_used: None,
            engine: DecisionEngine::Deterministic,
            judgment: None,
        };
        let doc = PendingAdjudications {
            schema_version: 1,
            entries: vec![entry],
        };

        let toml_str = toml::to_string_pretty(&doc).expect("must serialize");

        assert!(
            !toml_str.contains("engine"),
            "Deterministic entries must NOT write an 'engine' field; got:\n{toml_str}"
        );
        assert!(
            !toml_str.contains("judgment"),
            "Deterministic entries must NOT write a 'judgment' field; got:\n{toml_str}"
        );
    }
}
