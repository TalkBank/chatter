//! Core participant model assembled from participant-related headers.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Birth_Header>

use super::super::{ChatDate, IDHeader, ParticipantEntry, ParticipantName, SpeakerCode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Participant record assembled from participant-related headers.
///
/// Combines:
/// - `@Participants` entry (`code`, optional `name`, `role`)
/// - `@ID` payload (language/age/sex/corpus/etc.)
/// - optional `@Birth of <CODE>` date
///
/// # CHAT Format Reference
///
/// - [@Participants](https://talkbank.org/0info/manuals/CHAT.html#Participants_Header)
/// - [@ID](https://talkbank.org/0info/manuals/CHAT.html#ID_Header)
/// - [@Birth](https://talkbank.org/0info/manuals/CHAT.html#Birth_Header)
///
/// A detached snapshot for callers that need owned metadata. A `ChatFile`
/// derives its borrowed `ParticipantView` values from current header lines.
///
/// # Example
///
/// ```rust
/// # use talkbank_model::model::{Participant, ParticipantEntry, IDHeader, SpeakerCode, ParticipantRole, ParticipantName, LanguageCode};
/// let entry = ParticipantEntry::new(SpeakerCode::new("CHI"), Some(ParticipantName::new("Ruth")), ParticipantRole::new("Target_Child"));
///
/// let id = IDHeader::new(LanguageCode::new("eng")?, "CHI", "Target_Child");
///
/// let participant = Participant::new(entry, id);
/// assert_eq!(participant.code.as_str(), "CHI");
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(deny_unknown_fields)]
pub struct Participant {
    /// Speaker code used on main tiers (`*CHI: ...`).
    ///
    /// This code is used in main tier lines to identify the speaker:
    /// ```chat
    /// *CHI:    hello .
    /// ```
    pub code: SpeakerCode,

    /// Observed declaration details; missing roles remain explicit.
    pub details: crate::model::ParticipantDetails,

    /// Full `@ID` metadata payload.
    pub id: IDHeader,

    /// Optional date from `@Birth of <CODE>`.
    pub birth_date: Option<ChatDate>,
}

impl Participant {
    /// Name established by the participant declaration's syntax.
    pub fn name(&self) -> Option<&ParticipantName> {
        self.details.name()
    }

    /// Observed declared role, independent of ID metadata.
    pub fn role(&self) -> Option<&crate::model::ParticipantRole> {
        self.details.role()
    }

    /// Construct a participant from `@Participants` entry plus `@ID`.
    ///
    /// Snapshot constructor. Birth date, if present, is attached
    /// later via [`Self::with_birth_date`].
    ///
    /// # Arguments
    ///
    /// - `entry`: parsed `@Participants` entry
    /// - `id`: parsed `@ID` data for the same speaker code
    ///
    /// # Example
    ///
    /// ```rust
    /// # use talkbank_model::model::{Participant, ParticipantEntry, IDHeader, SpeakerCode, ParticipantRole, ParticipantName, LanguageCode};
    /// let entry = ParticipantEntry::new(SpeakerCode::new("CHI"), Some(ParticipantName::new("Ruth")), ParticipantRole::new("Target_Child"));
    ///
    /// let id = IDHeader::new(LanguageCode::new("eng")?, "CHI", "Target_Child");
    ///
    /// let participant = Participant::new(entry, id);
    /// assert_eq!(participant.code.as_str(), "CHI");
    /// assert_eq!(
    ///     participant.name().map(|n| n.as_str()),
    ///     Some("Ruth")
    /// );
    /// assert_eq!(participant.role().map(|role| role.as_str()), Some("Target_Child"));
    /// assert_eq!(participant.birth_date, None);
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(entry: ParticipantEntry, id: IDHeader) -> Self {
        Self {
            code: entry.speaker_code,
            details: entry.details,
            id,
            birth_date: None,
        }
    }

    /// Attach optional `@Birth of <CODE>` date.
    ///
    /// # Arguments
    ///
    /// - `date`: parsed CHAT date value
    ///
    /// # Example
    ///
    /// ```rust
    /// # use talkbank_model::model::{Participant, ParticipantEntry, IDHeader, SpeakerCode, ParticipantRole, ChatDate, LanguageCode};
    /// # let entry = ParticipantEntry::new(SpeakerCode::new("CHI"), None, ParticipantRole::new("Target_Child"));
    /// # let id = IDHeader::new(LanguageCode::new("eng")?, "CHI", "Target_Child");
    /// let participant = Participant::new(entry, id)
    ///     .with_birth_date(ChatDate::new("28-JUN-2001"));
    ///
    /// assert_eq!(
    ///     participant.birth_date.as_ref().map(|d| d.as_str()),
    ///     Some("28-JUN-2001")
    /// );
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_birth_date(mut self, date: ChatDate) -> Self {
        self.birth_date = Some(date);
        self
    }
}
