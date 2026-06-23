//! Convenience accessors over participant header-derived fields.
//!
//! These methods keep `Participant` usages shallow by exposing the most common
//! header-derived fields without forcing external code to dig through the
//! `IdHeader` structure.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>

use super::Participant;

impl Participant {
    /// Returns canonical speaker code (`CHI`, `MOT`, ...).
    ///
    /// This is the stable key used for cross-referencing utterances and headers.
    pub fn speaker_code(&self) -> &str {
        self.code.as_str()
    }

    /// Returns `true` when a birth date from `@Birth of` is attached.
    ///
    /// This is a convenience predicate for callers that branch on optional age-era metadata.
    pub fn has_birth_date(&self) -> bool {
        self.birth_date.is_some()
    }

    /// Returns the optional `@ID` age field.
    ///
    /// The value is preserved verbatim (for example `2;6.0`) for downstream tooling.
    pub fn age(&self) -> Option<&str> {
        self.id.age.as_ref().map(|a| a.as_str())
    }

    /// Returns the optional `@ID` sex field.
    ///
    /// Values are preserved as parsed and may be absent in corpora with partial metadata.
    pub fn sex(&self) -> Option<&super::super::Sex> {
        self.id.sex.as_ref()
    }

    /// Returns the participant's `@ID` language code.
    ///
    /// This is not necessarily the transcript default language.
    pub fn languages(&self) -> &crate::model::LanguageCodes {
        &self.id.language
    }

    /// Returns the `@ID` corpus field if it is non-empty.
    ///
    /// The corpus field is required (validated as E514 when empty), but the
    /// model can still hold an empty value during lenient recovery; this
    /// accessor reports `None` for an empty corpus, so dataset-level routing and
    /// corpus-specific policy toggles see a present-or-absent corpus name.
    pub fn corpus(&self) -> Option<&str> {
        let corpus = self.id.corpus.as_str();
        (!corpus.is_empty()).then_some(corpus)
    }
}
