//! Declaration evidence, independent of the successful participant metadata join.

use std::collections::HashSet;

use crate::model::{Header, SpeakerCode};

/// Why current headers cannot establish one participant declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParticipantDeclarationError {
    /// No declaration is present.
    #[error("@Participants declaration is absent")]
    Absent,
    /// Observed syntax was rejected.
    #[error("@Participants declaration is unparsed")]
    Rejected,
    /// More than one declaration competes for authority.
    #[error("@Participants declaration is repeated")]
    Repeated,
}

/// One parsed declaration and the context supplied with that same header.
/// Only the cardinality admission below can construct this evidence.
pub(super) struct AdmittedParticipantDeclaration<'a, Context> {
    entries: &'a crate::model::ParticipantEntries,
    context: Context,
}

impl<'a, Context> AdmittedParticipantDeclaration<'a, Context> {
    /// Consume admission while preserving its header's associated context.
    pub(super) fn into_parts(self) -> (&'a crate::model::ParticipantEntries, Context) {
        (self.entries, self.context)
    }
}

/// Admit cardinality and recovery once without discarding caller-owned context.
pub(super) fn admit_participant_declaration<'a, Context>(
    headers: impl IntoIterator<Item = (&'a Header, Context)>,
) -> Result<AdmittedParticipantDeclaration<'a, Context>, ParticipantDeclarationError> {
    let mut declarations = headers
        .into_iter()
        .filter_map(|(header, context)| match header {
            Header::Participants { entries } => {
                Some(Ok(AdmittedParticipantDeclaration { entries, context }))
            }
            Header::UnparsedParticipants(_) => Some(Err(ParticipantDeclarationError::Rejected)),
            _ => None,
        });
    match (declarations.next(), declarations.next()) {
        (None, _) => Err(ParticipantDeclarationError::Absent),
        (Some(_), Some(_)) => Err(ParticipantDeclarationError::Repeated),
        (Some(declaration), None) => declaration,
    }
}

pub(crate) fn participant_declaration<'a>(
    headers: impl IntoIterator<Item = &'a Header>,
) -> Result<&'a crate::model::ParticipantEntries, ParticipantDeclarationError> {
    admit_participant_declaration(headers.into_iter().map(|header| (header, ())))
        .map(|declaration| declaration.into_parts().0)
}

/// What the observed declaration establishes about a speaker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeakerMembership {
    /// The declaration names this speaker.
    Declared,
    /// The available declaration evidence establishes that this speaker is absent.
    Undeclared,
    /// No roster was supplied, or competing declarations prevent a conclusion.
    Indeterminate,
}

#[derive(Clone, Debug)]
enum RosterEvidence {
    Unscoped,
    Absent,
    Empty,
    Declared(HashSet<SpeakerCode>),
    Repeated,
    Rejected,
}

/// Speaker declaration evidence for validation, never inferred from ID metadata.
///
/// An isolated fragment has no roster scope. A full file distinguishes a missing
/// declaration, an empty declaration and repeated declarations. Neither missing
/// metadata nor a failed metadata join removes a declared speaker.
#[derive(Clone, Debug)]
pub struct ParticipantRoster(RosterEvidence);

impl ParticipantRoster {
    /// A caller has supplied no file-level declaration scope.
    pub fn unscoped() -> Self {
        Self(RosterEvidence::Unscoped)
    }

    /// Explicitly supplied declaration codes, including an explicitly empty list.
    pub fn from_codes(codes: HashSet<SpeakerCode>) -> Self {
        Self(if codes.is_empty() {
            RosterEvidence::Empty
        } else {
            RosterEvidence::Declared(codes)
        })
    }

    /// Read current headers once; repeated declarations never select a winner.
    pub fn from_headers<'a>(headers: impl IntoIterator<Item = &'a Header>) -> Self {
        match participant_declaration(headers) {
            Err(ParticipantDeclarationError::Absent) => Self(RosterEvidence::Absent),
            Err(ParticipantDeclarationError::Rejected) => Self(RosterEvidence::Rejected),
            Err(ParticipantDeclarationError::Repeated) => Self(RosterEvidence::Repeated),
            Ok(entries) => Self::from_codes(
                entries
                    .iter()
                    .map(|entry| entry.speaker_code.clone())
                    .collect(),
            ),
        }
    }

    /// Query declaration membership without consulting role or ID completeness.
    pub fn membership(&self, code: &SpeakerCode) -> SpeakerMembership {
        match &self.0 {
            RosterEvidence::Unscoped | RosterEvidence::Repeated | RosterEvidence::Rejected => {
                SpeakerMembership::Indeterminate
            }
            RosterEvidence::Absent | RosterEvidence::Empty => SpeakerMembership::Undeclared,
            RosterEvidence::Declared(codes) => {
                if codes.contains(code) {
                    SpeakerMembership::Declared
                } else {
                    SpeakerMembership::Undeclared
                }
            }
        }
    }

    /// A nonempty, unambiguous set for diagnostics that list expected speakers.
    /// Missing, empty, repeated and unscoped declarations cannot supply this set.
    pub fn declared_codes(&self) -> Option<&HashSet<SpeakerCode>> {
        match &self.0 {
            RosterEvidence::Declared(codes) => Some(codes),
            RosterEvidence::Unscoped
            | RosterEvidence::Absent
            | RosterEvidence::Empty
            | RosterEvidence::Repeated
            | RosterEvidence::Rejected => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ParticipantEntries;

    #[test]
    fn unscoped_absent_empty_and_repeated_are_distinct_evidence() {
        let code = SpeakerCode::new("CHI");
        assert_eq!(
            ParticipantRoster::unscoped().membership(&code),
            SpeakerMembership::Indeterminate
        );
        assert_eq!(
            ParticipantRoster::from_headers([]).membership(&code),
            SpeakerMembership::Undeclared
        );
        let empty = Header::Participants {
            entries: ParticipantEntries::new(Vec::new()),
        };
        assert_eq!(
            ParticipantRoster::from_headers([&empty]).membership(&code),
            SpeakerMembership::Undeclared
        );
        assert_eq!(
            ParticipantRoster::from_headers([&empty, &empty]).membership(&code),
            SpeakerMembership::Indeterminate
        );
    }

    #[test]
    fn supplied_declaration_codes_establish_membership_without_metadata() {
        let code = SpeakerCode::new("CHI");
        let roster = ParticipantRoster::from_codes([code.clone()].into_iter().collect());
        assert_eq!(roster.membership(&code), SpeakerMembership::Declared);
        assert_eq!(
            roster.membership(&SpeakerCode::new("MOT")),
            SpeakerMembership::Undeclared
        );
    }
}
