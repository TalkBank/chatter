//! Convenience APIs and trait implementations for `Header`.
//!
//! These helpers map a header to its kind (`kind.rs`, the owner of every
//! label) so serializers, diagnostics, and external consumers read a single
//! source of truth for canonical names, and delegate validation.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Line>

use super::header::Header;
use super::kind::{
    FixedKind, GemKind, HeaderKind, PlainKind, QualifiedKind, StructuredKind, TextKind,
};
use crate::model::WriteChat;

impl Header {
    /// Which header this is: the one map from a model header to its kind,
    /// and so to its label ([`HeaderKind::label`]). A header written with
    /// nothing after its colon has the kind of the header it would have
    /// been; a recognized declaration that could not be admitted has the
    /// kind of the declaration.
    pub fn kind(&self) -> HeaderKind {
        use HeaderKind as K;
        let text = |kind| K::Plain(PlainKind::Text(kind));
        let gem = |kind| K::Plain(PlainKind::Gem(kind));
        let structured = |kind| K::Plain(PlainKind::Structured(kind));
        match self {
            Header::Utf8 => K::Fixed(FixedKind::Utf8),
            Header::Begin => K::Fixed(FixedKind::Begin),
            Header::End => K::Fixed(FixedKind::End),
            Header::Languages { .. } | Header::UnparsedLanguages(_) => {
                K::Fixed(FixedKind::Languages)
            }
            Header::Participants { .. } | Header::UnparsedParticipants(_) => {
                K::Fixed(FixedKind::Participants)
            }
            Header::ID(_) | Header::UnparsedID(_) => K::Fixed(FixedKind::Id),
            Header::NewEpisode => K::Fixed(FixedKind::NewEpisode),
            Header::Blank => K::Fixed(FixedKind::Blank),
            Header::Unknown { .. } => K::Unknown,
            Header::EmptyContent(empty) => empty.kind(),
            Header::Date { .. } => text(TextKind::Date),
            Header::Pid { .. } => text(TextKind::Pid),
            Header::Situation { .. } => text(TextKind::Situation),
            Header::TapeLocation { .. } => text(TextKind::TapeLocation),
            Header::TimeDuration { .. } => text(TextKind::TimeDuration),
            Header::TimeStart { .. } => text(TextKind::TimeStart),
            Header::Location { .. } => text(TextKind::Location),
            Header::RoomLayout { .. } => text(TextKind::RoomLayout),
            Header::Transcriber { .. } => text(TextKind::Transcriber),
            Header::Warning { .. } => text(TextKind::Warning),
            Header::Activities { .. } => text(TextKind::Activities),
            Header::Bck { .. } => text(TextKind::Bck),
            Header::Videos { .. } => text(TextKind::Videos),
            Header::T { .. } => text(TextKind::T),
            Header::Font { .. } => text(TextKind::Font),
            Header::Window { .. } => text(TextKind::Window),
            Header::ColorWords { .. } => text(TextKind::ColorWords),
            Header::Number { .. } => text(TextKind::Number),
            Header::RecordingQuality { .. } => text(TextKind::RecordingQuality),
            Header::Transcription { .. } => text(TextKind::Transcription),
            Header::BeginGem { .. } => gem(GemKind::Begin),
            Header::EndGem { .. } => gem(GemKind::End),
            Header::LazyGem { .. } => gem(GemKind::Lazy),
            Header::Comment { .. } => structured(StructuredKind::Comment),
            Header::Options { .. } => structured(StructuredKind::Options),
            Header::Media(_) => structured(StructuredKind::Media),
            Header::Types(_) => structured(StructuredKind::Types),
            Header::Page { .. } => structured(StructuredKind::Page),
            Header::Birth { .. } => K::Qualified(QualifiedKind::Birth),
            Header::Birthplace { .. } => K::Qualified(QualifiedKind::Birthplace),
            Header::L1Of { .. } => K::Qualified(QualifiedKind::L1Of),
        }
    }

    /// Returns canonical header label text without the leading `@`: the
    /// label of [`Self::kind`], so serializers, diagnostics and UI metadata
    /// views read the one label table.
    pub fn name(&self) -> &'static str {
        self.kind().label()
    }

    /// Serializes this header to an owned CHAT line string.
    ///
    /// Prefer [`WriteChat`] directly in hot paths to avoid temporary allocation.
    /// This helper exists so callers that only need display text can skip
    /// passing a writer buffer altogether.
    pub fn to_chat(&self) -> String {
        crate::model::ChatString::render(|w| self.write_chat(w))
    }
}

impl std::fmt::Display for Header {
    /// Formats this header in canonical CHAT line form.
    ///
    /// This delegates to [`WriteChat`] so display output stays identical to file serialization.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

impl crate::validation::Validate for Header {
    /// Validates this header using shared header validation logic.
    ///
    /// `Header` values do not carry their own span, so callers that need
    /// source-accurate diagnostics should validate at line/chat-file level.
    fn validate(
        &self,
        context: &crate::validation::ValidationContext,
        errors: &impl crate::ErrorSink,
    ) {
        crate::validation::header::check_header(self, crate::Location::Unlocated, context, errors);
    }
}
