//! Operator-supplied speaker-mapping spec: parsing + types.

use std::collections::HashMap;

use talkbank_model::{ParticipantRole, SpeakerCode};

use super::error::SpeakerIdError;

/// What to do with a speaker named in the input file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeakerAssignment {
    /// Drop the speaker entirely: their utterances are removed, their
    /// `@Participants` entry is removed, their `@ID` row is removed.
    /// Used when the speaker is authoritatively covered by a separate
    /// reference file that the downstream merge stage will pull from.
    Drop,
    /// Rename the speaker to `code` with role tag `role`. Utterance
    /// content is byte-stable except for the leading `*OLD:` prefix;
    /// `@Participants` and `@ID` are rewritten per the contract in
    /// `speaker-id.md`. The participant `name` field on the
    /// `@Participants` entry, if any, is preserved from the input,
    /// only the code and role-tag tokens are rewritten.
    Rename {
        /// Replacement speaker code (e.g. `INV`, `CHI`).
        code: SpeakerCode,
        /// Replacement role tag (e.g. `Investigator`, `Target_Child`).
        role: ParticipantRole,
    },
}

/// Operator-supplied mapping from input speaker codes to
/// post-relabeling assignments.
pub type MappingSpec = HashMap<SpeakerCode, SpeakerAssignment>;

/// Parse the comma-separated CLI `--mapping` spec into a typed
/// [`MappingSpec`].
///
/// Grammar (per `speaker-id.md`):
/// - One or more comma-separated assignments.
/// - Each assignment is `OLD=drop` or `OLD=CODE:ROLE`.
/// - Whitespace around tokens is ignored.
///
/// # Examples
///
/// ```rust
/// # use talkbank_transform::speaker_id::{parse_mapping_spec, SpeakerAssignment};
/// # use talkbank_model::{SpeakerCode, ParticipantRole};
/// let parsed = parse_mapping_spec("PAR0=drop,PAR1=INV:Investigator").unwrap();
/// assert_eq!(parsed[&SpeakerCode::new("PAR0")], SpeakerAssignment::Drop);
/// ```
pub fn parse_mapping_spec(spec: &str) -> Result<MappingSpec, SpeakerIdError> {
    let mut out = MappingSpec::new();
    for entry in spec.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            return Err(SpeakerIdError::InvalidMappingSpec(
                "empty assignment (stray comma?)".to_string(),
            ));
        }
        let (old, rhs) = entry.split_once('=').ok_or_else(|| {
            SpeakerIdError::InvalidMappingSpec(format!("expected 'OLD=ASSIGNMENT' near {entry:?}"))
        })?;
        let old_code = SpeakerCode::new(old.trim());
        let rhs = rhs.trim();
        let assignment = if rhs == "drop" {
            SpeakerAssignment::Drop
        } else {
            let (code, role) = rhs.split_once(':').ok_or_else(|| {
                SpeakerIdError::InvalidMappingSpec(format!(
                    "expected 'CODE:ROLE' or 'drop' on the right of '=' near {entry:?}"
                ))
            })?;
            SpeakerAssignment::Rename {
                code: SpeakerCode::new(code.trim()),
                role: ParticipantRole::new(role.trim()),
            }
        };
        out.insert(old_code, assignment);
    }
    Ok(out)
}
