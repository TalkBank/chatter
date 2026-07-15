//! MECHANICAL conformance inventory -- DO NOT HAND-EDIT.
//!
//! Generated from `crates/talkbank-parser/src/generated_traversal.rs`: a
//! no-op `Inspect` impl per leaf node wrapper (its own node kind is separately
//! visited and dispatched below), a variant-dispatching `Inspect` impl per
//! `*Choice` enum (recurse into whichever variant is actually present), a
//! field-recursing `Inspect` impl per `*Children` struct, and one `dispatch`
//! arm per `extract_*` free function (the arm key is the node kind == the rule
//! name).
//!
//! Regenerate with the committed generator after a grammar/visitor regen:
//! `cargo run -p talkbank-parser-tests --example gen_conformance_inventory`.
//! The staleness guard `conformance_inventory_is_current` re-derives this file
//! from the current typed traversal + node-types.json and fails if the
//! committed copy has drifted, so a forgotten regen breaks the suite instead of
//! silently losing coverage. The harness + allowlist live in the parent
//! `generated_traversal_conformance.rs`.

#![allow(clippy::too_many_lines)]

use talkbank_parser_tests::generated_traversal::*;

use super::{Inspect, InspectField, RawViolation};

/// Generate a no-op `Inspect` for a leaf node wrapper: its own node kind is
/// separately visited by `walk_all` and dispatched below, so there is nothing
/// further to recurse into from here.
macro_rules! impl_inspect_leaf {
    ($($name:ident),* $(,)?) => {
        $(
            impl<'tree> Inspect for $name<'tree> {
                fn inspect(&self, _rule: &'static str, _out: &mut Vec<RawViolation>) {}
            }
        )*
    };
}

/// Generate a variant-dispatching `Inspect` for a `*Choice` enum: recurse into
/// whichever variant is actually present (a leaf-wrapper payload's own
/// `Inspect` is a no-op; a synthetic Children-group payload recurses for real).
macro_rules! impl_inspect_choice {
    ($name:ident { $($variant:ident),* $(,)? }) => {
        impl<'tree> Inspect for $name<'tree> {
            fn inspect(&self, rule: &'static str, out: &mut Vec<RawViolation>) {
                match self {
                    $( Self::$variant(inner) => inner.inspect(rule, out), )*
                }
            }
        }
    };
}

/// Generate an `Inspect` impl that recurses into each named field. The
/// field-shape-specific logic (required / optional / repeat, and whether the
/// payload itself needs further recursion) lives once in the harness's
/// blanket `InspectField` impls, so every field is visited identically here
/// regardless of its declared shape.
macro_rules! impl_inspect_struct {
    ($name:ident { $($field:ident),* $(,)? }) => {
        impl<'tree> Inspect for $name<'tree> {
            fn inspect(&self, rule: &'static str, out: &mut Vec<RawViolation>) {
                $( self.$field.inspect_field(rule, stringify!($field), out); )*
            }
        }
    };
}

// --- leaf node wrapper impls ---
impl_inspect_leaf!(
    ActDependentTierNode,
    ActTierPrefixNode,
    ActivitiesHeaderNode,
    ActivitiesPrefixNode,
    AddDependentTierNode,
    AddTierPrefixNode,
    AgeFormatNode,
    AltAnnotationNode,
    AltDependentTierNode,
    AltTierPrefixNode,
    AmpersandNode,
    AnnotationContentNode,
    AnonNode,
    AnonymizedNode,
    AtBeginNode,
    AtEndNode,
    AtUTF8Node,
    AudienceNode,
    AudioValueNode,
    BaseAnnotationsNode,
    BaseContentItemNode,
    BckHeaderNode,
    BckPrefixNode,
    BeginHeaderNode,
    BgHeaderNode,
    BgPrefixNode,
    BirthOfHeaderNode,
    BirthOfPrefixNode,
    BirthplaceOfHeaderNode,
    BirthplaceOfPrefixNode,
    BlankHeaderNode,
    BlankLineNode,
    BlankPrefixNode,
    BreakForCodingNode,
    BrokenQuestionNode,
    BulletEndNode,
    BulletNode,
    BulletStartNode,
    BulletTimestampNode,
    CANode,
    CaContinuationMarkerNode,
    CaDelimiterNode,
    CaElementNode,
    CaNoBreakLinkerNode,
    CaNoBreakNode,
    CaTechnicalBreakLinkerNode,
    CaTechnicalBreakNode,
    CheckedNode,
    CoarseNode,
    CodDependentTierNode,
    CodTierPrefixNode,
    CohDependentTierNode,
    CohTierPrefixNode,
    ColonNode,
    ColorWordsHeaderNode,
    ColorWordsPrefixNode,
    ComDependentTierNode,
    ComTierPrefixNode,
    CommaNode,
    CommentHeaderNode,
    CommentPrefixNode,
    ContentItemNode,
    ContentsNode,
    ContinuationNode,
    DateContentsNode,
    DateHeaderNode,
    DatePrefixNode,
    DefDependentTierNode,
    DefTierPrefixNode,
    DetailedNode,
    DoubleQuoteNode,
    EgHeaderNode,
    EgPrefixNode,
    EndHeaderNode,
    EngDependentTierNode,
    EngTierPrefixNode,
    ErrDependentTierNode,
    ErrTierPrefixNode,
    ErrorMarkerAnnotationNode,
    EthnicityValueNode,
    EventMarkerNode,
    EventNode,
    EventSegmentNode,
    ExclamationNode,
    ExcludeMarkerNode,
    ExpDependentTierNode,
    ExpTierPrefixNode,
    ExplanationAnnotationNode,
    Extra,
    EyeDialectNode,
    FacDependentTierNode,
    FacTierPrefixNode,
    FallingToLowNode,
    FallingToMidNode,
    FemaleValueNode,
    FinalCodesNode,
    FloDependentTierNode,
    FloTierPrefixNode,
    FontHeaderNode,
    FontPrefixNode,
    FormMarkerNode,
    FreeTextNode,
    FreecodeNode,
    FullDocumentNode,
    FullNode,
    GHeaderNode,
    GPrefixNode,
    GenericDateNode,
    GenericIdSesNode,
    GenericIdSexNode,
    GenericMediaStatusNode,
    GenericMediaTypeNode,
    GenericNumberNode,
    GenericOptionNameNode,
    GenericRecordingQualityNode,
    GenericTimeNode,
    GenericTranscriptionNode,
    GlsDependentTierNode,
    GlsTierPrefixNode,
    GpxDependentTierNode,
    GpxTierPrefixNode,
    GraContentsNode,
    GraDependentTierNode,
    GraHeadNode,
    GraIndexNode,
    GraRelationNameNode,
    GraRelationNode,
    GraTierPrefixNode,
    GreaterThanNode,
    GroupWithAnnotationsNode,
    HeaderGapNode,
    HeaderSepNode,
    HyphenNode,
    IdAgeNode,
    IdContentsNode,
    IdCorpusNode,
    IdCustomFieldNode,
    IdEducationNode,
    IdGroupNode,
    IdHeaderNode,
    IdLanguagesNode,
    IdPrefixNode,
    IdRoleNode,
    IdSesNode,
    IdSexNode,
    IdSpeakerNode,
    IllegalCurlyQuoteNode,
    IndexedOverlapFollowsNode,
    IndexedOverlapPrecedesNode,
    InlinePicNode,
    IntDependentTierNode,
    IntTierPrefixNode,
    InterruptedQuestionNode,
    InterruptionNode,
    K1Node,
    K2Node,
    K3Node,
    K4Node,
    K5Node,
    L1OfHeaderNode,
    L1OfPrefixNode,
    LBrackEqBangNode,
    LBrackEqNode,
    LBrackEqQuestionNode,
    LBrackNode,
    LBrackPercentNode,
    LBrackPlusNode,
    LParenNode,
    LangcodeNode,
    LanguageCodeNode,
    LanguagesContentsNode,
    LanguagesHeaderNode,
    LanguagesPrefixNode,
    LeafText,
    LeftBracketNode,
    LeftDoubleQuoteNode,
    LengtheningNode,
    LessThanNode,
    LevelPitchNode,
    LineNode,
    LinkerLazyOverlapNode,
    LinkerQuickUptakeNode,
    LinkerQuickUptakeOverlapNode,
    LinkerQuotationFollowsNode,
    LinkerSelfCompletionNode,
    LinkersNode,
    LocationHeaderNode,
    LocationPrefixNode,
    LongFeatureBeginMarkerNode,
    LongFeatureBeginNode,
    LongFeatureEndMarkerNode,
    LongFeatureEndNode,
    LongFeatureLabelNode,
    LongFeatureNode,
    MainPhoGroupNode,
    MainSinGroupNode,
    MainTierNode,
    MaleValueNode,
    MediaContentsNode,
    MediaFilenameNode,
    MediaHeaderNode,
    MediaPrefixNode,
    MediaStatusNode,
    MediaTypeNode,
    MissingValueNode,
    ModDependentTierNode,
    ModTierPrefixNode,
    ModsylDependentTierNode,
    ModsylTierPrefixNode,
    MorContentNode,
    MorContentsNode,
    MorDependentTierNode,
    MorFeatureNode,
    MorFeatureValueNode,
    MorLemmaNode,
    MorPosNode,
    MorPostCliticNode,
    MorTierPrefixNode,
    MorWordNode,
    MoreNode,
    NewEpisodeHeaderNode,
    NewEpisodePrefixNode,
    NewlineNode,
    NoAlignNode,
    NonColonSeparatorNode,
    NonvocalBeginMarkerNode,
    NonvocalBeginNode,
    NonvocalEndMarkerNode,
    NonvocalEndNode,
    NonvocalNode,
    NonvocalSimpleNode,
    NonwordNode,
    NonwordWithOptionalAnnotationsNode,
    NotransValueNode,
    NumberHeaderNode,
    NumberOptionNode,
    NumberPrefixNode,
    OptionNameNode,
    OptionsContentsNode,
    OptionsHeaderNode,
    OptionsPrefixNode,
    OrtDependentTierNode,
    OrtTierPrefixNode,
    OtherSpokenEventNode,
    OverlapPointNode,
    PageHeaderNode,
    PageNumberNode,
    PagePrefixNode,
    ParDependentTierNode,
    ParTierPrefixNode,
    ParaAnnotationNode,
    PartialNode,
    ParticipantNode,
    ParticipantWordNode,
    ParticipantsContentsNode,
    ParticipantsHeaderNode,
    ParticipantsPrefixNode,
    PauseTokenNode,
    PercentAnnotationNode,
    PeriodNode,
    PhoBeginGroupNode,
    PhoDependentTierNode,
    PhoEndGroupNode,
    PhoGroupNode,
    PhoGroupedContentNode,
    PhoGroupsNode,
    PhoTierPrefixNode,
    PhoWordNode,
    PhoWordsNode,
    PhoalnDependentTierNode,
    PhoalnTierPrefixNode,
    PhosylDependentTierNode,
    PhosylTierPrefixNode,
    PidHeaderNode,
    PidPrefixNode,
    PipeNode,
    PlusNode,
    Plus_2Node,
    PosTagNode,
    PostcodeNode,
    QuestionNode,
    QuotationNode,
    QuotedNewLineNode,
    QuotedPeriodSimpleNode,
    RParenNode,
    RecordingQualityHeaderNode,
    RecordingQualityOptionNode,
    RecordingQualityPrefixNode,
    ReplacementNode,
    RestOfLineNode,
    RetraceCompleteNode,
    RetraceMultipleNode,
    RetracePartialNode,
    RetraceReformulationNode,
    RightBraceNode,
    RightBracketNode,
    RightDoubleQuoteNode,
    RisingToHighNode,
    RisingToMidNode,
    RoomLayoutHeaderNode,
    RoomLayoutPrefixNode,
    ScopedContrastiveStressingNode,
    ScopedStressingNode,
    ScopedUncertainNode,
    SelfInterruptedQuestionNode,
    SelfInterruptionNode,
    SemicolonNode,
    SeparatorNode,
    SesCodeValueNode,
    SesCombinedNode,
    ShorteningNode,
    SinBeginGroupNode,
    SinDependentTierNode,
    SinEndGroupNode,
    SinGroupNode,
    SinGroupedContentNode,
    SinGroupsNode,
    SinTierPrefixNode,
    SinWordNode,
    SitDependentTierNode,
    SitTierPrefixNode,
    SituationHeaderNode,
    SituationPrefixNode,
    SourceFileNode,
    SpaDependentTierNode,
    SpaTierPrefixNode,
    SpaceNode,
    SpeakerNode,
    StandaloneWordNode,
    StarNode,
    StressMarkerNode,
    StrictDateNode,
    StrictTimeNode,
    SyllablePauseNode,
    THeaderNode,
    TPrefixNode,
    TabNode,
    TagMarkerNode,
    TapeLocationHeaderNode,
    TapeLocationPrefixNode,
    TextSegmentNode,
    TextWithBulletsAndPicsNode,
    TextWithBulletsNode,
    ThumbnailHeaderNode,
    ThumbnailPrefixNode,
    TierBodyNode,
    TierSepNode,
    TildeNode,
    TimDependentTierNode,
    TimTierPrefixNode,
    TimeDurationContentsNode,
    TimeDurationHeaderNode,
    TimeDurationPrefixNode,
    TimeStartHeaderNode,
    TimeStartPrefixNode,
    TrailingOffNode,
    TrailingOffQuestionNode,
    TranscriberHeaderNode,
    TranscriberPrefixNode,
    TranscriptionHeaderNode,
    TranscriptionOptionNode,
    TranscriptionPrefixNode,
    TypesActivityNode,
    TypesDesignNode,
    TypesGroupNode,
    TypesHeaderNode,
    TypesPrefixNode,
    UnderlineBeginNode,
    UnderlineEndNode,
    UnlinkedValueNode,
    UnmarkedEndingNode,
    UnsupportedDependentTierNode,
    UnsupportedHeaderNode,
    UnsupportedHeaderPrefixNode,
    UnsupportedLineNode,
    UnsupportedTierPrefixNode,
    UptakeSymbolNode,
    Utf8HeaderNode,
    UtteranceEndNode,
    UtteranceNode,
    VideoValueNode,
    VideosHeaderNode,
    VideosPrefixNode,
    VocativeMarkerNode,
    WarningHeaderNode,
    WarningPrefixNode,
    WhitespacesNode,
    WindowHeaderNode,
    WindowPrefixNode,
    WorDependentTierNode,
    WorTierBodyNode,
    WorTierPrefixNode,
    WorWordItemNode,
    WordBodyNode,
    WordLangSuffixNode,
    WordPrefixNode,
    WordSegmentNode,
    WordWithOptionalAnnotationsNode,
    XDependentTierNode,
    XTierPrefixNode,
    XphointDependentTierNode,
    XphointTierPrefixNode,
    ZeroNode,
);

// --- *Choice enum impls ---
impl_inspect_choice!(BaseAnnotationChoice {
    AltAnnotation,
    ErrorMarkerAnnotation,
    ExcludeMarker,
    ExplanationAnnotation,
    IndexedOverlapFollows,
    IndexedOverlapPrecedes,
    ParaAnnotation,
    PercentAnnotation,
    RetraceComplete,
    RetraceMultiple,
    RetracePartial,
    RetraceReformulation,
    ScopedContrastiveStressing,
    ScopedStressing,
    ScopedUncertain
});
impl_inspect_choice!(BaseAnnotationsChild0Child1Choice {
    AltAnnotation,
    ErrorMarkerAnnotation,
    ExcludeMarker,
    ExplanationAnnotation,
    IndexedOverlapFollows,
    IndexedOverlapPrecedes,
    ParaAnnotation,
    PercentAnnotation,
    RetraceComplete,
    RetraceMultiple,
    RetracePartial,
    RetraceReformulation,
    ScopedContrastiveStressing,
    ScopedStressing,
    ScopedUncertain
});
impl_inspect_choice!(BaseAnnotationsChild1Child1Choice {
    AltAnnotation,
    ErrorMarkerAnnotation,
    ExcludeMarker,
    ExplanationAnnotation,
    IndexedOverlapFollows,
    IndexedOverlapPrecedes,
    ParaAnnotation,
    PercentAnnotation,
    RetraceComplete,
    RetraceMultiple,
    RetracePartial,
    RetraceReformulation,
    ScopedContrastiveStressing,
    ScopedStressing,
    ScopedUncertain
});
impl_inspect_choice!(BaseContentItemChoice {
    UnderlineBegin,
    UnderlineEnd,
    PauseToken,
    WordWithOptionalAnnotations,
    NonwordWithOptionalAnnotations,
    OtherSpokenEvent,
    LongFeature,
    Nonvocal,
    Freecode,
    Bullet
});
impl_inspect_choice!(ContentItemChoice {
    BaseContentItem,
    GroupWithAnnotations,
    Quotation,
    IllegalCurlyQuote,
    MainPhoGroup,
    MainSinGroup
});
impl_inspect_choice!(ContentsChild0Choice {
    Whitespaces,
    ContentItem,
    Separator,
    OverlapPoint
});
impl_inspect_choice!(ContentsChild1Choice {
    Whitespaces,
    ContentItem,
    Separator,
    OverlapPoint
});
impl_inspect_choice!(DateContentsChoice {
    StrictDate,
    GenericDate
});
impl_inspect_choice!(DependentTierChoice {
    ActDependentTier,
    AddDependentTier,
    AltDependentTier,
    CodDependentTier,
    CohDependentTier,
    ComDependentTier,
    DefDependentTier,
    EngDependentTier,
    ErrDependentTier,
    ExpDependentTier,
    FacDependentTier,
    FloDependentTier,
    GlsDependentTier,
    GpxDependentTier,
    GraDependentTier,
    IntDependentTier,
    ModDependentTier,
    ModsylDependentTier,
    MorDependentTier,
    OrtDependentTier,
    ParDependentTier,
    PhoDependentTier,
    PhoalnDependentTier,
    PhosylDependentTier,
    SinDependentTier,
    SitDependentTier,
    SpaDependentTier,
    TimDependentTier,
    UnsupportedDependentTier,
    WorDependentTier,
    XDependentTier,
    XphointDependentTier
});
impl_inspect_choice!(FreeTextChild0Choice {
    RestOfLine,
    Continuation
});
impl_inspect_choice!(FreeTextChild1Choice {
    RestOfLine,
    Continuation
});
impl_inspect_choice!(FullDocumentChild1Choice {
    ColorWordsHeader,
    FontHeader,
    PidHeader,
    WindowHeader
});
impl_inspect_choice!(HeaderChoice {
    ActivitiesHeader,
    BckHeader,
    BgHeader,
    BirthOfHeader,
    BirthplaceOfHeader,
    BlankHeader,
    CommentHeader,
    DateHeader,
    EgHeader,
    GHeader,
    IdHeader,
    L1OfHeader,
    LanguagesHeader,
    LocationHeader,
    MediaHeader,
    NewEpisodeHeader,
    NumberHeader,
    OptionsHeader,
    PageHeader,
    ParticipantsHeader,
    RecordingQualityHeader,
    RoomLayoutHeader,
    SituationHeader,
    THeader,
    TapeLocationHeader,
    ThumbnailHeader,
    TimeDurationHeader,
    TimeStartHeader,
    TranscriberHeader,
    TranscriptionHeader,
    TypesHeader,
    UnsupportedHeader,
    VideosHeader,
    WarningHeader
});
impl_inspect_choice!(HeaderGapChild0Choice { Space, Tab });
impl_inspect_choice!(HeaderGapChild1Choice { Space, Tab });
impl_inspect_choice!(IdAgeChoice {
    AgeFormat,
    TRNRNTRN
});
impl_inspect_choice!(IdLanguagesChoice {
    LanguagesContents,
    RN
});
impl_inspect_choice!(IdSesChoice {
    SesCombined,
    SesCodeValue,
    EthnicityValue,
    GenericIdSes
});
impl_inspect_choice!(IdSexChoice {
    MaleValue,
    FemaleValue,
    GenericIdSex
});
impl_inspect_choice!(LineActivitiesHeaderChoice {
    ActivitiesHeader,
    BckHeader,
    BgHeader,
    BirthOfHeader,
    BirthplaceOfHeader,
    BlankHeader,
    CommentHeader,
    DateHeader,
    EgHeader,
    GHeader,
    IdHeader,
    L1OfHeader,
    LanguagesHeader,
    LocationHeader,
    MediaHeader,
    NewEpisodeHeader,
    NumberHeader,
    OptionsHeader,
    PageHeader,
    ParticipantsHeader,
    RecordingQualityHeader,
    RoomLayoutHeader,
    SituationHeader,
    THeader,
    TapeLocationHeader,
    ThumbnailHeader,
    TimeDurationHeader,
    TimeStartHeader,
    TranscriberHeader,
    TranscriptionHeader,
    TypesHeader,
    UnsupportedHeader,
    VideosHeader,
    WarningHeader
});
impl_inspect_choice!(LineChoice {
    ActivitiesHeader,
    Utterance,
    BlankLine,
    UnsupportedLine
});
impl_inspect_choice!(LinkerChoice {
    CaNoBreakLinker,
    CaTechnicalBreakLinker,
    LinkerLazyOverlap,
    LinkerQuickUptake,
    LinkerQuickUptakeOverlap,
    LinkerQuotationFollows,
    LinkerSelfCompletion
});
impl_inspect_choice!(LinkersChild0Child0Choice {
    CaNoBreakLinker,
    CaTechnicalBreakLinker,
    LinkerLazyOverlap,
    LinkerQuickUptake,
    LinkerQuickUptakeOverlap,
    LinkerQuotationFollows,
    LinkerSelfCompletion
});
impl_inspect_choice!(LinkersChild1Child0Choice {
    CaNoBreakLinker,
    CaTechnicalBreakLinker,
    LinkerLazyOverlap,
    LinkerQuickUptake,
    LinkerQuickUptakeOverlap,
    LinkerQuotationFollows,
    LinkerSelfCompletion
});
impl_inspect_choice!(LongFeatureChoice {
    LongFeatureBegin,
    LongFeatureEnd
});
impl_inspect_choice!(MediaFilenameChoice {
    DoubleQuote,
    AZAZ09
});
impl_inspect_choice!(MediaStatusChoice {
    MissingValue,
    UnlinkedValue,
    NotransValue,
    GenericMediaStatus
});
impl_inspect_choice!(MediaTypeChoice {
    VideoValue,
    AudioValue,
    MissingValue,
    GenericMediaType
});
impl_inspect_choice!(MorContentsChild0MorContentChild2Child1Choice {
    BreakForCoding,
    BrokenQuestion,
    Exclamation,
    InterruptedQuestion,
    Interruption,
    Period,
    Question,
    QuotedNewLine,
    QuotedPeriodSimple,
    SelfInterruptedQuestion,
    SelfInterruption,
    TrailingOff,
    TrailingOffQuestion
});
impl_inspect_choice!(MorContentsChild0BreakForCodingChoice {
    BreakForCoding,
    BrokenQuestion,
    Exclamation,
    InterruptedQuestion,
    Interruption,
    Period,
    Question,
    QuotedNewLine,
    QuotedPeriodSimple,
    SelfInterruptedQuestion,
    SelfInterruption,
    TrailingOff,
    TrailingOffQuestion
});
impl_inspect_choice!(MorContentsChild0Choice {
    MorContent,
    BreakForCoding
});
impl_inspect_choice!(NonColonSeparatorChoice {
    Comma,
    Semicolon,
    TagMarker,
    VocativeMarker,
    CaContinuationMarker,
    UnmarkedEnding,
    UptakeSymbol,
    CaNoBreak,
    CaTechnicalBreak,
    RisingToHigh,
    RisingToMid,
    LevelPitch,
    FallingToMid,
    FallingToLow
});
impl_inspect_choice!(NonvocalChoice {
    NonvocalBegin,
    NonvocalEnd,
    NonvocalSimple
});
impl_inspect_choice!(NonwordChoice { Event, Zero });
impl_inspect_choice!(NumberOptionChoice {
    _1,
    _2,
    _3,
    _4,
    _5,
    More,
    Audience,
    GenericNumber
});
impl_inspect_choice!(OptionNameChoice {
    CA,
    NoAlign,
    GenericOptionName
});
impl_inspect_choice!(PhoGroupChoice {
    PhoWords,
    PhoBeginGroup
});
impl_inspect_choice!(PreBeginHeaderChoice {
    ColorWordsHeader,
    FontHeader,
    PidHeader,
    WindowHeader
});
impl_inspect_choice!(RecordingQualityOptionChoice {
    _1,
    _2,
    _3,
    _4,
    _5,
    GenericRecordingQuality
});
impl_inspect_choice!(SeparatorChoice {
    NonColonSeparator,
    Colon
});
impl_inspect_choice!(SinGroupChoice {
    SinWord,
    SinBeginGroup
});
impl_inspect_choice!(SinWordChoice { Zero, AZAZ09 });
impl_inspect_choice!(SourceFileActDependentTierChoice {
    ActDependentTier,
    AddDependentTier,
    AltDependentTier,
    CodDependentTier,
    CohDependentTier,
    ComDependentTier,
    DefDependentTier,
    EngDependentTier,
    ErrDependentTier,
    ExpDependentTier,
    FacDependentTier,
    FloDependentTier,
    GlsDependentTier,
    GpxDependentTier,
    GraDependentTier,
    IntDependentTier,
    ModDependentTier,
    ModsylDependentTier,
    MorDependentTier,
    OrtDependentTier,
    ParDependentTier,
    PhoDependentTier,
    PhoalnDependentTier,
    PhosylDependentTier,
    SinDependentTier,
    SitDependentTier,
    SpaDependentTier,
    TimDependentTier,
    UnsupportedDependentTier,
    WorDependentTier,
    XDependentTier,
    XphointDependentTier
});
impl_inspect_choice!(SourceFileActivitiesHeaderChoice {
    ActivitiesHeader,
    BckHeader,
    BgHeader,
    BirthOfHeader,
    BirthplaceOfHeader,
    BlankHeader,
    CommentHeader,
    DateHeader,
    EgHeader,
    GHeader,
    IdHeader,
    L1OfHeader,
    LanguagesHeader,
    LocationHeader,
    MediaHeader,
    NewEpisodeHeader,
    NumberHeader,
    OptionsHeader,
    PageHeader,
    ParticipantsHeader,
    RecordingQualityHeader,
    RoomLayoutHeader,
    SituationHeader,
    THeader,
    TapeLocationHeader,
    ThumbnailHeader,
    TimeDurationHeader,
    TimeStartHeader,
    TranscriberHeader,
    TranscriptionHeader,
    TypesHeader,
    UnsupportedHeader,
    VideosHeader,
    WarningHeader
});
impl_inspect_choice!(SourceFileColorWordsHeaderChoice {
    ColorWordsHeader,
    FontHeader,
    PidHeader,
    WindowHeader
});
impl_inspect_choice!(SourceFileChoice {
    FullDocument,
    Utterance,
    MainTier,
    ActDependentTier,
    ActivitiesHeader,
    ColorWordsHeader,
    StandaloneWord
});
impl_inspect_choice!(StandaloneWordChild0Choice { WordPrefix, Zero });
impl_inspect_choice!(TerminatorChoice {
    BreakForCoding,
    BrokenQuestion,
    Exclamation,
    InterruptedQuestion,
    Interruption,
    Period,
    Question,
    QuotedNewLine,
    QuotedPeriodSimple,
    SelfInterruptedQuestion,
    SelfInterruption,
    TrailingOff,
    TrailingOffQuestion
});
impl_inspect_choice!(TextWithBulletsChild0Choice {
    TextSegment,
    Bullet,
    Continuation
});
impl_inspect_choice!(TextWithBulletsChild1Choice {
    TextSegment,
    Bullet,
    Continuation
});
impl_inspect_choice!(TextWithBulletsAndPicsChild0Choice {
    TextSegment,
    Bullet,
    InlinePic,
    Continuation
});
impl_inspect_choice!(TextWithBulletsAndPicsChild1Choice {
    TextSegment,
    Bullet,
    InlinePic,
    Continuation
});
impl_inspect_choice!(TimeDurationContentsChoice {
    StrictTime,
    GenericTime
});
impl_inspect_choice!(TranscriptionOptionChoice {
    EyeDialect,
    Partial,
    Full,
    Detailed,
    Coarse,
    Checked,
    Anonymized,
    GenericTranscription
});
impl_inspect_choice!(UtteranceChild1Choice {
    ActDependentTier,
    AddDependentTier,
    AltDependentTier,
    CodDependentTier,
    CohDependentTier,
    ComDependentTier,
    DefDependentTier,
    EngDependentTier,
    ErrDependentTier,
    ExpDependentTier,
    FacDependentTier,
    FloDependentTier,
    GlsDependentTier,
    GpxDependentTier,
    GraDependentTier,
    IntDependentTier,
    ModDependentTier,
    ModsylDependentTier,
    MorDependentTier,
    OrtDependentTier,
    ParDependentTier,
    PhoDependentTier,
    PhoalnDependentTier,
    PhosylDependentTier,
    SinDependentTier,
    SitDependentTier,
    SpaDependentTier,
    TimDependentTier,
    UnsupportedDependentTier,
    WorDependentTier,
    XDependentTier,
    XphointDependentTier
});
impl_inspect_choice!(UtteranceEndChild0Choice {
    BreakForCoding,
    BrokenQuestion,
    Exclamation,
    InterruptedQuestion,
    Interruption,
    Period,
    Question,
    QuotedNewLine,
    QuotedPeriodSimple,
    SelfInterruptedQuestion,
    SelfInterruption,
    TrailingOff,
    TrailingOffQuestion
});
impl_inspect_choice!(WorTierBodyChild1Child0Choice {
    WorWordItem,
    Bullet,
    Comma,
    TagMarker,
    VocativeMarker
});
impl_inspect_choice!(WorTierBodyChild2Choice {
    BreakForCoding,
    BrokenQuestion,
    Exclamation,
    InterruptedQuestion,
    Interruption,
    Period,
    Question,
    QuotedNewLine,
    QuotedPeriodSimple,
    SelfInterruptedQuestion,
    SelfInterruption,
    TrailingOff,
    TrailingOffQuestion
});
impl_inspect_choice!(WordBodyWordSegmentChild0Choice {
    WordSegment,
    Shortening,
    StressMarker
});
impl_inspect_choice!(WordBodyWordSegmentChild1LengtheningChoice {
    Lengthening,
    OverlapPoint,
    CaElement,
    CaDelimiter,
    UnderlineBegin,
    UnderlineEnd,
    SyllablePause,
    Tilde,
    Variant8
});
impl_inspect_choice!(WordBodyWordSegmentChild1Choice {
    WordSegment,
    Shortening,
    StressMarker,
    Lengthening
});
impl_inspect_choice!(WordBodyOverlapPointChild0Choice {
    OverlapPoint,
    CaElement,
    CaDelimiter,
    UnderlineBegin,
    SyllablePause
});
impl_inspect_choice!(WordBodyOverlapPointChild1Choice {
    OverlapPoint,
    CaElement,
    CaDelimiter,
    UnderlineBegin,
    SyllablePause
});
impl_inspect_choice!(WordBodyOverlapPointChild2Choice {
    WordSegment,
    Shortening,
    StressMarker
});
impl_inspect_choice!(WordBodyOverlapPointChild3LengtheningChoice {
    Lengthening,
    OverlapPoint,
    CaElement,
    CaDelimiter,
    UnderlineBegin,
    UnderlineEnd,
    SyllablePause,
    Tilde,
    Variant8
});
impl_inspect_choice!(WordBodyOverlapPointChild3Choice {
    WordSegment,
    Shortening,
    StressMarker,
    Lengthening
});
impl_inspect_choice!(WordBodyChoice {
    WordSegment,
    OverlapPoint
});

// --- *Children struct impls ---
impl_inspect_struct!(ActDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(ActivitiesHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AddDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AltAnnotationChildren {
    child_0,
    child_1,
    text,
    child_3
});
impl_inspect_struct!(AltDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(BaseAnnotationChildren { content });
impl_inspect_struct!(BaseAnnotationsChild0Children { child_0, child_1 });
impl_inspect_struct!(BaseAnnotationsChild1Children { child_0, child_1 });
impl_inspect_struct!(BaseAnnotationsChildren { child_0, child_1 });
impl_inspect_struct!(BaseContentItemChildren { content });
impl_inspect_struct!(BckHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(BeginHeaderChildren { child_0, child_1 });
impl_inspect_struct!(BgHeaderChild1Children { child_0, child_1 });
impl_inspect_struct!(BgHeaderChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(BirthOfHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3,
    child_4,
    child_5
});
impl_inspect_struct!(BirthplaceOfHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3,
    child_4,
    child_5
});
impl_inspect_struct!(BlankHeaderChildren { child_0, child_1 });
impl_inspect_struct!(BlankLineChildren { content });
impl_inspect_struct!(BulletChildren {
    child_0,
    start_time,
    child_2,
    end_time,
    child_4
});
impl_inspect_struct!(CodDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(CohDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(ColorWordsHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(ComDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(CommentHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(ContentItemChildren { content });
impl_inspect_struct!(ContentsChildren { child_0, child_1 });
impl_inspect_struct!(DateContentsChildren { content });
impl_inspect_struct!(DateHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(DefDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(DependentTierChildren { content });
impl_inspect_struct!(EgHeaderChild1Children { child_0, child_1 });
impl_inspect_struct!(EgHeaderChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(EndHeaderChildren { child_0, child_1 });
impl_inspect_struct!(EngDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(ErrDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(EventChildren {
    child_0,
    description
});
impl_inspect_struct!(ExpDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(ExplanationAnnotationChildren {
    child_0,
    child_1,
    text,
    child_3
});
impl_inspect_struct!(FacDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(FinalCodesChild0Children { child_0, child_1 });
impl_inspect_struct!(FinalCodesChild1Children { child_0, child_1 });
impl_inspect_struct!(FinalCodesChildren { child_0, child_1 });
impl_inspect_struct!(FloDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(FontHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(FreeTextChildren { child_0, child_1 });
impl_inspect_struct!(FullDocumentChildren {
    child_0,
    child_1,
    child_2,
    child_3,
    child_4
});
impl_inspect_struct!(GHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(GlsDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(GpxDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(GraContentsChild1Children { child_0, child_1 });
impl_inspect_struct!(GraContentsChildren { child_0, child_1 });
impl_inspect_struct!(GraDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(GraRelationChildren {
    index,
    child_1,
    head,
    child_3,
    relation
});
impl_inspect_struct!(GroupWithAnnotationsChildren {
    child_0,
    content_2,
    child_2,
    annotations
});
impl_inspect_struct!(HeaderChildren { content });
impl_inspect_struct!(HeaderGapChildren { child_0, child_1 });
impl_inspect_struct!(HeaderSepChildren { child_0, child_1 });
impl_inspect_struct!(IdAgeChildren { content });
impl_inspect_struct!(IdContentsChildren {
    child_0,
    child_1,
    child_2,
    child_3,
    child_4,
    child_5,
    child_6,
    child_7,
    child_8,
    child_9,
    child_10,
    child_11,
    child_12,
    child_13,
    child_14,
    child_15,
    child_16,
    child_17,
    child_18,
    child_19,
    child_20,
    child_21,
    child_22,
    child_23,
    child_24,
    child_25,
    child_26,
    child_27,
    child_28,
    child_29,
    child_30,
    child_31,
    child_32,
    child_33
});
impl_inspect_struct!(IdHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(IdLanguagesChildren { content });
impl_inspect_struct!(IdSesChildren { content });
impl_inspect_struct!(IdSexChildren { content });
impl_inspect_struct!(IntDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(L1OfHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3,
    child_4,
    child_5
});
impl_inspect_struct!(LangcodeChildren {
    child_0,
    child_1,
    code,
    child_3
});
impl_inspect_struct!(LanguagesContentsChild1Children {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(LanguagesContentsChildren { child_0, child_1 });
impl_inspect_struct!(LanguagesHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(LineChildren { content });
impl_inspect_struct!(LinkerChildren { content });
impl_inspect_struct!(LinkersChild0Children { child_0, child_1 });
impl_inspect_struct!(LinkersChild1Children { child_0, child_1 });
impl_inspect_struct!(LinkersChildren { child_0, child_1 });
impl_inspect_struct!(LocationHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(LongFeatureChildren { content });
impl_inspect_struct!(LongFeatureBeginChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(LongFeatureEndChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(MainPhoGroupChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(MainSinGroupChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(MainTierChildren {
    child_0,
    speaker,
    child_2,
    child_3,
    child_4
});
impl_inspect_struct!(MediaContentsChild4Children {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(MediaContentsChildren {
    child_0,
    child_1,
    child_2,
    child_3,
    child_4
});
impl_inspect_struct!(MediaFilenameDoubleQuoteChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(MediaFilenameChildren { content });
impl_inspect_struct!(MediaHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(MediaStatusChildren { content });
impl_inspect_struct!(MediaTypeChildren { content });
impl_inspect_struct!(ModDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(ModsylDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(MorContentChildren { main, post_clitics });
impl_inspect_struct!(MorContentsChild0MorContentChild1Children { child_0, child_1 });
impl_inspect_struct!(MorContentsChild0MorContentChild2Children { child_0, child_1 });
impl_inspect_struct!(MorContentsChild0MorContentChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(MorContentsChildren { child_0, child_1 });
impl_inspect_struct!(MorDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(MorFeatureChildren { child_0, child_1 });
impl_inspect_struct!(MorPostCliticChildren { child_0, child_1 });
impl_inspect_struct!(MorWordChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(NewEpisodeHeaderChildren { child_0, child_1 });
impl_inspect_struct!(NonColonSeparatorChildren { content });
impl_inspect_struct!(NonvocalChildren { content });
impl_inspect_struct!(NonvocalBeginChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(NonvocalEndChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(NonvocalSimpleChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(NonwordChildren { content });
impl_inspect_struct!(NonwordWithOptionalAnnotationsChildren {
    nonword,
    annotations
});
impl_inspect_struct!(NumberHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(NumberOptionChildren { content });
impl_inspect_struct!(OptionNameChildren { content });
impl_inspect_struct!(OptionsContentsChild1Children {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(OptionsContentsChildren { child_0, child_1 });
impl_inspect_struct!(OptionsHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(OrtDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(OtherSpokenEventChildren {
    child_0,
    child_1,
    child_2,
    child_3,
    child_4
});
impl_inspect_struct!(PageHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(ParDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(ParaAnnotationChildren {
    child_0,
    child_1,
    text,
    child_3
});
impl_inspect_struct!(ParticipantChild1Children { child_0, child_1 });
impl_inspect_struct!(ParticipantChildren {
    code,
    child_1,
    child_2
});
impl_inspect_struct!(ParticipantsContentsChild1Children {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(ParticipantsContentsChildren { child_0, child_1 });
impl_inspect_struct!(ParticipantsHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(PercentAnnotationChildren {
    child_0,
    child_1,
    text,
    child_3
});
impl_inspect_struct!(PhoDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(PhoGroupPhoBeginGroupChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(PhoGroupChildren { content });
impl_inspect_struct!(PhoGroupedContentChild1Children { child_0, child_1 });
impl_inspect_struct!(PhoGroupedContentChildren { child_0, child_1 });
impl_inspect_struct!(PhoGroupsChild1Children { child_0, child_1 });
impl_inspect_struct!(PhoGroupsChildren { child_0, child_1 });
impl_inspect_struct!(PhoWordsChild1Children { child_0, child_1 });
impl_inspect_struct!(PhoWordsChildren { child_0, child_1 });
impl_inspect_struct!(PhoalnDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(PhosylDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(PidHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(PostcodeChildren {
    child_0,
    child_1,
    code,
    child_3
});
impl_inspect_struct!(PreBeginHeaderChildren { content });
impl_inspect_struct!(QuotationChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(RecordingQualityHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(RecordingQualityOptionChildren { content });
impl_inspect_struct!(ReplacementChild2Children { child_0, child_1 });
impl_inspect_struct!(ReplacementChild3Children { child_0, child_1 });
impl_inspect_struct!(ReplacementChildren {
    child_0,
    child_1,
    child_2,
    child_3,
    child_4
});
impl_inspect_struct!(RoomLayoutHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(SeparatorChildren { content });
impl_inspect_struct!(ShorteningChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(SinDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(SinGroupSinBeginGroupChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(SinGroupChildren { content });
impl_inspect_struct!(SinGroupedContentChild1Children { child_0, child_1 });
impl_inspect_struct!(SinGroupedContentChildren { child_0, child_1 });
impl_inspect_struct!(SinGroupsChild1Children { child_0, child_1 });
impl_inspect_struct!(SinGroupsChildren { child_0, child_1 });
impl_inspect_struct!(SinWordChildren { content });
impl_inspect_struct!(SitDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(SituationHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(SourceFileChildren { content });
impl_inspect_struct!(SpaDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(StandaloneWordChildren {
    child_0,
    child_1,
    child_2,
    child_3,
    child_4
});
impl_inspect_struct!(THeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(TapeLocationHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(TerminatorChildren { content });
impl_inspect_struct!(TextWithBulletsChildren { child_0, child_1 });
impl_inspect_struct!(TextWithBulletsAndPicsChildren { child_0, child_1 });
impl_inspect_struct!(ThumbnailHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(TierBodyLanguageCodeChildren { child_0, child_1 });
impl_inspect_struct!(TierBodyChildren {
    linkers,
    language_code,
    content_2,
    ending
});
impl_inspect_struct!(TierSepChildren { child_0, child_1 });
impl_inspect_struct!(TimDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(TimeDurationContentsChildren { content });
impl_inspect_struct!(TimeDurationHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(TimeStartHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(TranscriberHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(TranscriptionHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(TranscriptionOptionChildren { content });
impl_inspect_struct!(TypesHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3,
    child_4,
    child_5,
    child_6,
    child_7,
    child_8,
    child_9,
    child_10,
    child_11
});
impl_inspect_struct!(UnsupportedDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(UnsupportedHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(UnsupportedLineChildren { child_0, child_1 });
impl_inspect_struct!(Utf8HeaderChildren { child_0, child_1 });
impl_inspect_struct!(UtteranceChildren { child_0, child_1 });
impl_inspect_struct!(UtteranceEndChild2Children { child_0, child_1 });
impl_inspect_struct!(UtteranceEndChildren {
    child_0,
    child_1,
    child_2,
    child_3,
    child_4
});
impl_inspect_struct!(VideosHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(WarningHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(WindowHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(WorDependentTierChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(WorTierBodyLanguageCodeChildren { child_0, child_1 });
impl_inspect_struct!(WorTierBodyChild1Children { child_0, child_1 });
impl_inspect_struct!(WorTierBodyChildren {
    language_code,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(WorWordItemChildren { content });
impl_inspect_struct!(WordBodyWordSegmentChildren { child_0, child_1 });
impl_inspect_struct!(WordBodyOverlapPointChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(WordBodyChildren { content });
impl_inspect_struct!(WordWithOptionalAnnotationsChild1Children {
    child_0,
    replacement
});
impl_inspect_struct!(WordWithOptionalAnnotationsChildren {
    word,
    child_1,
    annotations
});
impl_inspect_struct!(XDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(XphointDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});

/// Drive the generated `extract_*` for `node` if its kind has one, then
/// inspect the returned children. One arm per `extract_*` free function.
pub(super) fn dispatch(node: tree_sitter::Node, out: &mut Vec<RawViolation>) {
    match node.kind() {
        "act_dependent_tier" => extract_act_dependent_tier(ActDependentTierNode(node))
            .inspect("act_dependent_tier", out),
        "activities_header" => {
            extract_activities_header(ActivitiesHeaderNode(node)).inspect("activities_header", out)
        }
        "add_dependent_tier" => extract_add_dependent_tier(AddDependentTierNode(node))
            .inspect("add_dependent_tier", out),
        "alt_annotation" => {
            extract_alt_annotation(AltAnnotationNode(node)).inspect("alt_annotation", out)
        }
        "alt_dependent_tier" => extract_alt_dependent_tier(AltDependentTierNode(node))
            .inspect("alt_dependent_tier", out),
        "base_annotation" => extract_base_annotation(node).inspect("base_annotation", out),
        "base_annotations" => {
            extract_base_annotations(BaseAnnotationsNode(node)).inspect("base_annotations", out)
        }
        "base_content_item" => {
            extract_base_content_item(BaseContentItemNode(node)).inspect("base_content_item", out)
        }
        "bck_header" => extract_bck_header(BckHeaderNode(node)).inspect("bck_header", out),
        "begin_header" => extract_begin_header(BeginHeaderNode(node)).inspect("begin_header", out),
        "bg_header" => extract_bg_header(BgHeaderNode(node)).inspect("bg_header", out),
        "birth_of_header" => {
            extract_birth_of_header(BirthOfHeaderNode(node)).inspect("birth_of_header", out)
        }
        "birthplace_of_header" => extract_birthplace_of_header(BirthplaceOfHeaderNode(node))
            .inspect("birthplace_of_header", out),
        "blank_header" => extract_blank_header(BlankHeaderNode(node)).inspect("blank_header", out),
        "blank_line" => extract_blank_line(BlankLineNode(node)).inspect("blank_line", out),
        "bullet" => extract_bullet(BulletNode(node)).inspect("bullet", out),
        "cod_dependent_tier" => extract_cod_dependent_tier(CodDependentTierNode(node))
            .inspect("cod_dependent_tier", out),
        "coh_dependent_tier" => extract_coh_dependent_tier(CohDependentTierNode(node))
            .inspect("coh_dependent_tier", out),
        "color_words_header" => extract_color_words_header(ColorWordsHeaderNode(node))
            .inspect("color_words_header", out),
        "com_dependent_tier" => extract_com_dependent_tier(ComDependentTierNode(node))
            .inspect("com_dependent_tier", out),
        "comment_header" => {
            extract_comment_header(CommentHeaderNode(node)).inspect("comment_header", out)
        }
        "content_item" => extract_content_item(ContentItemNode(node)).inspect("content_item", out),
        "contents" => extract_contents(ContentsNode(node)).inspect("contents", out),
        "date_contents" => {
            extract_date_contents(DateContentsNode(node)).inspect("date_contents", out)
        }
        "date_header" => extract_date_header(DateHeaderNode(node)).inspect("date_header", out),
        "def_dependent_tier" => extract_def_dependent_tier(DefDependentTierNode(node))
            .inspect("def_dependent_tier", out),
        "dependent_tier" => extract_dependent_tier(node).inspect("dependent_tier", out),
        "eg_header" => extract_eg_header(EgHeaderNode(node)).inspect("eg_header", out),
        "end_header" => extract_end_header(EndHeaderNode(node)).inspect("end_header", out),
        "eng_dependent_tier" => extract_eng_dependent_tier(EngDependentTierNode(node))
            .inspect("eng_dependent_tier", out),
        "err_dependent_tier" => extract_err_dependent_tier(ErrDependentTierNode(node))
            .inspect("err_dependent_tier", out),
        "event" => extract_event(EventNode(node)).inspect("event", out),
        "exp_dependent_tier" => extract_exp_dependent_tier(ExpDependentTierNode(node))
            .inspect("exp_dependent_tier", out),
        "explanation_annotation" => extract_explanation_annotation(ExplanationAnnotationNode(node))
            .inspect("explanation_annotation", out),
        "fac_dependent_tier" => extract_fac_dependent_tier(FacDependentTierNode(node))
            .inspect("fac_dependent_tier", out),
        "final_codes" => extract_final_codes(FinalCodesNode(node)).inspect("final_codes", out),
        "flo_dependent_tier" => extract_flo_dependent_tier(FloDependentTierNode(node))
            .inspect("flo_dependent_tier", out),
        "font_header" => extract_font_header(FontHeaderNode(node)).inspect("font_header", out),
        "free_text" => extract_free_text(FreeTextNode(node)).inspect("free_text", out),
        "full_document" => {
            extract_full_document(FullDocumentNode(node)).inspect("full_document", out)
        }
        "g_header" => extract_g_header(GHeaderNode(node)).inspect("g_header", out),
        "gls_dependent_tier" => extract_gls_dependent_tier(GlsDependentTierNode(node))
            .inspect("gls_dependent_tier", out),
        "gpx_dependent_tier" => extract_gpx_dependent_tier(GpxDependentTierNode(node))
            .inspect("gpx_dependent_tier", out),
        "gra_contents" => extract_gra_contents(GraContentsNode(node)).inspect("gra_contents", out),
        "gra_dependent_tier" => extract_gra_dependent_tier(GraDependentTierNode(node))
            .inspect("gra_dependent_tier", out),
        "gra_relation" => extract_gra_relation(GraRelationNode(node)).inspect("gra_relation", out),
        "group_with_annotations" => extract_group_with_annotations(GroupWithAnnotationsNode(node))
            .inspect("group_with_annotations", out),
        "header" => extract_header(node).inspect("header", out),
        "header_gap" => extract_header_gap(HeaderGapNode(node)).inspect("header_gap", out),
        "header_sep" => extract_header_sep(HeaderSepNode(node)).inspect("header_sep", out),
        "id_age" => extract_id_age(IdAgeNode(node)).inspect("id_age", out),
        "id_contents" => extract_id_contents(IdContentsNode(node)).inspect("id_contents", out),
        "id_header" => extract_id_header(IdHeaderNode(node)).inspect("id_header", out),
        "id_languages" => extract_id_languages(IdLanguagesNode(node)).inspect("id_languages", out),
        "id_ses" => extract_id_ses(IdSesNode(node)).inspect("id_ses", out),
        "id_sex" => extract_id_sex(IdSexNode(node)).inspect("id_sex", out),
        "int_dependent_tier" => extract_int_dependent_tier(IntDependentTierNode(node))
            .inspect("int_dependent_tier", out),
        "l1_of_header" => extract_l1_of_header(L1OfHeaderNode(node)).inspect("l1_of_header", out),
        "langcode" => extract_langcode(LangcodeNode(node)).inspect("langcode", out),
        "languages_contents" => extract_languages_contents(LanguagesContentsNode(node))
            .inspect("languages_contents", out),
        "languages_header" => {
            extract_languages_header(LanguagesHeaderNode(node)).inspect("languages_header", out)
        }
        "line" => extract_line(LineNode(node)).inspect("line", out),
        "linker" => extract_linker(node).inspect("linker", out),
        "linkers" => extract_linkers(LinkersNode(node)).inspect("linkers", out),
        "location_header" => {
            extract_location_header(LocationHeaderNode(node)).inspect("location_header", out)
        }
        "long_feature" => extract_long_feature(LongFeatureNode(node)).inspect("long_feature", out),
        "long_feature_begin" => extract_long_feature_begin(LongFeatureBeginNode(node))
            .inspect("long_feature_begin", out),
        "long_feature_end" => {
            extract_long_feature_end(LongFeatureEndNode(node)).inspect("long_feature_end", out)
        }
        "main_pho_group" => {
            extract_main_pho_group(MainPhoGroupNode(node)).inspect("main_pho_group", out)
        }
        "main_sin_group" => {
            extract_main_sin_group(MainSinGroupNode(node)).inspect("main_sin_group", out)
        }
        "main_tier" => extract_main_tier(MainTierNode(node)).inspect("main_tier", out),
        "media_contents" => {
            extract_media_contents(MediaContentsNode(node)).inspect("media_contents", out)
        }
        "media_filename" => {
            extract_media_filename(MediaFilenameNode(node)).inspect("media_filename", out)
        }
        "media_header" => extract_media_header(MediaHeaderNode(node)).inspect("media_header", out),
        "media_status" => extract_media_status(MediaStatusNode(node)).inspect("media_status", out),
        "media_type" => extract_media_type(MediaTypeNode(node)).inspect("media_type", out),
        "mod_dependent_tier" => extract_mod_dependent_tier(ModDependentTierNode(node))
            .inspect("mod_dependent_tier", out),
        "modsyl_dependent_tier" => extract_modsyl_dependent_tier(ModsylDependentTierNode(node))
            .inspect("modsyl_dependent_tier", out),
        "mor_content" => extract_mor_content(MorContentNode(node)).inspect("mor_content", out),
        "mor_contents" => extract_mor_contents(MorContentsNode(node)).inspect("mor_contents", out),
        "mor_dependent_tier" => extract_mor_dependent_tier(MorDependentTierNode(node))
            .inspect("mor_dependent_tier", out),
        "mor_feature" => extract_mor_feature(MorFeatureNode(node)).inspect("mor_feature", out),
        "mor_post_clitic" => {
            extract_mor_post_clitic(MorPostCliticNode(node)).inspect("mor_post_clitic", out)
        }
        "mor_word" => extract_mor_word(MorWordNode(node)).inspect("mor_word", out),
        "new_episode_header" => extract_new_episode_header(NewEpisodeHeaderNode(node))
            .inspect("new_episode_header", out),
        "non_colon_separator" => extract_non_colon_separator(NonColonSeparatorNode(node))
            .inspect("non_colon_separator", out),
        "nonvocal" => extract_nonvocal(NonvocalNode(node)).inspect("nonvocal", out),
        "nonvocal_begin" => {
            extract_nonvocal_begin(NonvocalBeginNode(node)).inspect("nonvocal_begin", out)
        }
        "nonvocal_end" => extract_nonvocal_end(NonvocalEndNode(node)).inspect("nonvocal_end", out),
        "nonvocal_simple" => {
            extract_nonvocal_simple(NonvocalSimpleNode(node)).inspect("nonvocal_simple", out)
        }
        "nonword" => extract_nonword(NonwordNode(node)).inspect("nonword", out),
        "nonword_with_optional_annotations" => {
            extract_nonword_with_optional_annotations(NonwordWithOptionalAnnotationsNode(node))
                .inspect("nonword_with_optional_annotations", out)
        }
        "number_header" => {
            extract_number_header(NumberHeaderNode(node)).inspect("number_header", out)
        }
        "number_option" => {
            extract_number_option(NumberOptionNode(node)).inspect("number_option", out)
        }
        "option_name" => extract_option_name(OptionNameNode(node)).inspect("option_name", out),
        "options_contents" => {
            extract_options_contents(OptionsContentsNode(node)).inspect("options_contents", out)
        }
        "options_header" => {
            extract_options_header(OptionsHeaderNode(node)).inspect("options_header", out)
        }
        "ort_dependent_tier" => extract_ort_dependent_tier(OrtDependentTierNode(node))
            .inspect("ort_dependent_tier", out),
        "other_spoken_event" => extract_other_spoken_event(OtherSpokenEventNode(node))
            .inspect("other_spoken_event", out),
        "page_header" => extract_page_header(PageHeaderNode(node)).inspect("page_header", out),
        "par_dependent_tier" => extract_par_dependent_tier(ParDependentTierNode(node))
            .inspect("par_dependent_tier", out),
        "para_annotation" => {
            extract_para_annotation(ParaAnnotationNode(node)).inspect("para_annotation", out)
        }
        "participant" => extract_participant(ParticipantNode(node)).inspect("participant", out),
        "participants_contents" => extract_participants_contents(ParticipantsContentsNode(node))
            .inspect("participants_contents", out),
        "participants_header" => extract_participants_header(ParticipantsHeaderNode(node))
            .inspect("participants_header", out),
        "percent_annotation" => extract_percent_annotation(PercentAnnotationNode(node))
            .inspect("percent_annotation", out),
        "pho_dependent_tier" => extract_pho_dependent_tier(PhoDependentTierNode(node))
            .inspect("pho_dependent_tier", out),
        "pho_group" => extract_pho_group(PhoGroupNode(node)).inspect("pho_group", out),
        "pho_grouped_content" => extract_pho_grouped_content(PhoGroupedContentNode(node))
            .inspect("pho_grouped_content", out),
        "pho_groups" => extract_pho_groups(PhoGroupsNode(node)).inspect("pho_groups", out),
        "pho_words" => extract_pho_words(PhoWordsNode(node)).inspect("pho_words", out),
        "phoaln_dependent_tier" => extract_phoaln_dependent_tier(PhoalnDependentTierNode(node))
            .inspect("phoaln_dependent_tier", out),
        "phosyl_dependent_tier" => extract_phosyl_dependent_tier(PhosylDependentTierNode(node))
            .inspect("phosyl_dependent_tier", out),
        "pid_header" => extract_pid_header(PidHeaderNode(node)).inspect("pid_header", out),
        "postcode" => extract_postcode(PostcodeNode(node)).inspect("postcode", out),
        "pre_begin_header" => extract_pre_begin_header(node).inspect("pre_begin_header", out),
        "quotation" => extract_quotation(QuotationNode(node)).inspect("quotation", out),
        "recording_quality_header" => {
            extract_recording_quality_header(RecordingQualityHeaderNode(node))
                .inspect("recording_quality_header", out)
        }
        "recording_quality_option" => {
            extract_recording_quality_option(RecordingQualityOptionNode(node))
                .inspect("recording_quality_option", out)
        }
        "replacement" => extract_replacement(ReplacementNode(node)).inspect("replacement", out),
        "room_layout_header" => extract_room_layout_header(RoomLayoutHeaderNode(node))
            .inspect("room_layout_header", out),
        "separator" => extract_separator(SeparatorNode(node)).inspect("separator", out),
        "shortening" => extract_shortening(ShorteningNode(node)).inspect("shortening", out),
        "sin_dependent_tier" => extract_sin_dependent_tier(SinDependentTierNode(node))
            .inspect("sin_dependent_tier", out),
        "sin_group" => extract_sin_group(SinGroupNode(node)).inspect("sin_group", out),
        "sin_grouped_content" => extract_sin_grouped_content(SinGroupedContentNode(node))
            .inspect("sin_grouped_content", out),
        "sin_groups" => extract_sin_groups(SinGroupsNode(node)).inspect("sin_groups", out),
        "sin_word" => extract_sin_word(SinWordNode(node)).inspect("sin_word", out),
        "sit_dependent_tier" => extract_sit_dependent_tier(SitDependentTierNode(node))
            .inspect("sit_dependent_tier", out),
        "situation_header" => {
            extract_situation_header(SituationHeaderNode(node)).inspect("situation_header", out)
        }
        "source_file" => extract_source_file(SourceFileNode(node)).inspect("source_file", out),
        "spa_dependent_tier" => extract_spa_dependent_tier(SpaDependentTierNode(node))
            .inspect("spa_dependent_tier", out),
        "standalone_word" => {
            extract_standalone_word(StandaloneWordNode(node)).inspect("standalone_word", out)
        }
        "t_header" => extract_t_header(THeaderNode(node)).inspect("t_header", out),
        "tape_location_header" => extract_tape_location_header(TapeLocationHeaderNode(node))
            .inspect("tape_location_header", out),
        "terminator" => extract_terminator(node).inspect("terminator", out),
        "text_with_bullets" => {
            extract_text_with_bullets(TextWithBulletsNode(node)).inspect("text_with_bullets", out)
        }
        "text_with_bullets_and_pics" => {
            extract_text_with_bullets_and_pics(TextWithBulletsAndPicsNode(node))
                .inspect("text_with_bullets_and_pics", out)
        }
        "thumbnail_header" => {
            extract_thumbnail_header(ThumbnailHeaderNode(node)).inspect("thumbnail_header", out)
        }
        "tier_body" => extract_tier_body(TierBodyNode(node)).inspect("tier_body", out),
        "tier_sep" => extract_tier_sep(TierSepNode(node)).inspect("tier_sep", out),
        "tim_dependent_tier" => extract_tim_dependent_tier(TimDependentTierNode(node))
            .inspect("tim_dependent_tier", out),
        "time_duration_contents" => extract_time_duration_contents(TimeDurationContentsNode(node))
            .inspect("time_duration_contents", out),
        "time_duration_header" => extract_time_duration_header(TimeDurationHeaderNode(node))
            .inspect("time_duration_header", out),
        "time_start_header" => {
            extract_time_start_header(TimeStartHeaderNode(node)).inspect("time_start_header", out)
        }
        "transcriber_header" => extract_transcriber_header(TranscriberHeaderNode(node))
            .inspect("transcriber_header", out),
        "transcription_header" => extract_transcription_header(TranscriptionHeaderNode(node))
            .inspect("transcription_header", out),
        "transcription_option" => extract_transcription_option(TranscriptionOptionNode(node))
            .inspect("transcription_option", out),
        "types_header" => extract_types_header(TypesHeaderNode(node)).inspect("types_header", out),
        "unsupported_dependent_tier" => {
            extract_unsupported_dependent_tier(UnsupportedDependentTierNode(node))
                .inspect("unsupported_dependent_tier", out)
        }
        "unsupported_header" => extract_unsupported_header(UnsupportedHeaderNode(node))
            .inspect("unsupported_header", out),
        "unsupported_line" => {
            extract_unsupported_line(UnsupportedLineNode(node)).inspect("unsupported_line", out)
        }
        "utf8_header" => extract_utf8_header(Utf8HeaderNode(node)).inspect("utf8_header", out),
        "utterance" => extract_utterance(UtteranceNode(node)).inspect("utterance", out),
        "utterance_end" => {
            extract_utterance_end(UtteranceEndNode(node)).inspect("utterance_end", out)
        }
        "videos_header" => {
            extract_videos_header(VideosHeaderNode(node)).inspect("videos_header", out)
        }
        "warning_header" => {
            extract_warning_header(WarningHeaderNode(node)).inspect("warning_header", out)
        }
        "window_header" => {
            extract_window_header(WindowHeaderNode(node)).inspect("window_header", out)
        }
        "wor_dependent_tier" => extract_wor_dependent_tier(WorDependentTierNode(node))
            .inspect("wor_dependent_tier", out),
        "wor_tier_body" => {
            extract_wor_tier_body(WorTierBodyNode(node)).inspect("wor_tier_body", out)
        }
        "wor_word_item" => {
            extract_wor_word_item(WorWordItemNode(node)).inspect("wor_word_item", out)
        }
        "word_body" => extract_word_body(WordBodyNode(node)).inspect("word_body", out),
        "word_with_optional_annotations" => {
            extract_word_with_optional_annotations(WordWithOptionalAnnotationsNode(node))
                .inspect("word_with_optional_annotations", out)
        }
        "x_dependent_tier" => {
            extract_x_dependent_tier(XDependentTierNode(node)).inspect("x_dependent_tier", out)
        }
        "xphoint_dependent_tier" => extract_xphoint_dependent_tier(XphointDependentTierNode(node))
            .inspect("xphoint_dependent_tier", out),
        _ => {}
    }
}
