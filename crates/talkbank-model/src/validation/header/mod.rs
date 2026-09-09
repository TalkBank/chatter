//! Header-validation orchestration and rule entrypoints.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Date_Header>
//!
//! The public submodules expose focused rule families (structure, participant,
//! metadata), while `check_header` provides the single dispatch point used by
//! line/file validators.

pub mod metadata;
pub mod participant;
pub mod structure;

mod checkers;
mod unknown;
mod validate;

pub(crate) use crate::validation::check_speaker_id;
pub(crate) use validate::check_header;

#[cfg(test)]
mod tests {
    //! Two checks over a `SpeakerCode` no parse produces.
    //!
    //! Nine tests that used to sit here built a `Header` value by hand
    //! (`Header::Options { .. }` for the text `@Options:\tCA, NewThing`) and
    //! called `check_header` on it at `Span::DUMMY`. They are
    //! `talkbank-parser-tests/tests/integration/header_values_from_source.rs`
    //! now, over header lines the parser built. These two stay because the
    //! value they test, a speaker code containing `:`, is one the parser never
    //! builds: `@Participants:\tCH:I Child` is E320 and E316 at parse, so the
    //! invalid-character check is reachable only from a value built by hand,
    //! and building it is the only way to reach the check at all.
    use super::check_header;
    use crate::model::{Header, ParticipantEntry, ParticipantRole, SpeakerCode};
    use crate::validation::ValidationContext;
    use crate::{ErrorCollector, Span};

    /// Speaker IDs containing `:` are rejected as invalid.
    ///
    /// Colon is reserved by CHAT for tier-prefix syntax, so allowing it would make
    /// speaker parsing ambiguous.
    #[test]
    fn test_speaker_id_with_colon_invalid() {
        let entry = ParticipantEntry {
            speaker_code: SpeakerCode::new("CH:I"),
            name: None,
            role: ParticipantRole::new("Child"),
        };

        let header = Header::Participants {
            entries: vec![entry].into(),
        };

        let errors = ErrorCollector::new();
        let ctx = ValidationContext::default();
        check_header(&header, Span::DUMMY, &ctx, &errors);
        let error_vec = errors.into_vec();

        // Should catch the ':' character (reserved as delimiter)
        assert!(
            !error_vec.is_empty(),
            "Should have errors for speaker ID with ':'"
        );
        assert!(
            error_vec
                .iter()
                .any(|e| e.message.contains("invalid character") && e.message.contains("colon"))
        );
    }

    /// Standard uppercase speaker IDs pass character validation.
    ///
    /// The assertion specifically checks that no invalid-character diagnostics are emitted.
    #[test]
    fn test_speaker_id_valid() {
        let entry = ParticipantEntry {
            speaker_code: SpeakerCode::new("CHI"),
            name: None,
            role: ParticipantRole::new("Child"),
        };

        let header = Header::Participants {
            entries: vec![entry].into(),
        };

        let errors = ErrorCollector::new();
        let ctx = ValidationContext::default();
        check_header(&header, Span::DUMMY, &ctx, &errors);
        let error_vec = errors.into_vec();

        // Should not have speaker ID character errors (might have other errors though)
        assert!(
            !error_vec
                .iter()
                .any(|e| e.message.contains("invalid character"))
        );
    }
}
