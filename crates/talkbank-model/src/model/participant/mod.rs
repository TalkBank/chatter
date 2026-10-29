//! Participant model combining `@Participants`, `@ID`, and optional `@Birth`.
//!
//! In CHAT, participant metadata is split across multiple header families:
//! - `@Participants`: code, optional name, role
//! - `@ID`: language/corpus/age/sex plus optional study metadata
//! - `@Birth of <CODE>`: optional birth date
//!
//! This module exposes a borrowed [`ParticipantView`] of current headers and
//! an explicitly owned [`Participant`] snapshot so downstream code does not
//! need to re-join those headers manually. A snapshot no longer tracks edits
//! to the source document; serializing it does not certify header validity.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Birth_Header>
//!
//! # CHAT Requirement
//!
//! Every participant listed in `@Participants` must have a corresponding `@ID`.
//! Validation diagnoses missing or inconsistent metadata. Readable participant
//! views can still contain invalid field values, and a declared speaker without
//! a readable matching ID has no joined view. Use the participant roster when
//! the question is declaration rather than availability of joined metadata.
//!
//! # Example CHAT Structure
//!
//! ```chat
//! @Participants:    CHI Ruth Target_Child, INV Chiat Investigator
//! @ID:    eng|chiat|CHI|10;03.||||Target_Child|||
//! @ID:    eng|chiat|INV|||||Investigator|||
//! @Birth of CHI:    28-JUN-2001
//! ```
//!
//! This produces two `Participant` values with merged metadata.
//!
//! # Example Usage
//!
//! ```rust
//! use talkbank_model::model::{
//!     Participant,
//!     ParticipantEntry,
//!     IDHeader,
//!     SpeakerCode,
//!     ParticipantRole,
//!     ParticipantName,
//!     ChatDate,
//!     LanguageCode,
//! };
//!
//! let entry = ParticipantEntry::new(
//!     SpeakerCode::new("CHI"),
//!     Some(ParticipantName::new("Ruth")),
//!     ParticipantRole::new("Target_Child"),
//! );
//!
//! let id = IDHeader::new(LanguageCode::new("eng")?, "CHI", "Target_Child")
//!     .with_age("10;03.")
//!     .with_corpus("chiat");
//!
//! let participant = Participant::new(entry, id)
//!     .with_birth_date(ChatDate::new("28-JUN-2001"));
//!
//! assert_eq!(participant.code.as_str(), "CHI");
//! assert_eq!(
//!     participant.name().map(|n| n.as_str()),
//!     Some("Ruth")
//! );
//! assert_eq!(
//!     participant.birth_date.as_ref().map(|d| d.as_str()),
//!     Some("28-JUN-2001")
//! );
//! # Ok::<_, Box<dyn std::error::Error>>(())
//! ```

mod accessors;
mod core;
mod declared;
pub mod join;
mod roster;
#[cfg(test)]
mod tests;

pub use core::Participant;
pub use declared::DeclaredSpeaker;
pub use join::ParticipantView;

pub(crate) use roster::participant_declaration;
pub use roster::{ParticipantDeclarationError, ParticipantRoster, SpeakerMembership};
