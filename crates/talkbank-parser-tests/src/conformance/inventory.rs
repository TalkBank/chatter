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
//! `just conformance-gen`.
//! The staleness guard `conformance_inventory_is_current` re-derives this file
//! from the current typed traversal + node-types.json and fails if the
//! committed copy has drifted, so a forgotten regen breaks the suite instead of
//! silently losing coverage. The harness lives in the parent module
//! `conformance`; the conformance test's allowlist in
//! `tests/integration/generated_traversal_conformance.rs`.

#![allow(clippy::too_many_lines)]

use crate::classify;
use crate::generated_traversal::*;

use super::{Inspect, InspectField, Observation, Position};

/// Generate a no-op `Inspect` for a leaf node wrapper: its own node kind is
/// separately visited by `walk_all` and dispatched below, so there is nothing
/// further to recurse into from here.
macro_rules! impl_inspect_leaf {
    ($($name:ident),* $(,)?) => {
        $(
            impl<'tree> Inspect for $name<'tree> {
                fn inspect(&self, _rule: &'static str, _out: &mut Vec<Observation>) {}
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
            fn inspect(&self, rule: &'static str, out: &mut Vec<Observation>) {
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
            fn inspect(&self, rule: &'static str, out: &mut Vec<Observation>) {
                $( self.$field.inspect_field(
                    rule,
                    Position::new(stringify!($name), stringify!($field)),
                    out,
                ); )*
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
    CodeSwitchAnnotationNode,
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
    LBrackAtSNode,
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
    QuotationWithOptionalAnnotationsNode,
    QuotedNewLineNode,
    QuotedPeriodSimpleNode,
    RParenNode,
    RecordingQualityHeaderNode,
    RecordingQualityOptionNode,
    RecordingQualityPrefixNode,
    RepeatedFormMarkerNode,
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
    SepTrailingSpaceNode,
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
    CodeSwitchAnnotation,
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
    CodeSwitchAnnotation,
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
    CodeSwitchAnnotation,
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
impl_inspect_choice!(AdmittedBaseAnnotationsChild0Child1Choice {
    AltAnnotation,
    CodeSwitchAnnotation,
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
impl_inspect_choice!(AdmittedBaseAnnotationsChild1Child1Choice {
    AltAnnotation,
    CodeSwitchAnnotation,
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
impl_inspect_choice!(AdmittedBaseContentItemChoice {
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
impl_inspect_choice!(ContentItemCaNoBreakLinkerChoice {
    CaNoBreakLinker,
    CaTechnicalBreakLinker,
    LinkerLazyOverlap,
    LinkerQuickUptake,
    LinkerQuickUptakeOverlap,
    LinkerQuotationFollows,
    LinkerSelfCompletion
});
impl_inspect_choice!(ContentItemChoice {
    BaseContentItem,
    GroupWithAnnotations,
    QuotationWithOptionalAnnotations,
    IllegalCurlyQuote,
    MainPhoGroup,
    MainSinGroup,
    CaNoBreakLinker
});
impl_inspect_choice!(AdmittedContentItemCaNoBreakLinkerChoice {
    CaNoBreakLinker,
    CaTechnicalBreakLinker,
    LinkerLazyOverlap,
    LinkerQuickUptake,
    LinkerQuickUptakeOverlap,
    LinkerQuotationFollows,
    LinkerSelfCompletion
});
impl_inspect_choice!(AdmittedContentItemChoice {
    BaseContentItem,
    GroupWithAnnotations,
    QuotationWithOptionalAnnotations,
    IllegalCurlyQuote,
    MainPhoGroup,
    MainSinGroup,
    CaNoBreakLinker
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
impl_inspect_choice!(AdmittedContentsChild0Choice {
    Whitespaces,
    ContentItem,
    Separator,
    OverlapPoint
});
impl_inspect_choice!(AdmittedContentsChild1Choice {
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
impl_inspect_choice!(AdmittedFullDocumentChild1Choice {
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
impl_inspect_choice!(AdmittedIdLanguagesChoice {
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
impl_inspect_choice!(AdmittedLineActivitiesHeaderChoice {
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
impl_inspect_choice!(AdmittedLineChoice {
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
impl_inspect_choice!(AdmittedLongFeatureChoice {
    LongFeatureBegin,
    LongFeatureEnd
});
impl_inspect_choice!(MediaFilenameChoice {
    DoubleQuote,
    RNTRNRNT
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
impl_inspect_choice!(AdmittedMorContentsChild0MorContentChild2Child1Choice {
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
impl_inspect_choice!(AdmittedMorContentsChild0BreakForCodingChoice {
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
impl_inspect_choice!(AdmittedMorContentsChild0Choice {
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
impl_inspect_choice!(AdmittedNonvocalChoice {
    NonvocalBegin,
    NonvocalEnd,
    NonvocalSimple
});
impl_inspect_choice!(NonwordChoice { Event, Zero });
impl_inspect_choice!(AdmittedNonwordChoice { Event, Zero });
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
impl_inspect_choice!(AdmittedPhoGroupChoice {
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
impl_inspect_choice!(AdmittedSeparatorChoice {
    NonColonSeparator,
    Colon
});
impl_inspect_choice!(SinGroupChoice {
    SinWord,
    SinBeginGroup
});
impl_inspect_choice!(AdmittedSinGroupChoice {
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
impl_inspect_choice!(AdmittedSourceFileActDependentTierChoice {
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
impl_inspect_choice!(AdmittedSourceFileActivitiesHeaderChoice {
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
impl_inspect_choice!(AdmittedSourceFileColorWordsHeaderChoice {
    ColorWordsHeader,
    FontHeader,
    PidHeader,
    WindowHeader
});
impl_inspect_choice!(AdmittedSourceFileChoice {
    FullDocument,
    Utterance,
    MainTier,
    ActDependentTier,
    ActivitiesHeader,
    ColorWordsHeader,
    StandaloneWord
});
impl_inspect_choice!(StandaloneWordChild0Choice { WordPrefix, Zero });
impl_inspect_choice!(StandaloneWordChild2Choice {
    FormMarker,
    RepeatedFormMarker
});
impl_inspect_choice!(AdmittedStandaloneWordChild0Choice { WordPrefix, Zero });
impl_inspect_choice!(AdmittedStandaloneWordChild2Choice {
    FormMarker,
    RepeatedFormMarker
});
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
impl_inspect_choice!(AdmittedTextWithBulletsChild0Choice {
    TextSegment,
    Bullet,
    Continuation
});
impl_inspect_choice!(AdmittedTextWithBulletsChild1Choice {
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
impl_inspect_choice!(AdmittedTextWithBulletsAndPicsChild0Choice {
    TextSegment,
    Bullet,
    InlinePic,
    Continuation
});
impl_inspect_choice!(AdmittedTextWithBulletsAndPicsChild1Choice {
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
impl_inspect_choice!(AdmittedUtteranceChild1Choice {
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
impl_inspect_choice!(AdmittedUtteranceEndChild0Choice {
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
impl_inspect_choice!(AdmittedWorTierBodyChild1Child0Choice {
    WorWordItem,
    Bullet,
    Comma,
    TagMarker,
    VocativeMarker
});
impl_inspect_choice!(AdmittedWorTierBodyChild2Choice {
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
impl_inspect_choice!(AdmittedWordBodyWordSegmentChild0Choice {
    WordSegment,
    Shortening,
    StressMarker
});
impl_inspect_choice!(AdmittedWordBodyWordSegmentChild1LengtheningChoice {
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
impl_inspect_choice!(AdmittedWordBodyWordSegmentChild1Choice {
    WordSegment,
    Shortening,
    StressMarker,
    Lengthening
});
impl_inspect_choice!(AdmittedWordBodyOverlapPointChild0Choice {
    OverlapPoint,
    CaElement,
    CaDelimiter,
    UnderlineBegin,
    SyllablePause
});
impl_inspect_choice!(AdmittedWordBodyOverlapPointChild1Choice {
    OverlapPoint,
    CaElement,
    CaDelimiter,
    UnderlineBegin,
    SyllablePause
});
impl_inspect_choice!(AdmittedWordBodyOverlapPointChild2Choice {
    WordSegment,
    Shortening,
    StressMarker
});
impl_inspect_choice!(AdmittedWordBodyOverlapPointChild3LengtheningChoice {
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
impl_inspect_choice!(AdmittedWordBodyOverlapPointChild3Choice {
    WordSegment,
    Shortening,
    StressMarker,
    Lengthening
});
impl_inspect_choice!(AdmittedWordBodyChoice {
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
impl_inspect_struct!(AdmittedActDependentTierChildren {
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
impl_inspect_struct!(AdmittedActivitiesHeaderChildren {
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
impl_inspect_struct!(AdmittedAddDependentTierChildren {
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
impl_inspect_struct!(AdmittedAltDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(BaseAnnotationChildren { content });
impl_inspect_struct!(BaseAnnotationsChild0Children { child_0, child_1 });
impl_inspect_struct!(BaseAnnotationsChild1Children { child_0, child_1 });
impl_inspect_struct!(BaseAnnotationsChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedBaseAnnotationsChild0Children { child_0, child_1 });
impl_inspect_struct!(AdmittedBaseAnnotationsChild1Children { child_0, child_1 });
impl_inspect_struct!(AdmittedBaseAnnotationsChildren { child_0, child_1 });
impl_inspect_struct!(BaseContentItemChildren { content });
impl_inspect_struct!(AdmittedBaseContentItemChildren { content });
impl_inspect_struct!(BckHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedBckHeaderChildren {
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
impl_inspect_struct!(AdmittedBgHeaderChild1Children { child_0, child_1 });
impl_inspect_struct!(AdmittedBgHeaderChildren {
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
impl_inspect_struct!(AdmittedBirthOfHeaderChildren {
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
impl_inspect_struct!(AdmittedBirthplaceOfHeaderChildren {
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
impl_inspect_struct!(AdmittedCodDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(CodeSwitchAnnotationChild1Children { child_0, code });
impl_inspect_struct!(CodeSwitchAnnotationChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(CohDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedCohDependentTierChildren {
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
impl_inspect_struct!(AdmittedColorWordsHeaderChildren {
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
impl_inspect_struct!(AdmittedComDependentTierChildren {
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
impl_inspect_struct!(AdmittedCommentHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(ContentItemChildren { content });
impl_inspect_struct!(AdmittedContentItemChildren { content });
impl_inspect_struct!(ContentsChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedContentsChildren { child_0, child_1 });
impl_inspect_struct!(DateContentsChildren { content });
impl_inspect_struct!(DateHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedDateHeaderChildren {
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
impl_inspect_struct!(AdmittedDefDependentTierChildren {
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
impl_inspect_struct!(AdmittedEgHeaderChild1Children { child_0, child_1 });
impl_inspect_struct!(AdmittedEgHeaderChildren {
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
impl_inspect_struct!(AdmittedEngDependentTierChildren {
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
impl_inspect_struct!(AdmittedErrDependentTierChildren {
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
impl_inspect_struct!(AdmittedExpDependentTierChildren {
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
impl_inspect_struct!(AdmittedFacDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(FinalCodesChild0Children { child_0, child_1 });
impl_inspect_struct!(FinalCodesChild1Children { child_0, child_1 });
impl_inspect_struct!(FinalCodesChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedFinalCodesChild0Children { child_0, child_1 });
impl_inspect_struct!(AdmittedFinalCodesChild1Children { child_0, child_1 });
impl_inspect_struct!(AdmittedFinalCodesChildren { child_0, child_1 });
impl_inspect_struct!(FloDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedFloDependentTierChildren {
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
impl_inspect_struct!(AdmittedFontHeaderChildren {
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
impl_inspect_struct!(AdmittedFullDocumentChildren {
    child_0,
    child_1,
    child_2,
    child_3,
    child_4
});
impl_inspect_struct!(GHeaderChild1Children { child_0, child_1 });
impl_inspect_struct!(GHeaderChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(AdmittedGHeaderChild1Children { child_0, child_1 });
impl_inspect_struct!(AdmittedGHeaderChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(GlsDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedGlsDependentTierChildren {
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
impl_inspect_struct!(AdmittedGpxDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(GraContentsChild1Children { child_0, child_1 });
impl_inspect_struct!(GraContentsChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedGraContentsChild1Children { child_0, child_1 });
impl_inspect_struct!(AdmittedGraContentsChildren { child_0, child_1 });
impl_inspect_struct!(GraDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedGraDependentTierChildren {
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
impl_inspect_struct!(AdmittedGroupWithAnnotationsChildren {
    child_0,
    content_2,
    child_2,
    annotations
});
impl_inspect_struct!(HeaderChildren { content });
impl_inspect_struct!(HeaderGapChildren { child_0, child_1 });
impl_inspect_struct!(HeaderSepChildren {
    child_0,
    child_1,
    child_2
});
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
impl_inspect_struct!(AdmittedIdContentsChildren {
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
impl_inspect_struct!(AdmittedIdHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(IdLanguagesChildren { content });
impl_inspect_struct!(AdmittedIdLanguagesChildren { content });
impl_inspect_struct!(IdSesChildren { content });
impl_inspect_struct!(IdSexChildren { content });
impl_inspect_struct!(IntDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedIntDependentTierChildren {
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
impl_inspect_struct!(AdmittedL1OfHeaderChildren {
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
impl_inspect_struct!(AdmittedLanguagesHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(LineChildren { content });
impl_inspect_struct!(AdmittedLineChildren { content });
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
impl_inspect_struct!(AdmittedLocationHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(LongFeatureChildren { content });
impl_inspect_struct!(AdmittedLongFeatureChildren { content });
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
impl_inspect_struct!(AdmittedMainPhoGroupChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(MainSinGroupChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(AdmittedMainSinGroupChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(MainTierChildren {
    child_0,
    speaker,
    child_2,
    child_3,
    child_4,
    child_5
});
impl_inspect_struct!(AdmittedMainTierChildren {
    child_0,
    speaker,
    child_2,
    child_3,
    child_4,
    child_5
});
impl_inspect_struct!(MediaContentsChild5Children {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(MediaContentsChildren {
    child_0,
    child_1,
    child_2,
    child_3,
    child_4,
    child_5
});
impl_inspect_struct!(AdmittedMediaContentsChild5Children {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(AdmittedMediaContentsChildren {
    child_0,
    child_1,
    child_2,
    child_3,
    child_4,
    child_5
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
impl_inspect_struct!(AdmittedMediaHeaderChildren {
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
impl_inspect_struct!(AdmittedModDependentTierChildren {
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
impl_inspect_struct!(AdmittedModsylDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(MorContentChildren { main, post_clitics });
impl_inspect_struct!(AdmittedMorContentChildren { main, post_clitics });
impl_inspect_struct!(MorContentsChild0MorContentChild1Children { child_0, child_1 });
impl_inspect_struct!(MorContentsChild0MorContentChild2Children { child_0, child_1 });
impl_inspect_struct!(MorContentsChild0MorContentChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(MorContentsChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedMorContentsChild0MorContentChild1Children { child_0, child_1 });
impl_inspect_struct!(AdmittedMorContentsChild0MorContentChild2Children { child_0, child_1 });
impl_inspect_struct!(AdmittedMorContentsChild0MorContentChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(AdmittedMorContentsChildren { child_0, child_1 });
impl_inspect_struct!(MorDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedMorDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(MorFeatureChildren { child_0, child_1 });
impl_inspect_struct!(MorPostCliticChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedMorPostCliticChildren { child_0, child_1 });
impl_inspect_struct!(MorWordChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedMorWordChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(NewEpisodeHeaderChildren { child_0, child_1 });
impl_inspect_struct!(NonColonSeparatorChildren { content });
impl_inspect_struct!(NonvocalChildren { content });
impl_inspect_struct!(AdmittedNonvocalChildren { content });
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
impl_inspect_struct!(AdmittedNonwordChildren { content });
impl_inspect_struct!(NonwordWithOptionalAnnotationsChildren {
    nonword,
    annotations
});
impl_inspect_struct!(AdmittedNonwordWithOptionalAnnotationsChildren {
    nonword,
    annotations
});
impl_inspect_struct!(NumberHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedNumberHeaderChildren {
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
impl_inspect_struct!(AdmittedOptionsContentsChild1Children {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(AdmittedOptionsContentsChildren { child_0, child_1 });
impl_inspect_struct!(OptionsHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedOptionsHeaderChildren {
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
impl_inspect_struct!(AdmittedOrtDependentTierChildren {
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
impl_inspect_struct!(AdmittedOtherSpokenEventChildren {
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
impl_inspect_struct!(AdmittedPageHeaderChildren {
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
impl_inspect_struct!(AdmittedParDependentTierChildren {
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
impl_inspect_struct!(AdmittedParticipantsContentsChild1Children {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(AdmittedParticipantsContentsChildren { child_0, child_1 });
impl_inspect_struct!(ParticipantsHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedParticipantsHeaderChildren {
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
impl_inspect_struct!(AdmittedPhoDependentTierChildren {
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
impl_inspect_struct!(AdmittedPhoGroupPhoBeginGroupChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(AdmittedPhoGroupChildren { content });
impl_inspect_struct!(PhoGroupedContentChild1Children { child_0, child_1 });
impl_inspect_struct!(PhoGroupedContentChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedPhoGroupedContentChild1Children { child_0, child_1 });
impl_inspect_struct!(AdmittedPhoGroupedContentChildren { child_0, child_1 });
impl_inspect_struct!(PhoGroupsChild1Children { child_0, child_1 });
impl_inspect_struct!(PhoGroupsChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedPhoGroupsChild1Children { child_0, child_1 });
impl_inspect_struct!(AdmittedPhoGroupsChildren { child_0, child_1 });
impl_inspect_struct!(PhoWordsChild1Children { child_0, child_1 });
impl_inspect_struct!(PhoWordsChildren { child_0, child_1 });
impl_inspect_struct!(PhoalnDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedPhoalnDependentTierChildren {
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
impl_inspect_struct!(AdmittedPhosylDependentTierChildren {
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
impl_inspect_struct!(AdmittedPidHeaderChildren {
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
impl_inspect_struct!(AdmittedQuotationChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(QuotationWithOptionalAnnotationsChildren {
    quotation,
    annotations
});
impl_inspect_struct!(AdmittedQuotationWithOptionalAnnotationsChildren {
    quotation,
    annotations
});
impl_inspect_struct!(RecordingQualityHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedRecordingQualityHeaderChildren {
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
impl_inspect_struct!(AdmittedReplacementChild2Children { child_0, child_1 });
impl_inspect_struct!(AdmittedReplacementChild3Children { child_0, child_1 });
impl_inspect_struct!(AdmittedReplacementChildren {
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
impl_inspect_struct!(AdmittedRoomLayoutHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(SeparatorChildren { content });
impl_inspect_struct!(AdmittedSeparatorChildren { content });
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
impl_inspect_struct!(AdmittedSinDependentTierChildren {
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
impl_inspect_struct!(AdmittedSinGroupSinBeginGroupChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(AdmittedSinGroupChildren { content });
impl_inspect_struct!(SinGroupedContentChild1Children { child_0, child_1 });
impl_inspect_struct!(SinGroupedContentChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedSinGroupedContentChild1Children { child_0, child_1 });
impl_inspect_struct!(AdmittedSinGroupedContentChildren { child_0, child_1 });
impl_inspect_struct!(SinGroupsChild1Children { child_0, child_1 });
impl_inspect_struct!(SinGroupsChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedSinGroupsChild1Children { child_0, child_1 });
impl_inspect_struct!(AdmittedSinGroupsChildren { child_0, child_1 });
impl_inspect_struct!(SinWordChildren { content });
impl_inspect_struct!(SitDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedSitDependentTierChildren {
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
impl_inspect_struct!(AdmittedSituationHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(SourceFileChildren { content });
impl_inspect_struct!(AdmittedSourceFileChildren { content });
impl_inspect_struct!(SpaDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedSpaDependentTierChildren {
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
impl_inspect_struct!(AdmittedStandaloneWordChildren {
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
impl_inspect_struct!(AdmittedTHeaderChildren {
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
impl_inspect_struct!(AdmittedTapeLocationHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(TerminatorChildren { content });
impl_inspect_struct!(TextWithBulletsChild0BulletChildren { child_0, child_1 });
impl_inspect_struct!(TextWithBulletsChild1BulletChildren { child_0, child_1 });
impl_inspect_struct!(TextWithBulletsChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedTextWithBulletsChild0BulletChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedTextWithBulletsChild1BulletChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedTextWithBulletsChildren { child_0, child_1 });
impl_inspect_struct!(TextWithBulletsAndPicsChild0BulletChildren { child_0, child_1 });
impl_inspect_struct!(TextWithBulletsAndPicsChild0InlinePicChildren { child_0, child_1 });
impl_inspect_struct!(TextWithBulletsAndPicsChild1BulletChildren { child_0, child_1 });
impl_inspect_struct!(TextWithBulletsAndPicsChild1InlinePicChildren { child_0, child_1 });
impl_inspect_struct!(TextWithBulletsAndPicsChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedTextWithBulletsAndPicsChild0BulletChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedTextWithBulletsAndPicsChild0InlinePicChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedTextWithBulletsAndPicsChild1BulletChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedTextWithBulletsAndPicsChild1InlinePicChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedTextWithBulletsAndPicsChildren { child_0, child_1 });
impl_inspect_struct!(ThumbnailHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedThumbnailHeaderChildren {
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
impl_inspect_struct!(AdmittedTierBodyLanguageCodeChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedTierBodyChildren {
    linkers,
    language_code,
    content_2,
    ending
});
impl_inspect_struct!(TierSepChildren {
    child_0,
    child_1,
    child_2
});
impl_inspect_struct!(TimDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedTimDependentTierChildren {
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
impl_inspect_struct!(AdmittedTimeDurationHeaderChildren {
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
impl_inspect_struct!(AdmittedTimeStartHeaderChildren {
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
impl_inspect_struct!(AdmittedTranscriberHeaderChildren {
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
impl_inspect_struct!(AdmittedTranscriptionHeaderChildren {
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
impl_inspect_struct!(AdmittedTypesHeaderChildren {
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
impl_inspect_struct!(AdmittedUnsupportedDependentTierChildren {
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
impl_inspect_struct!(AdmittedUnsupportedHeaderChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(UnsupportedLineChildren { child_0, child_1 });
impl_inspect_struct!(Utf8HeaderChildren { child_0, child_1 });
impl_inspect_struct!(UtteranceChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedUtteranceChildren { child_0, child_1 });
impl_inspect_struct!(UtteranceEndChild2Children { child_0, child_1 });
impl_inspect_struct!(UtteranceEndChildren {
    child_0,
    child_1,
    child_2,
    child_3,
    child_4
});
impl_inspect_struct!(AdmittedUtteranceEndChild2Children { child_0, child_1 });
impl_inspect_struct!(AdmittedUtteranceEndChildren {
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
impl_inspect_struct!(AdmittedVideosHeaderChildren {
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
impl_inspect_struct!(AdmittedWarningHeaderChildren {
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
impl_inspect_struct!(AdmittedWindowHeaderChildren {
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
impl_inspect_struct!(AdmittedWorDependentTierChildren {
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
impl_inspect_struct!(AdmittedWorTierBodyLanguageCodeChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedWorTierBodyChild1Children { child_0, child_1 });
impl_inspect_struct!(AdmittedWorTierBodyChildren {
    language_code,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(WorWordItemChildren { content });
impl_inspect_struct!(AdmittedWorWordItemChildren { content });
impl_inspect_struct!(WordBodyWordSegmentChildren { child_0, child_1 });
impl_inspect_struct!(WordBodyOverlapPointChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(WordBodyChildren { content });
impl_inspect_struct!(AdmittedWordBodyWordSegmentChildren { child_0, child_1 });
impl_inspect_struct!(AdmittedWordBodyOverlapPointChildren {
    child_0,
    child_1,
    child_2,
    child_3
});
impl_inspect_struct!(AdmittedWordBodyChildren { content });
impl_inspect_struct!(WordWithOptionalAnnotationsChild1Children {
    child_0,
    replacement
});
impl_inspect_struct!(WordWithOptionalAnnotationsChildren {
    word,
    child_1,
    annotations
});
impl_inspect_struct!(AdmittedWordWithOptionalAnnotationsChild1Children {
    child_0,
    replacement
});
impl_inspect_struct!(AdmittedWordWithOptionalAnnotationsChildren {
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
impl_inspect_struct!(AdmittedXDependentTierChildren {
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
impl_inspect_struct!(AdmittedXphointDependentTierChildren {
    child_0,
    child_1,
    child_2,
    child_3
});

/// Drive the generated `extract_*` for `node` if its kind has one, then
/// inspect the returned children. One arm per `extract_*` free function.
// Conformance assertions must fail the test on producer faults.
#[allow(clippy::expect_used)]
pub fn dispatch(node: tree_sitter::Node, out: &mut Vec<Observation>) {
    match node.kind() {
        "act_dependent_tier" => extract_act_dependent_tier(classify::<ActDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("act_dependent_tier", out),
        "activities_header" => extract_activities_header(classify::<ActivitiesHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("activities_header", out),
        "add_dependent_tier" => extract_add_dependent_tier(classify::<AddDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("add_dependent_tier", out),
        "alt_annotation" => extract_alt_annotation(classify::<AltAnnotationNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("alt_annotation", out),
        "alt_dependent_tier" => extract_alt_dependent_tier(classify::<AltDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("alt_dependent_tier", out),
        "base_annotation" => extract_base_annotation(node).inspect("base_annotation", out),
        "base_annotations" => extract_base_annotations(classify::<BaseAnnotationsNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("base_annotations", out),
        "base_content_item" => extract_base_content_item(classify::<BaseContentItemNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("base_content_item", out),
        "bck_header" => extract_bck_header(classify::<BckHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("bck_header", out),
        "begin_header" => extract_begin_header(classify::<BeginHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("begin_header", out),
        "bg_header" => extract_bg_header(classify::<BgHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("bg_header", out),
        "birth_of_header" => extract_birth_of_header(classify::<BirthOfHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("birth_of_header", out),
        "birthplace_of_header" => {
            extract_birthplace_of_header(classify::<BirthplaceOfHeaderNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("birthplace_of_header", out)
        }
        "blank_header" => extract_blank_header(classify::<BlankHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("blank_header", out),
        "blank_line" => extract_blank_line(classify::<BlankLineNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("blank_line", out),
        "bullet" => extract_bullet(classify::<BulletNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("bullet", out),
        "cod_dependent_tier" => extract_cod_dependent_tier(classify::<CodDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("cod_dependent_tier", out),
        "code_switch_annotation" => {
            extract_code_switch_annotation(classify::<CodeSwitchAnnotationNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("code_switch_annotation", out)
        }
        "coh_dependent_tier" => extract_coh_dependent_tier(classify::<CohDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("coh_dependent_tier", out),
        "color_words_header" => extract_color_words_header(classify::<ColorWordsHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("color_words_header", out),
        "com_dependent_tier" => extract_com_dependent_tier(classify::<ComDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("com_dependent_tier", out),
        "comment_header" => extract_comment_header(classify::<CommentHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("comment_header", out),
        "content_item" => extract_content_item(classify::<ContentItemNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("content_item", out),
        "contents" => extract_contents(classify::<ContentsNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("contents", out),
        "date_contents" => extract_date_contents(classify::<DateContentsNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("date_contents", out),
        "date_header" => extract_date_header(classify::<DateHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("date_header", out),
        "def_dependent_tier" => extract_def_dependent_tier(classify::<DefDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("def_dependent_tier", out),
        "dependent_tier" => extract_dependent_tier(node).inspect("dependent_tier", out),
        "eg_header" => extract_eg_header(classify::<EgHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("eg_header", out),
        "end_header" => extract_end_header(classify::<EndHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("end_header", out),
        "eng_dependent_tier" => extract_eng_dependent_tier(classify::<EngDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("eng_dependent_tier", out),
        "err_dependent_tier" => extract_err_dependent_tier(classify::<ErrDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("err_dependent_tier", out),
        "event" => extract_event(classify::<EventNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("event", out),
        "exp_dependent_tier" => extract_exp_dependent_tier(classify::<ExpDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("exp_dependent_tier", out),
        "explanation_annotation" => {
            extract_explanation_annotation(classify::<ExplanationAnnotationNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("explanation_annotation", out)
        }
        "fac_dependent_tier" => extract_fac_dependent_tier(classify::<FacDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("fac_dependent_tier", out),
        "final_codes" => extract_final_codes(classify::<FinalCodesNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("final_codes", out),
        "flo_dependent_tier" => extract_flo_dependent_tier(classify::<FloDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("flo_dependent_tier", out),
        "font_header" => extract_font_header(classify::<FontHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("font_header", out),
        "free_text" => extract_free_text(classify::<FreeTextNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("free_text", out),
        "full_document" => extract_full_document(classify::<FullDocumentNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("full_document", out),
        "g_header" => extract_g_header(classify::<GHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("g_header", out),
        "gls_dependent_tier" => extract_gls_dependent_tier(classify::<GlsDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("gls_dependent_tier", out),
        "gpx_dependent_tier" => extract_gpx_dependent_tier(classify::<GpxDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("gpx_dependent_tier", out),
        "gra_contents" => extract_gra_contents(classify::<GraContentsNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("gra_contents", out),
        "gra_dependent_tier" => extract_gra_dependent_tier(classify::<GraDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("gra_dependent_tier", out),
        "gra_relation" => extract_gra_relation(classify::<GraRelationNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("gra_relation", out),
        "group_with_annotations" => {
            extract_group_with_annotations(classify::<GroupWithAnnotationsNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("group_with_annotations", out)
        }
        "header" => extract_header(node).inspect("header", out),
        "header_gap" => extract_header_gap(classify::<HeaderGapNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("header_gap", out),
        "header_sep" => extract_header_sep(classify::<HeaderSepNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("header_sep", out),
        "id_age" => extract_id_age(classify::<IdAgeNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("id_age", out),
        "id_contents" => extract_id_contents(classify::<IdContentsNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("id_contents", out),
        "id_header" => extract_id_header(classify::<IdHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("id_header", out),
        "id_languages" => extract_id_languages(classify::<IdLanguagesNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("id_languages", out),
        "id_ses" => extract_id_ses(classify::<IdSesNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("id_ses", out),
        "id_sex" => extract_id_sex(classify::<IdSexNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("id_sex", out),
        "int_dependent_tier" => extract_int_dependent_tier(classify::<IntDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("int_dependent_tier", out),
        "l1_of_header" => extract_l1_of_header(classify::<L1OfHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("l1_of_header", out),
        "langcode" => extract_langcode(classify::<LangcodeNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("langcode", out),
        "languages_contents" => extract_languages_contents(classify::<LanguagesContentsNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("languages_contents", out),
        "languages_header" => extract_languages_header(classify::<LanguagesHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("languages_header", out),
        "line" => extract_line(classify::<LineNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("line", out),
        "linker" => extract_linker(node).inspect("linker", out),
        "linkers" => extract_linkers(classify::<LinkersNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("linkers", out),
        "location_header" => extract_location_header(classify::<LocationHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("location_header", out),
        "long_feature" => extract_long_feature(classify::<LongFeatureNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("long_feature", out),
        "long_feature_begin" => extract_long_feature_begin(classify::<LongFeatureBeginNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("long_feature_begin", out),
        "long_feature_end" => extract_long_feature_end(classify::<LongFeatureEndNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("long_feature_end", out),
        "main_pho_group" => extract_main_pho_group(classify::<MainPhoGroupNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("main_pho_group", out),
        "main_sin_group" => extract_main_sin_group(classify::<MainSinGroupNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("main_sin_group", out),
        "main_tier" => extract_main_tier(classify::<MainTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("main_tier", out),
        "media_contents" => extract_media_contents(classify::<MediaContentsNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("media_contents", out),
        "media_filename" => extract_media_filename(classify::<MediaFilenameNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("media_filename", out),
        "media_header" => extract_media_header(classify::<MediaHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("media_header", out),
        "media_status" => extract_media_status(classify::<MediaStatusNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("media_status", out),
        "media_type" => extract_media_type(classify::<MediaTypeNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("media_type", out),
        "mod_dependent_tier" => extract_mod_dependent_tier(classify::<ModDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("mod_dependent_tier", out),
        "modsyl_dependent_tier" => {
            extract_modsyl_dependent_tier(classify::<ModsylDependentTierNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("modsyl_dependent_tier", out)
        }
        "mor_content" => extract_mor_content(classify::<MorContentNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("mor_content", out),
        "mor_contents" => extract_mor_contents(classify::<MorContentsNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("mor_contents", out),
        "mor_dependent_tier" => extract_mor_dependent_tier(classify::<MorDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("mor_dependent_tier", out),
        "mor_feature" => extract_mor_feature(classify::<MorFeatureNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("mor_feature", out),
        "mor_post_clitic" => extract_mor_post_clitic(classify::<MorPostCliticNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("mor_post_clitic", out),
        "mor_word" => extract_mor_word(classify::<MorWordNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("mor_word", out),
        "new_episode_header" => extract_new_episode_header(classify::<NewEpisodeHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("new_episode_header", out),
        "non_colon_separator" => {
            extract_non_colon_separator(classify::<NonColonSeparatorNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("non_colon_separator", out)
        }
        "nonvocal" => extract_nonvocal(classify::<NonvocalNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("nonvocal", out),
        "nonvocal_begin" => extract_nonvocal_begin(classify::<NonvocalBeginNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("nonvocal_begin", out),
        "nonvocal_end" => extract_nonvocal_end(classify::<NonvocalEndNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("nonvocal_end", out),
        "nonvocal_simple" => extract_nonvocal_simple(classify::<NonvocalSimpleNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("nonvocal_simple", out),
        "nonword" => extract_nonword(classify::<NonwordNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("nonword", out),
        "nonword_with_optional_annotations" => extract_nonword_with_optional_annotations(
            classify::<NonwordWithOptionalAnnotationsNode>(node),
        )
        .expect("generated reconstruction must preserve its selected plan")
        .inspect("nonword_with_optional_annotations", out),
        "number_header" => extract_number_header(classify::<NumberHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("number_header", out),
        "number_option" => extract_number_option(classify::<NumberOptionNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("number_option", out),
        "option_name" => extract_option_name(classify::<OptionNameNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("option_name", out),
        "options_contents" => extract_options_contents(classify::<OptionsContentsNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("options_contents", out),
        "options_header" => extract_options_header(classify::<OptionsHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("options_header", out),
        "ort_dependent_tier" => extract_ort_dependent_tier(classify::<OrtDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("ort_dependent_tier", out),
        "other_spoken_event" => extract_other_spoken_event(classify::<OtherSpokenEventNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("other_spoken_event", out),
        "page_header" => extract_page_header(classify::<PageHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("page_header", out),
        "par_dependent_tier" => extract_par_dependent_tier(classify::<ParDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("par_dependent_tier", out),
        "para_annotation" => extract_para_annotation(classify::<ParaAnnotationNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("para_annotation", out),
        "participant" => extract_participant(classify::<ParticipantNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("participant", out),
        "participants_contents" => {
            extract_participants_contents(classify::<ParticipantsContentsNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("participants_contents", out)
        }
        "participants_header" => {
            extract_participants_header(classify::<ParticipantsHeaderNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("participants_header", out)
        }
        "percent_annotation" => extract_percent_annotation(classify::<PercentAnnotationNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("percent_annotation", out),
        "pho_dependent_tier" => extract_pho_dependent_tier(classify::<PhoDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("pho_dependent_tier", out),
        "pho_group" => extract_pho_group(classify::<PhoGroupNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("pho_group", out),
        "pho_grouped_content" => {
            extract_pho_grouped_content(classify::<PhoGroupedContentNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("pho_grouped_content", out)
        }
        "pho_groups" => extract_pho_groups(classify::<PhoGroupsNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("pho_groups", out),
        "pho_words" => extract_pho_words(classify::<PhoWordsNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("pho_words", out),
        "phoaln_dependent_tier" => {
            extract_phoaln_dependent_tier(classify::<PhoalnDependentTierNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("phoaln_dependent_tier", out)
        }
        "phosyl_dependent_tier" => {
            extract_phosyl_dependent_tier(classify::<PhosylDependentTierNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("phosyl_dependent_tier", out)
        }
        "pid_header" => extract_pid_header(classify::<PidHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("pid_header", out),
        "postcode" => extract_postcode(classify::<PostcodeNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("postcode", out),
        "pre_begin_header" => extract_pre_begin_header(node).inspect("pre_begin_header", out),
        "quotation" => extract_quotation(classify::<QuotationNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("quotation", out),
        "quotation_with_optional_annotations" => {
            extract_quotation_with_optional_annotations(classify::<
                QuotationWithOptionalAnnotationsNode,
            >(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("quotation_with_optional_annotations", out)
        }
        "recording_quality_header" => {
            extract_recording_quality_header(classify::<RecordingQualityHeaderNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("recording_quality_header", out)
        }
        "recording_quality_option" => {
            extract_recording_quality_option(classify::<RecordingQualityOptionNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("recording_quality_option", out)
        }
        "replacement" => extract_replacement(classify::<ReplacementNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("replacement", out),
        "room_layout_header" => extract_room_layout_header(classify::<RoomLayoutHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("room_layout_header", out),
        "separator" => extract_separator(classify::<SeparatorNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("separator", out),
        "shortening" => extract_shortening(classify::<ShorteningNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("shortening", out),
        "sin_dependent_tier" => extract_sin_dependent_tier(classify::<SinDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("sin_dependent_tier", out),
        "sin_group" => extract_sin_group(classify::<SinGroupNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("sin_group", out),
        "sin_grouped_content" => {
            extract_sin_grouped_content(classify::<SinGroupedContentNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("sin_grouped_content", out)
        }
        "sin_groups" => extract_sin_groups(classify::<SinGroupsNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("sin_groups", out),
        "sin_word" => extract_sin_word(classify::<SinWordNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("sin_word", out),
        "sit_dependent_tier" => extract_sit_dependent_tier(classify::<SitDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("sit_dependent_tier", out),
        "situation_header" => extract_situation_header(classify::<SituationHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("situation_header", out),
        "source_file" => extract_source_file(classify::<SourceFileNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("source_file", out),
        "spa_dependent_tier" => extract_spa_dependent_tier(classify::<SpaDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("spa_dependent_tier", out),
        "standalone_word" => extract_standalone_word(classify::<StandaloneWordNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("standalone_word", out),
        "t_header" => extract_t_header(classify::<THeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("t_header", out),
        "tape_location_header" => {
            extract_tape_location_header(classify::<TapeLocationHeaderNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("tape_location_header", out)
        }
        "terminator" => extract_terminator(node).inspect("terminator", out),
        "text_with_bullets" => extract_text_with_bullets(classify::<TextWithBulletsNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("text_with_bullets", out),
        "text_with_bullets_and_pics" => {
            extract_text_with_bullets_and_pics(classify::<TextWithBulletsAndPicsNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("text_with_bullets_and_pics", out)
        }
        "thumbnail_header" => extract_thumbnail_header(classify::<ThumbnailHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("thumbnail_header", out),
        "tier_body" => extract_tier_body(classify::<TierBodyNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("tier_body", out),
        "tier_sep" => extract_tier_sep(classify::<TierSepNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("tier_sep", out),
        "tim_dependent_tier" => extract_tim_dependent_tier(classify::<TimDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("tim_dependent_tier", out),
        "time_duration_contents" => {
            extract_time_duration_contents(classify::<TimeDurationContentsNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("time_duration_contents", out)
        }
        "time_duration_header" => {
            extract_time_duration_header(classify::<TimeDurationHeaderNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("time_duration_header", out)
        }
        "time_start_header" => extract_time_start_header(classify::<TimeStartHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("time_start_header", out),
        "transcriber_header" => extract_transcriber_header(classify::<TranscriberHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("transcriber_header", out),
        "transcription_header" => {
            extract_transcription_header(classify::<TranscriptionHeaderNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("transcription_header", out)
        }
        "transcription_option" => {
            extract_transcription_option(classify::<TranscriptionOptionNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("transcription_option", out)
        }
        "types_header" => extract_types_header(classify::<TypesHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("types_header", out),
        "unsupported_dependent_tier" => {
            extract_unsupported_dependent_tier(classify::<UnsupportedDependentTierNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("unsupported_dependent_tier", out)
        }
        "unsupported_header" => extract_unsupported_header(classify::<UnsupportedHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("unsupported_header", out),
        "unsupported_line" => extract_unsupported_line(classify::<UnsupportedLineNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("unsupported_line", out),
        "utf8_header" => extract_utf8_header(classify::<Utf8HeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("utf8_header", out),
        "utterance" => extract_utterance(classify::<UtteranceNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("utterance", out),
        "utterance_end" => extract_utterance_end(classify::<UtteranceEndNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("utterance_end", out),
        "videos_header" => extract_videos_header(classify::<VideosHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("videos_header", out),
        "warning_header" => extract_warning_header(classify::<WarningHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("warning_header", out),
        "window_header" => extract_window_header(classify::<WindowHeaderNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("window_header", out),
        "wor_dependent_tier" => extract_wor_dependent_tier(classify::<WorDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("wor_dependent_tier", out),
        "wor_tier_body" => extract_wor_tier_body(classify::<WorTierBodyNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("wor_tier_body", out),
        "wor_word_item" => extract_wor_word_item(classify::<WorWordItemNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("wor_word_item", out),
        "word_body" => extract_word_body(classify::<WordBodyNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("word_body", out),
        "word_with_optional_annotations" => extract_word_with_optional_annotations(classify::<
            WordWithOptionalAnnotationsNode,
        >(node))
        .expect("generated reconstruction must preserve its selected plan")
        .inspect("word_with_optional_annotations", out),
        "x_dependent_tier" => extract_x_dependent_tier(classify::<XDependentTierNode>(node))
            .expect("generated reconstruction must preserve its selected plan")
            .inspect("x_dependent_tier", out),
        "xphoint_dependent_tier" => {
            extract_xphoint_dependent_tier(classify::<XphointDependentTierNode>(node))
                .expect("generated reconstruction must preserve its selected plan")
                .inspect("xphoint_dependent_tier", out)
        }
        _ => {}
    }
}
