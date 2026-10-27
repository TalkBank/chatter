//! Owned decision sources and their completion transition.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{AdjudicationError, AdjudicationKind, PendingEntry};
use crate::speaker_id::{InsertedRoleSpec, SpeakerAction};

/// One operator decision on a pending entry. The variant determines
/// the apply-logic in `apply_decision`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorDecision {
    /// Accept the algorithm's suggested choice verbatim. The pending
    /// entry's `suggested` field becomes the override entry's
    /// `mapping` + `adult_roles`.
    AcceptSuggested {
        /// Optional operator note recorded in the override entry.
        note: Option<String>,
    },
    /// Override the algorithm's suggestion with an operator-supplied
    /// mapping. Used when the operator has external evidence (audio
    /// review, contributor data sheet) that contradicts the
    /// algorithm's per-speaker scoring. Both `mapping` and
    /// `adult_roles` come from the operator; the pending entry's
    /// `suggested` field is ignored at apply time but preserved in
    /// the pending file's audit trail.
    OverrideMapping {
        /// Operator-supplied per-speaker actions. Must cover every
        /// speaker the merge stage will see in the donor file (same
        /// rule as the algorithm's mapping).
        mapping: BTreeMap<String, SpeakerAction>,
        /// Operator-supplied per-speaker role assignments. May differ
        /// from the pending entry's suggested roles.
        adult_roles: BTreeMap<String, InsertedRoleSpec>,
        /// Operator note explaining the override. Strongly
        /// recommended on this path, captures the *why* a future
        /// reader would want.
        note: Option<String>,
    },
    /// Parent-role-lookup decision: operator picks a role for an
    /// already-identified parent speaker. The mapping comes from the
    /// pending entry's `speaker_mapping` field; only `adult_roles` is
    /// operator-supplied.
    ChooseRole {
        /// Operator-supplied role assignment, keyed by the single donor
        /// speaker code this pending entry names (`ParentRoleLookup`'s
        /// `donor_speaker` field).
        adult_roles: BTreeMap<String, InsertedRoleSpec>,
        /// Operator note explaining the choice. Recommended when
        /// the source isn't obvious (e.g., audio-based judgment vs
        /// contributor data sheet).
        note: Option<String>,
    },
}

/// Operator-supplied input to the adjudication tool. Implementations:
/// `ScriptedPrompter` for tests and scripted operator workflows;
/// the CLI's `TerminalPrompter` for interactive use.
pub trait Prompter {
    /// Prompt the operator for a decision on `entry`. Implementations
    /// should produce a decision whose shape is compatible with
    /// `entry.kind`; mismatches are surfaced as
    /// [`AdjudicationError::DecisionKindMismatch`] at apply time.
    fn ask(&mut self, entry: &PendingEntry) -> Result<OperatorDecision, AdjudicationError>;

    /// Consume this source after every pending entry was resolved. Scripted
    /// sources must refuse unconsumed decisions; interactive sources may finish
    /// without reading another terminal response.
    fn finish(self) -> Result<(), AdjudicationError>;
}

/// Returns scripted operator decisions in `(session_id, decision)`
/// order. Used by L4 tests and by the CLI's `--scripted` mode.
pub struct ScriptedPrompter {
    decisions: std::vec::IntoIter<(String, OperatorDecision)>,
}

impl ScriptedPrompter {
    /// Construct from an ordered list of `(session_id, decision)`
    /// pairs. The prompter answers `ask()` calls in order; each call
    /// matches its session ID against the next scripted entry.
    pub fn from_decisions(decisions: Vec<(String, OperatorDecision)>) -> Self {
        Self {
            decisions: decisions.into_iter(),
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
        Ok(Self::from_decisions(decisions))
    }
}

/// Top-level scripted-decisions TOML document. See
/// `adjudication-workflow.md` §"The `--scripted` mode" for the
/// authoritative format.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScriptedDecisions {
    /// Admission occurs during deserialization, before decisions are exposed.
    #[serde(rename = "schema_version")]
    _schema_version: ScriptedSchemaVersion,
    /// Operator decisions to apply, matched to pending entries by
    /// session_id in document order.
    decisions: Vec<ScriptedDecisionEntry>,
}

/// Both retained scripted formats use the current choice representation.
/// Their versions are independent of the pending-file schema version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
enum ScriptedSchemaVersion {
    V1,
    V2,
}

impl TryFrom<u32> for ScriptedSchemaVersion {
    type Error = String;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::V1),
            2 => Ok(Self::V2),
            other => Err(format!(
                "unsupported scripted-decisions schema_version {other}; supported versions are 1 and 2"
            )),
        }
    }
}

impl From<ScriptedSchemaVersion> for u32 {
    fn from(version: ScriptedSchemaVersion) -> Self {
        match version {
            ScriptedSchemaVersion::V1 => 1,
            ScriptedSchemaVersion::V2 => 2,
        }
    }
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
    /// `choice = { kind = "override-mapping", mapping = { … }, adult_roles = { … }, note = "..." }`
    OverrideMapping {
        mapping: BTreeMap<String, SpeakerAction>,
        adult_roles: BTreeMap<String, InsertedRoleSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
    /// `choice = { kind = "choose-role", adult_roles = { … }, note = "..." }`
    ChooseRole {
        adult_roles: BTreeMap<String, InsertedRoleSpec>,
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
                adult_roles,
                note,
            } => OperatorDecision::OverrideMapping {
                mapping,
                adult_roles,
                note,
            },
            ScriptedChoice::ChooseRole { adult_roles, note } => {
                OperatorDecision::ChooseRole { adult_roles, note }
            }
        }
    }
}

impl Prompter for ScriptedPrompter {
    fn ask(&mut self, entry: &PendingEntry) -> Result<OperatorDecision, AdjudicationError> {
        let Some((expected_session, decision)) = self.decisions.next() else {
            return Err(AdjudicationError::PrompterFailed {
                session_id: entry.session_id.clone(),
                detail: "scripted prompter exhausted; pending entry asks for one more".to_owned(),
            });
        };
        if expected_session != entry.session_id {
            return Err(AdjudicationError::PrompterFailed {
                session_id: entry.session_id.clone(),
                detail: format!(
                    "scripted prompter next decision is for session {expected_session:?}, \
                     but pending entry is for {:?}",
                    entry.session_id
                ),
            });
        }
        Ok(decision)
    }

    fn finish(mut self) -> Result<(), AdjudicationError> {
        match self.decisions.next() {
            None => Ok(()),
            Some((session_id, _)) => {
                Err(AdjudicationError::UnconsumedScriptedDecision { session_id })
            }
        }
    }
}
