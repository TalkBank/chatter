//! One speaker as DECLARED in `@Participants`, optionally enriched from `@ID`.
//!
//! This exists because the two headers answer different questions and only one
//! of them is authoritative about who is in the transcript.
//!
//! `@Participants` is the DECLARATION: it fixes the roster and its order.
//! `@ID` is METADATA about a declared speaker: age, sex, group, SES, education.
//! Valid CHAT carries an `@ID` for every declared speaker, so in a clean file
//! the two agree exactly, and [`ChatFile::participants`](crate::model::ChatFile::participants)
//! (which is keyed and populated from the `@ID` join) is a complete roster.
//!
//! In a file that is NOT clean, they diverge, and the map loses. A speaker
//! declared in `@Participants` with no `@ID` raises E522 and is then absent
//! from the map, so a consumer iterating the map sees a transcript with fewer
//! speakers than it declares, with nothing in the value it holds to say so.
//! Reporting an error and dropping the fact are two different things, and the
//! model was doing both.

use super::Participant;
use crate::model::{ParticipantEntry, ParticipantName, ParticipantRole, SpeakerCode};

/// A speaker declared in `@Participants`, in declaration order, together with
/// its `@ID` metadata when that header exists.
///
/// Yielded by [`ChatFile::declared_speakers`](crate::model::ChatFile::declared_speakers).
///
/// Code, name and role come from the DECLARATION, which is the header that
/// establishes them. Everything else that CHAT knows about a speaker lives on
/// the `@ID` header and is reached through [`id_metadata`](Self::id_metadata).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeclaredSpeaker<'a> {
    entry: &'a ParticipantEntry,
    metadata: Option<&'a Participant>,
}

impl<'a> DeclaredSpeaker<'a> {
    /// Pairs a declaration with the `@ID`-derived metadata found for it.
    pub(crate) fn new(entry: &'a ParticipantEntry, metadata: Option<&'a Participant>) -> Self {
        Self { entry, metadata }
    }

    /// The speaker code as declared, e.g. `CHI`.
    pub fn code(&self) -> &'a SpeakerCode {
        &self.entry.speaker_code
    }

    /// The declared name, when the entry carries one.
    pub fn name(&self) -> Option<&'a ParticipantName> {
        self.entry.name.as_ref()
    }

    /// The declared role, e.g. `Target_Child`.
    pub fn role(&self) -> &'a ParticipantRole {
        &self.entry.role
    }

    /// The `@ID`-derived metadata for this speaker: age, sex, group, SES,
    /// education, and the birth date when `@Birth of <CODE>` is present.
    ///
    /// `None` means no `@ID` header names this speaker. That is invalid CHAT
    /// (E522) rather than an ordinary absence, so a consumer that needs the
    /// metadata should treat it as a defect in the file, not as a speaker with
    /// unknown demographics.
    pub fn id_metadata(&self) -> Option<&'a Participant> {
        self.metadata
    }
}
