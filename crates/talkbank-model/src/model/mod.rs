//! Canonical domain model for CHAT format
//!
//! This module defines the data structures representing parsed CHAT transcripts.
//!
//! # Module Organization
//!
//! - `file/` - Document-level structures (ChatFile, Line, Utterance)
//! - `header/` - @ header lines (Header, IDHeader, MediaHeader)
//! - `content/` - Main tier content (Word, Pause, Event, Action, etc.)
//! - `annotation/` - Square bracket annotations `[*]`, `[: word]`, etc.
//! - `dependent_tier/` - % dependent tiers (MOR, GRA, PHO, etc.)
//!
//! CHAT references:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! # Design Principles
//!
//! - **Type-safe**: Use enums and structs to encode CHAT semantics
//! - **Pure data**: Model contains only data, validation is separate
//! - **Serializable**: All types are JSON-serializable via serde
//! - **Composable**: Granular types that can be parsed independently
//!
//! # Data Contract Rules
//!
//! - Avoid dummy/sentinel values in data structures; unknown states should be explicit in types
//! - Prefer explicit domain enums over `Option` when `None` would carry ambiguous meaning
//! - Use explicit state enums for runtime metadata (e.g., `Uncomputed` vs `Computed`)
//! - Keep parse-origin metadata (`parse_health`, language metadata) in the model for downstream tooling
//! - Any new field added for parser recovery must have clear semantics and tests

// Core trait for CHAT serialization
mod chat_writer;
pub use chat_writer::{ChatString, ChatWriter};

mod write_chat;
pub use write_chat::WriteChat;

// Semantic equality trait for roundtrip testing
mod semantic_eq;
pub use semantic_eq::SemanticEq;
mod semantic_diff;
pub use semantic_diff::{
    DEFAULT_MAX_DIFFS, PathSegment, SemanticDiff, SemanticDiffContext, SemanticDiffKind,
    SemanticDiffReport, SemanticDifference, SemanticPath,
};
mod validation_tag;
pub use validation_tag::{ValidationTag, ValidationTagged};

// Shared macros used across model modules
mod macros;

// Organized submodules
pub mod annotation;
pub mod content;
pub mod dependent_tier;
pub mod file;
pub mod header;
// Standalone modules
mod alignment_set;
mod intern;
mod language_metadata;
mod non_empty_string;
pub mod participant;
mod provenance;
mod time;

// Re-export RecoveryText
pub use non_empty_string::{EmptyText, NonEmptyLiteral, NonEmptyStr, NonEmptyString, RecoveryText};
pub use time::MediaTiming;

// Re-export provenance types
pub use provenance::{
    AsrWordsJson, LanguageId, Morphosyntax, NlpResponse, NlpResponseJson, NlpTokens, Provenance,
    RawChatText, TierDomainMarker, TokenizedWords, TranscriptJson,
};

// Re-export interning functions
pub use intern::{
    language_interner, participant_interner, pos_interner, speaker_interner, stem_interner,
};

// Re-export file types
pub use file::{
    ChatFile, ChatFileLines, DeclaredLanguages, DeclaredLanguagesRef, FileStem, FileStemError,
    LanguageDeclarationError, Line, OwnedFileStem, OwnedTranscriptName, ParseHealth,
    ParseHealthState, ParseHealthTier, RecordedTranscriptTiming, TranscriptName,
    TranscriptTimingEvidence, Utterance, UtteranceLanguage, UtteranceLanguageMetadata,
};

// Re-export participant type
pub use participant::{
    DeclaredSpeaker, Participant, ParticipantDeclarationError, ParticipantRoster, ParticipantView,
    SpeakerMembership,
};

// Re-export header types
pub use header::{
    ActivitiesDescription,
    ActivityType,
    AgeComponents,
    AgeValue,
    BackgroundDescription,
    BirthplaceDescription,
    CaOptionEffect,
    ChatDate,
    ChatOptionFlag,
    ChatOptionFlags,
    CheckedChatDate,
    ColorWordList,
    CorpusName,
    CustomIdField,
    DesignType,
    EducationDescription,
    Ethnicity,
    FontSpec,
    GemEndFailure,
    GemEvent,
    GemLabel,
    GemPair,
    GemState,
    GroupName,
    GroupType,
    Header,
    IDHeader,
    InvalidPageNumber,
    InvalidTypesField,
    LanguageCode,
    LanguageCodeError,
    LanguageCodes,
    LanguageHeaderFailure,
    LanguageName,
    LocationDescription,
    MediaFilename,
    MediaFilenameError,
    MediaFilenameProblem,
    MediaHeader,
    MediaStatus,
    MediaType,
    Month,
    MultipleLanguageCodes,
    Number,
    PageNumber,
    ParticipantDetails,
    ParticipantEntries,
    ParticipantEntry,
    ParticipantName,
    ParticipantRole,
    PidValue,
    RecordingQuality,
    RegisteredLanguageCode,
    RoomLayoutDescription,
    SesCode,
    SesValue,
    Sex,
    SituationDescription,
    SpeakerCode,
    TDescription,
    TapeLocationDescription,
    TimeDurationValue,
    TimeSegment,
    TimeStartValue,
    TimeValue,
    TooFewLanguageCodes,
    TranscriberName,
    Transcription,
    // @Types header components
    TypesHeader,
    UnclosedGem,
    UnparsedIDHeader,
    UnparsedLanguageHeader,
    UnparsedParticipantHeader,
    VideoSpec,
    WarningText,
    WindowGeometry,
};

// Re-export content types
// Crate-internal: see `content::structure` for why this is not public API.
pub use content::{
    Action,
    BracketedContent,
    BracketedItem,
    BracketedItems,
    Bullet,
    BulletSource,
    BulletState,
    CADelimiter,
    CADelimiterType,
    CAElement,
    CAElementType,
    ConflictingWordSuffixes,
    Event,
    EventType,
    FormLabel,
    FormMarkerPayload,
    FormType,
    Freecode,
    Group,
    InvalidEventType,
    InvalidFormLabel,
    InvalidScopedLabel,
    InvalidTimedPauseSpelling,
    InvalidWordPosTag,
    LanguagePrecode,
    Linker,
    LinkerKind,
    LongFeatureBegin,
    LongFeatureEnd,
    // Labels and markers
    LongFeatureLabel,
    // Main tier structure
    MainTier,
    MainTierLocations,
    NonvocalBegin,
    NonvocalEnd,
    NonvocalLabel,
    NonvocalSimple,
    OrphanAnnotation,
    OtherSpokenEvent,
    OverlapIndex,
    OverlapPoint,
    OverlapPointKind,
    // Content items
    ParsedPauseDuration,
    Pause,
    PauseDuration,
    PauseTimedDuration,
    PhoGroup,
    Postcode,
    Quotation,
    Retrace,
    RetraceKind,
    Separator,
    SinGroup,
    Terminator,
    TierContent,
    TierContentItems,
    TierSeparator,
    TimedPause,
    UnderlineMarker,
    UnparsedBullet,
    UnspannedSwitchTarget,
    UtteranceContent,
    // Word types
    Word,
    WordCategory,
    WordCliticBoundary,
    WordCompoundMarker,
    WordContent,
    WordContents,
    WordLanguageMarker,
    WordLengthening,
    WordMaterial,
    WordPhonetic,
    WordPosTag,
    WordShortening,
    WordStressMarker,
    WordStressMarkerType,
    WordSuffix,
    WordSyllablePause,
    WordText,
    WordUnderlineBegin,
    WordUnderlineEnd,
};
pub use content::{
    AngleAnnotations, AngleRef, ContainerMut, ContentStructure, Descend, GroupKind, GroupMut,
    GroupRef, LeafContent, LeafRef, QuotationRef, RetraceRef, WordRef,
};

// Re-export annotation types
// `AnnotatedContentAnnotations` rides with `Annotated` deliberately: it is the
// argument type of the public `Annotated::new`, so a caller who can name the
// wrapper must be able to name the proof its constructor demands. It was
// omitted when the non-empty invariant landed, which left the only route to a
// legal `Annotated` running through a module path nothing else here uses.
pub use annotation::{
    Annotated, AnnotatedContentAnnotations, CodeSwitchSpan, ContentAnnotation,
    InvalidOverlapMarkerIndex, NoScopedAnnotations, OverlapMarkerIndex, ReplacedWord, Replacement,
    ScopedAlternative, ScopedError, ScopedExplanation, ScopedOverlapBegin, ScopedOverlapEnd,
    ScopedParalinguistic, ScopedPercentComment, ScopedUnknown,
};

// Re-export dependent tier types
pub use dependent_tier::{
    // ACT/COD
    ActTier,
    // Text tiers
    AddTier,
    // Bullet content
    BulletContent,
    BulletContentBullet,
    BulletContentPicture,
    BulletContentSegment,
    BulletContentText,
    CodTier,
    ComTier,
    // Dependent tier enum
    DependentTier,
    // Dependent-tier line wrapper (tier + separator)
    DependentTierEntry,
    DependentTierKey,
    ExpTier,
    GpxTier,
    GraLowering,
    GraTier,
    GraTierType,
    // GRA
    GrammaticalRelation,
    GrammaticalRelationType,
    IntTier,
    InvalidGrammaticalRelationType,
    InvalidUserDefinedDependentTierLabel,
    // MOR
    Mor,
    MorAnalysis,
    MorChunk,
    MorChunkKind,
    MorFeature,
    MorFeatureShapeError,
    MorStem,
    MorTier,
    MorTierType,
    MorWord,
    PhoItem,
    PhoTier,
    PhoTierType,
    // PHO
    PhoWord,
    // Public fallible-parse error types (every public constructor's
    // error type must be nameable downstream; see BUG-3 2026-07-08)
    PhoalnParseError,
    PosCategory,
    RecordedWorTiming,
    // SIN
    SinGroupGestures,
    SinItem,
    SinTier,
    SinToken,
    SitTier,
    SpaTier,
    // Public fallible-parse error type (classify_syl_word /
    // tokenize_syl_word); nameable downstream per BUG-3, same as its
    // PhoalnParseError / XphointParseError siblings above.
    SylWordError,
    TextTier,
    UnparsedTextTier,
    UnparsedTier,
    UnsupportedDependentTier,
    UserDefinedDependentTier,
    UserDefinedDependentTierLabel,
    WorTier,
    WorTimingEvidence,
    XphointParseError,
};

// Re-export alignment metadata
pub use alignment_set::{AlignmentSet, AlignmentUnit, AlignmentUnits};

// Re-export language metadata
pub use language_metadata::{
    LanguageMetadata,
    LanguageSource,
    WordLanguageInfo,
    WordLanguageInfos,
    // `WordLanguages` is the type of a public field
    // (`WordLanguageInfo::languages`), so it was public by leak and unnameable;
    // a parse-backed test comparing that field needs the name.
    WordLanguages,
};

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_derive::ValidationTagged;

    /// Demo enum used to validate `ValidationTagged` defaults/overrides.
    #[derive(ValidationTagged)]
    enum DemoValidationState {
        Clean,
        ParseError,
        DeferredWarning,
        #[validation_tag(error)]
        ExplicitProblem,
    }

    /// `ValidationTagged` derives convention-based tags with explicit overrides.
    ///
    /// The enum fixture covers clean, warning, inferred error, and annotated error cases.
    #[test]
    fn test_validation_tagged_convention_and_annotation() {
        assert_eq!(
            DemoValidationState::Clean.validation_tag(),
            ValidationTag::Clean
        );
        assert_eq!(
            DemoValidationState::ParseError.validation_tag(),
            ValidationTag::Error
        );
        assert_eq!(
            DemoValidationState::DeferredWarning.validation_tag(),
            ValidationTag::Warning
        );
        assert_eq!(
            DemoValidationState::ExplicitProblem.validation_tag(),
            ValidationTag::Error
        );
        assert!(DemoValidationState::ParseError.is_validation_error());
        assert!(DemoValidationState::DeferredWarning.has_validation_issue());
    }

    /// Demo enum for `Unsupported` naming convention.
    #[derive(ValidationTagged)]
    #[allow(dead_code)]
    enum DemoUnsupportedState {
        Known,
        Unsupported(String),
    }

    /// `Unsupported` variant maps to `Warning` by naming convention.
    #[test]
    fn test_validation_tagged_unsupported_convention() {
        assert_eq!(
            DemoUnsupportedState::Known.validation_tag(),
            ValidationTag::Clean
        );
        assert_eq!(
            DemoUnsupportedState::Unsupported("x".into()).validation_tag(),
            ValidationTag::Warning
        );
        assert!(DemoUnsupportedState::Unsupported("x".into()).has_validation_issue());
        assert!(DemoUnsupportedState::Unsupported("x".into()).is_validation_warning());
        assert!(!DemoUnsupportedState::Known.has_validation_issue());
    }

    /// Existing model enums with `Unsupported` get `Warning` tag via derive.
    #[test]
    fn test_existing_enums_validation_tagged() {
        assert!(RecordingQuality::from_text("1").validation_tag() == ValidationTag::Clean);
        assert!(RecordingQuality::from_text("unknown").has_validation_issue());
        assert!(MediaType::from_text("audio").validation_tag() == ValidationTag::Clean);
        assert!(MediaType::from_text("unknown").has_validation_issue());
        assert!(Number::from_text("1").validation_tag() == ValidationTag::Clean);
        assert!(Number::from_text("unknown").has_validation_issue());
    }
}
