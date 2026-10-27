//! Admission of replacement identities before any speaker-mapping output.

use std::collections::{HashMap, hash_map::Entry};

use crate::speaker_headers::SpeakerRewriteInput;
use talkbank_model::{ParticipantName, ParticipantRole, SpeakerCode};

use crate::speaker_token::{SpeakerToken, SpeakerTokenError};

use super::mapping::{MappingSpec, SpeakerAssignment};

/// Why an operator-supplied replacement cannot be inserted into CHAT.
#[derive(Debug, thiserror::Error)]
pub enum MappingAssignmentError {
    /// Two retained participant entries would declare the same output identity.
    #[error("speaker mapping would assign both {first} and {second} to {target}")]
    TargetCollision {
        /// Earlier source entry in declaration order.
        first: SpeakerCode,
        /// Later source entry in declaration order.
        second: SpeakerCode,
        /// Conflicting final identity, including unchanged participants.
        target: SpeakerCode,
    },
    /// The replacement code cannot be used at every speaker-reference site.
    #[error("mapping for {speaker}: {source}")]
    Speaker {
        /// Source identity whose assignment was refused.
        speaker: SpeakerCode,
        /// Shared speaker-token admission failure.
        source: SpeakerTokenError,
    },
    /// The replacement role is outside the shared CHAT role vocabulary.
    #[error("mapping for {speaker}: invalid participant role {role:?}")]
    Role {
        /// Source identity whose assignment was refused.
        speaker: SpeakerCode,
        /// Rejected role, retained without normalization.
        role: ParticipantRole,
    },
    /// The replacement name is empty or contains a header/list delimiter.
    #[error(
        "mapping for {speaker}: participant name must be nonblank and contain no comma or line break: {name:?}"
    )]
    Name {
        /// Source identity whose assignment was refused.
        speaker: SpeakerCode,
        /// Rejected name, retained without normalization.
        name: ParticipantName,
    },
}

/// A mapping bound to the input declaration whose final targets were admitted.
///
/// Both immutable borrows survive through rendering: an accepted mapping cannot
/// be paired with another input or changed after collision assessment. This does
/// not certify source coverage or general validity of the input file.
pub(super) struct AdmittedMapping<'a> {
    input: SpeakerRewriteInput<'a>,
    mapping: &'a MappingSpec,
}

impl<'a> AdmittedMapping<'a> {
    pub(super) fn admit(
        input: SpeakerRewriteInput<'a>,
        mapping: &'a MappingSpec,
    ) -> Result<Self, MappingAssignmentError> {
        for (speaker, assignment) in mapping {
            match assignment {
                SpeakerAssignment::Drop => {}
                SpeakerAssignment::Rename {
                    code,
                    role,
                    specific_role,
                } => {
                    SpeakerToken::admit(code.clone()).map_err(|source| {
                        MappingAssignmentError::Speaker {
                            speaker: speaker.clone(),
                            source,
                        }
                    })?;
                    if !talkbank_model::validation::is_allowed_participant_role(role.as_str()) {
                        return Err(MappingAssignmentError::Role {
                            speaker: speaker.clone(),
                            role: role.clone(),
                        });
                    }
                    if let Some(name) = specific_role
                        && (name.as_str().trim().is_empty()
                            || name.as_str().contains([',', '\r', '\n']))
                    {
                        return Err(MappingAssignmentError::Name {
                            speaker: speaker.clone(),
                            name: name.clone(),
                        });
                    }
                }
            }
        }
        // Judge final declaration occupancy simultaneously. Swaps and reuse of
        // a dropped identity are legal; an unchanged participant still occupies
        // its target. Entries absent from the source create no declaration.
        let mut targets = HashMap::new();
        for participant in input.participants().iter() {
            let source = &participant.speaker_code;
            let target = match mapping.get(source) {
                Some(SpeakerAssignment::Drop) => continue,
                Some(SpeakerAssignment::Rename { code, .. }) => code,
                None => source,
            };
            match targets.entry(target) {
                Entry::Vacant(slot) => {
                    slot.insert(source);
                }
                Entry::Occupied(slot) => {
                    return Err(MappingAssignmentError::TargetCollision {
                        first: (*slot.get()).clone(),
                        second: source.clone(),
                        target: target.clone(),
                    });
                }
            }
        }
        Ok(Self { input, mapping })
    }

    pub(super) fn file(&self) -> &'a talkbank_model::ChatFile {
        self.input.file()
    }

    pub(super) fn get(&self, speaker: &SpeakerCode) -> Option<&SpeakerAssignment> {
        self.mapping.get(speaker)
    }
}
