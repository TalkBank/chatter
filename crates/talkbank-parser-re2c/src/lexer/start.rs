//! Public lexer entry points, independent of generated DFA state numbers.

/// A supported starting context for lexing CHAT source.
///
/// Internal states reached halfway through a token are deliberately absent.
/// Use this type or the typed `COND_*` constants when lexing a fragment.
/// ```
/// use talkbank_parser_re2c::{lex_line, lexer::StartCondition, token::Token};
/// let result = lex_line("hello .", StartCondition::MainContent);
/// assert!(result.is_clean());
/// assert!(matches!(result.tokens()[0].0, Token::Word { body: "hello", .. }));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartCondition {
    /// Top-level line classification.
    Initial,
    /// Main-tier and word-tier content.
    MainContent,
    /// Morphological tier content.
    MorContent,
    /// Grammatical relation tier content.
    GraContent,
    /// Phonological tier content.
    PhoContent,
    /// Gestural tier content.
    SinContent,
    /// Generic dependent-tier text.
    TierContent,
    /// Comment text including inline pictures.
    ComContent,
    /// User-defined dependent-tier text.
    UserTierContent,
    /// Generic structured-header text.
    HeaderContent,
    /// Participant ID fields.
    IdContent,
    /// Transcript type fields.
    TypesContent,
    /// Language codes.
    LanguagesContent,
    /// Participant declarations.
    ParticipantsContent,
    /// Media declarations.
    MediaContent,
}

impl StartCondition {
    /// Translate once at lexer construction; only generated code owns later states.
    pub(super) const fn state(self) -> usize {
        match self {
            Self::Initial => super::YYC_INITIAL,
            Self::MainContent => super::YYC_MAIN_CONTENT,
            Self::MorContent => super::YYC_MOR_CONTENT,
            Self::GraContent => super::YYC_GRA_CONTENT,
            Self::PhoContent => super::YYC_PHO_CONTENT,
            Self::SinContent => super::YYC_SIN_CONTENT,
            Self::TierContent => super::YYC_TIER_CONTENT,
            Self::ComContent => super::YYC_COM_CONTENT,
            Self::UserTierContent => super::YYC_USER_TIER_CONTENT,
            Self::HeaderContent => super::YYC_HEADER_CONTENT,
            Self::IdContent => super::YYC_ID_CONTENT,
            Self::TypesContent => super::YYC_TYPES_CONTENT,
            Self::LanguagesContent => super::YYC_LANGUAGES_CONTENT,
            Self::ParticipantsContent => super::YYC_PARTICIPANTS_CONTENT,
            Self::MediaContent => super::YYC_MEDIA_CONTENT,
        }
    }
}
