//! Canonical diagnostic code enum for TalkBank parsing/validation.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

/// Standard error codes for CHAT parsing and validation.
///
/// This enum uses the `#[error_code_enum]` procedural macro which generates:
/// - Serde rename attributes for each variant
/// - `as_str()` method for ErrorCode -> &str conversion
/// - `new()` method for &str -> ErrorCode conversion
/// - `Display` implementation
/// - `documentation_url()` method
///
/// All mappings are generated from a single source of truth, eliminating
/// the fragile double-maintenance pattern.
#[talkbank_derive::error_code_enum]
pub enum ErrorCode {
    // =========================================================================
    // Generic/Internal Errors (E0xx, E1xx)
    // =========================================================================
    /// Internal error (unexpected condition in parser or validator).
    #[code("E001")]
    InternalError,
    /// Test-only error code used in unit tests.
    #[code("E002")]
    TestError,
    /// Input string is empty.
    #[code("E003")]
    EmptyString,

    // =========================================================================
    // Structural/File Errors (E1xx)
    // =========================================================================
    /// Invalid line format in the CHAT file.
    #[code("E101")]
    InvalidLineFormat,

    // =========================================================================
    // Parser Errors (E3xx)
    // =========================================================================
    /// Missing main tier (speaker line) in utterance.
    #[code("E301")]
    MissingMainTier,
    /// Expected tree-sitter node is missing.
    #[code("E302")]
    MissingNode,
    /// General syntax error in CHAT input.
    #[code("E303")]
    SyntaxError,
    /// Missing speaker code on main tier line.
    #[code("E304")]
    MissingSpeaker,
    /// Missing utterance terminator (`.`, `?`, `!`, etc.).
    #[code("E305")]
    MissingTerminator,
    /// Utterance contains no words or content.
    #[code("E306")]
    EmptyUtterance,
    /// Speaker code is syntactically invalid.
    #[code("E307")]
    InvalidSpeaker,
    /// Speaker code is not declared in `@Participants`.
    #[code("E308")]
    UndeclaredSpeaker,
    /// Unexpected syntax encountered during parsing.
    #[code("E309")]
    UnexpectedSyntax,
    /// Parser failed to produce a valid parse tree.
    #[code("E310")]
    ParseFailed,
    /// Unexpected node type in the parse tree.
    #[code("E311")]
    UnexpectedNode,
    /// Unclosed bracket in annotation or word content.
    #[code("E312")]
    UnclosedBracket,
    /// Unclosed parenthesis in annotation or word content.
    #[code("E313")]
    UnclosedParenthesis,
    /// Annotation is syntactically incomplete.
    #[code("E314")]
    IncompleteAnnotation,
    /// Invalid control character in input.
    #[code("E315")]
    InvalidControlCharacter,
    /// Content could not be parsed.
    #[code("E316")]
    UnparsableContent,
    /// Line could not be parsed.
    #[code("E319")]
    UnparsableLine,
    /// Header line could not be parsed.
    #[code("E320")]
    UnparsableHeader,
    /// Utterance could not be parsed.
    #[code("E321")]
    UnparsableUtterance,
    /// Empty colon with no content following it.
    #[code("E322")]
    EmptyColon,
    /// Missing colon after speaker code.
    #[code("E323")]
    MissingColonAfterSpeaker,
    /// Unrecognized error in utterance content.
    #[code("E324")]
    UnrecognizedUtteranceError,
    /// Unexpected child node in utterance.
    #[code("E325")]
    UnexpectedUtteranceChild,
    /// Unexpected line type in CHAT file.
    #[code("E326")]
    UnexpectedLineType,
    /// Error during tree-sitter CST traversal.
    #[code("E330")]
    TreeParsingError,
    /// Unexpected node encountered in a specific parsing context.
    #[code("E331")]
    UnexpectedNodeInContext,
    /// Unknown base content type in word.
    #[code("E340")]
    UnknownBaseContent,
    /// Unbalanced quotation marks spanning across utterances.
    #[code("E341")]
    UnbalancedQuotationCrossUtterance,
    /// Tree-sitter inserted a MISSING placeholder for a required element.
    #[code("E342")]
    MissingRequiredElement,
    /// Invalid nesting of scoped annotations.
    #[code("E344")]
    InvalidContentAnnotationNesting,
    /// Unmatched scoped annotation end marker.
    #[code("E346")]
    UnmatchedContentAnnotationEnd,
    /// Unbalanced overlap markers.
    #[code("E347")]
    UnbalancedOverlap,
    /// Missing overlap end marker.
    #[code("E348")]
    MissingOverlapEnd,
    /// Missing opening quotation mark.
    #[code("E351")]
    MissingQuoteBegin,
    /// Missing closing quotation mark.
    #[code("E352")]
    MissingQuoteEnd,
    /// Missing context for other-completion annotation.
    #[code("E353")]
    MissingOtherCompletionContext,
    /// Missing trailing-off terminator.
    #[code("E354")]
    MissingTrailingOffTerminator,
    /// Interleaved scoped annotations (overlapping scopes).
    #[code("E355")]
    InterleavedContentAnnotations,
    /// Unmatched underline begin marker.
    #[code("E356")]
    UnmatchedUnderlineBegin,
    /// Unmatched underline end marker.
    #[code("E357")]
    UnmatchedUnderlineEnd,
    /// Unmatched long feature begin marker.
    #[code("E358")]
    UnmatchedLongFeatureBegin,
    /// Unmatched long feature end marker.
    #[code("E359")]
    UnmatchedLongFeatureEnd,
    /// Invalid media bullet format.
    #[code("E360")]
    InvalidMediaBullet,
    /// Invalid timestamp value in media bullet.
    #[code("E361")]
    InvalidTimestamp,
    /// Timestamp end is before start (backwards range).
    #[code("E362")]
    TimestampBackwards,
    /// Invalid postcode format.
    #[code("E363")]
    InvalidPostcode,
    /// Malformed word content.
    #[code("E364")]
    MalformedWordContent,
    /// Malformed tier content.
    #[code("E365")]
    MalformedTierContent,
    /// Long feature begin/end labels do not match.
    #[code("E366")]
    LongFeatureLabelMismatch,
    /// Unmatched nonvocal begin marker.
    #[code("E367")]
    UnmatchedNonvocalBegin,
    /// Unmatched nonvocal end marker.
    #[code("E368")]
    UnmatchedNonvocalEnd,
    /// Nonvocal begin/end labels do not match.
    #[code("E369")]
    NonvocalLabelMismatch,
    /// Structural ordering error in utterance elements.
    #[code("E370")]
    StructuralOrderError,
    /// Pause marker inside a phonological group.
    #[code("E371")]
    PauseInPhoGroup,
    /// Nested quotation (quotation inside quotation).
    #[code("E372")]
    NestedQuotation,
    /// Invalid overlap index value.
    #[code("E373")]
    InvalidOverlapIndex,
    /// Failed to parse scoped annotation content.
    #[code("E375")]
    ContentAnnotationParseError,
    /// Failed to parse replacement annotation content.
    #[code("E376")]
    ReplacementParseError,
    /// Failed to parse `%mor` tier content.
    #[code("E382")]
    MorParseError,
    /// Replacement `[: text]` on fragment or phonological fragment (`&+`).
    #[code("E387")]
    ReplacementOnFragment,
    /// Replacement `[: text]` on nonword (`&~`).
    #[code("E388")]
    ReplacementOnNonword,
    /// Replacement `[: text]` on filler (`&-`).
    #[code("E389")]
    ReplacementOnFiller,
    /// Replacement text contains an omission (`0word`).
    #[code("E390")]
    ReplacementContainsOmission,
    /// Replacement text contains untranscribed marker (`xxx`/`yyy`/`www`).
    #[code("E391")]
    ReplacementContainsUntranscribed,

    // =========================================================================
    // Word Errors (E2xx)
    // =========================================================================
    /// Missing form type on special word.
    #[code("E202")]
    MissingFormType,
    /// Invalid form type value.
    #[code("E203")]
    InvalidFormType,
    /// Unknown annotation type in word.
    #[code("E207")]
    UnknownAnnotation,
    /// Replacement annotation is empty.
    #[code("E208")]
    EmptyReplacement,
    /// Spoken content portion of word is empty.
    #[code("E209")]
    EmptySpokenContent,
    /// Illegal replacement for fragment (deprecated: use E387).
    #[code("E210")]
    IllegalReplacementForFragment,
    /// Invalid word format.
    #[code("E212")]
    InvalidWordFormat,
    /// Untranscribed marker in replacement (deprecated: use E391).
    #[code("E213")]
    UntranscribedInReplacement,
    /// Empty annotated scoped annotations.
    #[code("E214")]
    EmptyAnnotatedContentAnnotations,
    /// Illegal digits in word content.
    #[code("E220")]
    IllegalDigits,
    /// Unbalanced CA (Conversation Analysis) delimiter.
    #[code("E230")]
    UnbalancedCADelimiter,
    /// Unbalanced shortening markers.
    #[code("E231")]
    UnbalancedShortening,
    /// Invalid compound marker position.
    #[code("E232")]
    InvalidCompoundMarkerPosition,
    /// Empty part in compound word.
    #[code("E233")]
    EmptyCompoundPart,
    /// Illegal use of untranscribed marker.
    #[code("E241")]
    IllegalUntranscribed,
    /// Unbalanced quotation marks within a word.
    #[code("E242")]
    UnbalancedQuotation,
    /// Illegal characters in word content.
    #[code("E243")]
    IllegalCharactersInWord,
    /// Consecutive stress markers in word.
    #[code("E244")]
    ConsecutiveStressMarkers,
    /// Stress marker not placed before spoken material.
    #[code("E245")]
    StressNotBeforeSpokenMaterial,
    /// Lengthening marker not placed after spoken material.
    #[code("E246")]
    LengtheningNotAfterSpokenMaterial,
    /// Multiple primary stress markers in one word.
    #[code("E247")]
    MultiplePrimaryStress,
    /// Tertiary language needs an explicit language code.
    #[code("E248")]
    TertiaryLanguageNeedsExplicitCode,
    /// Missing language context for language-tagged word.
    #[code("E249")]
    MissingLanguageContext,
    /// Secondary stress marker without primary stress.
    #[code("E250")]
    SecondaryStressWithoutPrimary,
    /// Word content text is empty.
    #[code("E251")]
    EmptyWordContentText,
    /// Syllable pause not between spoken material.
    #[code("E252")]
    SyllablePauseNotBetweenSpokenMaterial,
    /// Word content is empty.
    #[code("E253")]
    EmptyWordContent,
    // E254 (UndeclaredExplicitWordLanguage) was RETIRED 2026-07-15: an
    // explicit word-level `@s:CODE` deliberately carries no requirement to
    // be declared in `@Languages` (docs/design/2026-07-15-at-s-language-
    // declaration-decision.md part 1). The number is not reused.
    /// Whole-utterance language switch should use `[- LANG]` instead of tagging every word with `@s`.
    #[code("E255")]
    WholeUtteranceLanguageSwitchShouldUsePrecode,
    /// Curly single quotation mark (U+2018 or U+2019) used as a word
    /// character; CHAT requires the ASCII apostrophe. CLAN CHECK 138/139.
    #[code("E256")]
    IllegalCurlyQuote,
    /// Consecutive commas (`,,`), should use single comma or `‚` (CLAN CHECK 107)
    #[code("E258")]
    ConsecutiveCommas,
    /// Comma after non-spoken content (paralinguistic event, filler, nonword, placeholder, omitted word)
    #[code("E259")]
    CommaAfterNonSpokenContent,

    // =========================================================================
    // Dependent Tier Structural Errors (E4xx)
    // =========================================================================
    /// Duplicate dependent tier on same utterance.
    #[code("E401")]
    DuplicateDependentTier,
    /// Dependent tier without a preceding main tier.
    #[code("E404")]
    OrphanedDependentTier,

    // =========================================================================
    // Header Errors (E5xx)
    // =========================================================================
    /// Duplicate header line.
    #[code("E501")]
    DuplicateHeader,
    /// Missing `@End` header.
    #[code("E502")]
    MissingEndHeader,
    /// Missing `@UTF8` header.
    #[code("E503")]
    MissingUTF8Header,
    /// Missing required header (e.g., `@Participants`).
    #[code("E504")]
    MissingRequiredHeader,
    /// Invalid `@ID` header format.
    #[code("E505")]
    InvalidIDFormat,
    /// Empty `@Participants` header.
    #[code("E506")]
    EmptyParticipantsHeader,
    /// Empty `@Languages` header.
    #[code("E507")]
    EmptyLanguagesHeader,
    /// Empty `@Date` header.
    #[code("E508")]
    EmptyDateHeader,
    /// Empty `@Media` header.
    #[code("E509")]
    EmptyMediaHeader,
    /// Empty language field in `@ID` header.
    #[code("E510")]
    EmptyIDLanguage,
    /// Empty speaker field in `@ID` header.
    #[code("E511")]
    EmptyIDSpeaker,
    /// Empty participant code in `@Participants`.
    #[code("E512")]
    EmptyParticipantCode,
    /// Empty participant role in `@Participants`.
    #[code("E513")]
    EmptyParticipantRole,
    /// Empty corpus field (2nd field) in `@ID` header. The corpus name is
    /// required: `@ID:` is `lang|corpus|code|...`, and a blank corpus is
    /// invalid. Corresponds to CLAN CHECK error 63 ("Missing Corpus name").
    #[code("E514")]
    EmptyIDCorpus,
    /// Empty role field in `@ID` header.
    #[code("E515")]
    EmptyIDRole,
    /// Empty date value.
    #[code("E516")]
    EmptyDate,
    /// Invalid age format in `@ID` header.
    #[code("E517")]
    InvalidAgeFormat,
    /// Invalid date format.
    #[code("E518")]
    InvalidDateFormat,
    /// Invalid ISO 639 language code.
    #[code("E519")]
    InvalidLanguageCode,
    /// Speaker code not defined in `@Participants`.
    #[code("E522")]
    SpeakerNotDefined,
    /// `@ID` header references an undeclared participant.
    #[code("E523")]
    OrphanIDHeader,
    /// `@Birth` header references an unknown participant.
    #[code("E524")]
    BirthUnknownParticipant,
    /// Unknown or unrecognized header type.
    #[code("E525")]
    UnknownHeader,
    /// Unmatched `@Bg` (begin gem) without corresponding `@Eg`.
    #[code("E526")]
    UnmatchedBeginGem,
    /// Unmatched `@Eg` (end gem) without corresponding `@Bg`.
    #[code("E527")]
    UnmatchedEndGem,
    /// Gem begin/end labels do not match.
    #[code("E528")]
    GemLabelMismatch,
    /// Nested `@Bg` (begin gem inside existing gem scope).
    #[code("E529")]
    NestedBeginGem,
    /// Lazy gem (`@G`) used inside an explicit gem scope.
    #[code("E530")]
    LazyGemInsideScope,
    /// `@Media` filename does not match the file being parsed.
    #[code("E531")]
    MediaFilenameMismatch,
    /// Invalid participant role value.
    #[code("E532")]
    InvalidParticipantRole,
    /// Empty `@Options` header.
    #[code("E533")]
    EmptyOptionsHeader,
    /// Unsupported `@Options` value.
    #[code("E534")]
    UnsupportedOption,
    /// Unsupported `@Media` type (not `audio`, `video`, or `missing`).
    #[code("E535")]
    UnsupportedMediaType,
    /// Unsupported `@Media` status value.
    #[code("E536")]
    UnsupportedMediaStatus,
    /// Unsupported `@Number` value.
    #[code("E537")]
    UnsupportedNumber,
    /// Unsupported `@Recording Quality` value.
    #[code("E538")]
    UnsupportedRecordingQuality,
    /// Unsupported `@Transcription` value.
    #[code("E539")]
    UnsupportedTranscription,
    /// Invalid `@Time Duration` format.
    #[code("E540")]
    InvalidTimeDuration,
    /// Invalid `@Time Start` format.
    #[code("E541")]
    InvalidTimeStart,
    /// Unsupported `@ID` sex value (not `male` or `female`).
    #[code("E542")]
    UnsupportedSex,
    /// Header out of canonical order (e.g., `@Options` before `@Participants`).
    #[code("E543")]
    HeaderOutOfOrder,
    /// `@Media` header declares linkage (no `unlinked` / `missing` /
    /// `notrans` status) but the transcript carries no timing evidence
    /// (no bullets on any utterance, no `%wor` bullets, no `@Bg`/`@Eg`
    /// time range). Reinstates the legacy CHAT validation check.
    #[code("E544")]
    MediaLinkageWithoutTiming,
    /// Invalid `@Birth of` date format, must match `DD-MMM-YYYY`
    /// per CLAN `depfile.cut`'s `@d<dd-lll-yyyy>` template.
    #[code("E545")]
    InvalidBirthDateFormat,
    /// Unsupported `@ID` SES value.
    #[code("E546")]
    UnsupportedSesValue,
    /// A constant participant-specific header (`@Birth of`, `@Birthplace of`,
    /// or `@L1 of`) does not immediately follow the `@ID` block: a changeable
    /// header (e.g. `@Comment`, `@Date`) appears between the `@ID` headers and
    /// the constant header. Per the CHAT manual these constant headers must
    /// directly follow `@ID`. Corresponds to CLAN CHECK error 127.
    #[code("E547")]
    ConstantHeaderOutOfOrder,
    /// An `@ID` header does not immediately follow the `@Participants` /
    /// `@Options` headers (or another `@ID`): a changeable header such as
    /// `@Comment` appears between `@Participants`/`@Options` and the `@ID`
    /// block. Per the CHAT manual the `@ID` block directly follows
    /// `@Participants` (and the optional `@Options`). Corresponds to CLAN
    /// CHECK error 126.
    #[code("E548")]
    IdHeaderOutOfOrder,
    /// The same speaker code is declared more than once in the `@Participants`
    /// header. Each participant must be declared exactly once. Corresponds to
    /// CLAN CHECK error 13.
    #[code("E549")]
    DuplicateSpeakerDeclaration,
    /// The `@Participants` header ends with a trailing comma (a stray comma
    /// after the last participant, with no participant following it). The
    /// participant list is comma-separated, so a trailing comma is a dangling
    /// separator rather than an empty header. Corresponds to CLAN CHECK error
    /// 100 ("Commas at the end of PARTICIPANTS tier are not allowed").
    #[code("E550")]
    TrailingCommaInParticipants,
    /// The `@Options` header does not immediately follow the `@Participants`
    /// header. Per the CHAT spec the optional `@Options` line, when present,
    /// must sit directly after `@Participants`, before the `@ID` block or any
    /// other header. An `@Options` whose immediately-preceding header is
    /// something else (e.g. an `@ID` or `@Comment`), once `@Participants` has
    /// been seen, is an ordering violation. Corresponds to CLAN CHECK error 125
    /// ("\"@Options\" header must immediately follow \"@Participants:\" header").
    #[code("E551")]
    OptionsHeaderOutOfOrder,
    /// The `@Media` header declares `unlinked` status, yet the transcript
    /// contains timing bullets. `unlinked` means the media file exists but its
    /// utterances have not been aligned to timestamps; the presence of timing
    /// bullets contradicts that, so the `unlinked` qualifier must be removed
    /// (the media is in fact linked). This is the inverse of
    /// [`MediaLinkageWithoutTiming`](Self::MediaLinkageWithoutTiming) (E544) and
    /// corresponds to CLAN CHECK error 124 ("remove \"unlinked\" from @Media
    /// header").
    #[code("E552")]
    MediaUnlinkedWithTiming,

    // =========================================================================
    // Tier Errors (E6xx)
    // =========================================================================
    /// Generic tier validation error.
    #[code("E600")]
    TierValidationError,
    /// Invalid dependent tier name or format.
    #[code("E601")]
    InvalidDependentTier,
    /// Malformed tier header line.
    #[code("E602")]
    MalformedTierHeader,
    /// Invalid `%tim` tier format.
    #[code("E603")]
    InvalidTimTierFormat,
    /// `%gra` tier present without corresponding `%mor` tier.
    #[code("E604")]
    GraWithoutMor,
    /// Unsupported dependent tier (not a standard `%` tier or `%x` user-defined tier).
    #[code("E605")]
    UnsupportedDependentTier,

    // =========================================================================
    // Temporal/Media Bullet Errors (E7xx)
    // =========================================================================
    /// Unexpected node in tier content.
    #[code("E700")]
    UnexpectedTierNode,
    /// Tier begin time is not monotonically increasing (CLAN Error 83).
    #[code("E701")]
    TierBeginTimeNotMonotonic,
    /// Invalid morphology format on `%mor` tier.
    #[code("E702")]
    InvalidMorphologyFormat,
    /// Unexpected node in morphology tier.
    #[code("E703")]
    UnexpectedMorphologyNode,
    /// Speaker overlaps with themselves (CLAN Error 133).
    #[code("E704")]
    SpeakerSelfOverlap,
    /// `%mor` tier has fewer words than main tier.
    #[code("E705")]
    MorCountMismatchTooFew,
    /// `%mor` tier has more words than main tier.
    #[code("E706")]
    MorCountMismatchTooMany,
    /// `%mor` tier terminator presence does not match main tier.
    #[code("E707")]
    MorTerminatorPresenceMismatch,
    /// Malformed grammar relation on `%gra` tier.
    #[code("E708")]
    MalformedGrammarRelation,
    /// Invalid index in grammar relation.
    #[code("E709")]
    InvalidGrammarIndex,
    /// Unexpected node in `%gra` tier.
    #[code("E710")]
    UnexpectedGrammarNode,
    /// `%mor` word has empty stem, POS category, prefix, or suffix.
    #[code("E711")]
    MorEmptyContent,
    /// `%gra` word index is out of range.
    #[code("E712")]
    GraInvalidWordIndex,
    /// `%gra` head index is out of range.
    #[code("E713")]
    GraInvalidHeadIndex,
    /// `%pho`, `%mod`, or `%wor` tier has fewer alignable words than main tier.
    #[code("E714")]
    PhoCountMismatchTooFew,
    /// `%pho`, `%mod`, or `%wor` tier has more alignable words than main tier.
    #[code("E715")]
    PhoCountMismatchTooMany,
    /// `%mor` tier terminator value does not match main tier.
    #[code("E716")]
    MorTerminatorValueMismatch,
    /// `%sin` tier has fewer words than main tier.
    #[code("E718")]
    SinCountMismatchTooFew,
    /// `%sin` tier has more words than main tier.
    #[code("E719")]
    SinCountMismatchTooMany,
    /// `%mor` and `%gra` tier word counts do not match.
    #[code("E720")]
    MorGraCountMismatch,
    /// `%gra` indices are not sequential.
    #[code("E721")]
    GraNonSequentialIndex,
    /// `%gra` tier has no ROOT relation.
    #[code("E722")]
    GraNoRoot,
    /// `%gra` tier has multiple ROOT relations.
    #[code("E723")]
    GraMultipleRoots,
    /// `%gra` tier contains a circular dependency.
    #[code("E724")]
    GraCircularDependency,
    /// `%modsyl` tier word count does not match `%mod` tier.
    #[code("E725")]
    ModsylModCountMismatch,
    /// `%phosyl` tier word count does not match `%pho` tier.
    #[code("E726")]
    PhosylPhoCountMismatch,
    /// `%phoaln` tier word count does not match `%mod` tier.
    #[code("E727")]
    PhoalnModCountMismatch,
    /// `%phoaln` tier word count does not match `%pho` tier.
    #[code("E728")]
    PhoalnPhoCountMismatch,
    /// Bullet start time overlaps with previous tier's end time (CLAN Error 84).
    ///
    /// Current tier's BEG is less than the previous tier's END, indicating
    /// overlapping timing. Unlike speaker self-overlap (E704), this applies
    /// across different speakers.
    #[code("E729")]
    BulletOverlap,
    /// Bullet timing gap exceeds threshold (CLAN Error 85).
    ///
    /// Gap between current tier's BEG and previous tier's END exceeds the
    /// acceptable discontinuity threshold. Only reported in bullet consistency
    /// mode (`+c0`).
    #[code("E730")]
    BulletGap,
    /// Speaker's bullet start time is before their own previous bullet end time
    /// (CLAN Error 133).
    ///
    /// Supplements the overlap-marker-based E704 with actual bullet timing
    /// check for same-speaker self-overlap.
    #[code("E731")]
    SpeakerBulletSelfOverlap,
    /// Missing bullet on tier when bullet consistency mode is active (CLAN Error 110).
    ///
    /// When `+c0` or `+c1` is specified, every main tier must have timing.
    #[code("E732")]
    MissingBullet,
    /// `%mod` tier has fewer words than main tier.
    ///
    /// The model-phonology tier (`%mod`) has fewer alignable tokens than the
    /// main-tier words. Each main-tier word must have a corresponding `%mod`
    /// token. This code is separate from E714 (`%pho`) because the two tiers
    /// represent distinct phonological layers.
    #[code("E733")]
    ModCountMismatchTooFew,
    /// `%mod` tier has more words than main tier.
    ///
    /// The model-phonology tier (`%mod`) has more alignable tokens than the
    /// main-tier words. Remove the extra `%mod` tokens so counts match.
    /// This code is separate from E715 (`%pho`) for the same reason as E733.
    #[code("E734")]
    ModCountMismatchTooMany,
    /// A `%xmodsyl` / `%xphosyl` syllabification unit is not a `phone:CODE` pair.
    ///
    /// A unit must be one IPA phone, an ASCII `:`, then a single constituent
    /// code. A unit with no `:`, an empty phone, or an empty code is invalid.
    #[code("E735")]
    SylUnitMalformed,
    /// A `%xmodsyl` / `%xphosyl` constituent code is not one of `O N C L R E A D U`.
    ///
    /// `U` (unknown) is legal: Phon emits it when a phone has no concrete
    /// syllable constituent. Boundary (`B`), stress (`S`), word (`W`) and tone
    /// (`T`) are never emitted on the syllabification tiers.
    #[code("E736")]
    SylIllegalConstituentCode,
    /// `%xmodsyl` does not reproduce its `%mod` word.
    ///
    /// Stripping the `:CODE` from every unit and concatenating the phones must
    /// equal the corresponding `%mod` word exactly.
    #[code("E737")]
    ModsylReconstructionMismatch,
    /// `%xphosyl` does not reproduce its `%pho` word.
    ///
    /// Stripping the `:CODE` from every unit and concatenating the phones must
    /// equal the corresponding `%pho` word exactly.
    #[code("E738")]
    PhosylReconstructionMismatch,
    /// A `%xphoaln` alignment pair is malformed.
    ///
    /// Every pair must contain exactly one `↔` with a non-empty side on each
    /// side (use `∅` for a null); a pair of `∅↔∅` is invalid.
    #[code("E739")]
    PhoalnPairMalformed,
    /// `%xphoaln` model sides do not reproduce the `%mod` word.
    ///
    /// Concatenating the left element of each pair, skipping `∅`, must equal the
    /// corresponding `%mod` word exactly.
    #[code("E740")]
    PhoalnModReconstructionMismatch,
    /// `%xphoaln` actual sides do not reproduce the `%pho` word.
    ///
    /// Concatenating the right element of each pair, skipping `∅`, must equal the
    /// corresponding `%pho` word exactly.
    #[code("E741")]
    PhoalnPhoReconstructionMismatch,
    /// A `%xphoint` time bullet has `start >= end`.
    ///
    /// Each phone interval's start offset must be strictly less than its end.
    #[code("E742")]
    XphointBulletInvalid,
    /// `%xphoint` interval start times are not non-decreasing.
    ///
    /// Each interval's start must be greater than or equal to the previous
    /// interval's start across the whole tier.
    #[code("E743")]
    XphointIntervalNotMonotonic,
    /// `%xphoint` intervals fall outside the record's media interval.
    ///
    /// The first interval's start and last interval's end must lie within the
    /// `*SPK:` line's media bullet (subject to 1 ms rounding tolerance).
    #[code("E744")]
    XphointMediaBoundsViolation,
    /// `%xphoint` group phones do not reproduce the `%pho` word.
    ///
    /// Concatenating a group's phones, in order, must equal the corresponding
    /// `%pho` word exactly.
    #[code("E745")]
    XphointPhoneReconstructionMismatch,
    /// `%xphoint` group count does not match the `%pho` word count.
    ///
    /// There must be exactly one ` / `-separated group per `%pho` word.
    #[code("E746")]
    XphointGroupCountMismatch,

    /// A blank line in the transcript.
    ///
    /// CHAT does not allow blank lines anywhere in the transcript (CLAN CHECK
    /// 91). The grammar represents a blank line as a structural `blank_line`
    /// node (the single-break `newline` token no longer fuses consecutive
    /// newlines), so the parser emits this from the tree, not by scanning the
    /// source text.
    #[code("E747")]
    BlankLineNotAllowed,

    /// A media bullet timestamp written with a leading zero before another
    /// digit (e.g. `\u{15}012_200\u{15}`).
    ///
    /// Bullet times are plain millisecond integers; CLAN CHECK rejects a
    /// component matching `0[0-9]` as an "Illegal time representation inside
    /// a bullet." (CHECK 90). A bare `0` component is legal. Detected at
    /// parse time because the leading zero exists only in the source text
    /// (the model stores `u64` milliseconds); the bullet's numeric value
    /// still parses, so the AST keeps the bullet while the diagnostic makes
    /// the file invalid.
    #[code("E748")]
    LeadingZeroBulletTime,

    /// A comma glued to the word that follows it (`hey ,you`).
    ///
    /// A comma on a speaker tier must be followed by a space or
    /// end-of-line (CLAN CHECK 92). Detected by span adjacency over the
    /// in-order content walk: the rule fires only when the next item is
    /// a word starting at the byte immediately after the comma, so
    /// exempt constructs that put any other character there (group `<`,
    /// overlap and CA marks) are naturally not flagged.
    #[code("E749")]
    CommaGluedToNextWord,

    /// A space directly after the opening `<` or directly before the
    /// closing `>` of an angle-bracket group (`< dog>` / `<dog >`).
    ///
    /// Group delimiters hug their content (CLAN CHECK 160). The grammar
    /// tolerates the whitespace as an explicit optional `whitespaces`
    /// CST node so the parse recovers; this diagnostic marks the file
    /// invalid instead of silently dropping the space (which also
    /// silently rewrote the text on normalize).
    #[code("E750")]
    SpaceInsideAngleGroup,

    /// A pause marker glued to the end of the preceding word
    /// (`hello(.)`).
    ///
    /// Pauses are free-standing, space-delimited items (CLAN CHECK 57).
    /// Detected by span adjacency over the in-order content walk: the
    /// pause's span starts at the byte where the previous word's span
    /// ends.
    #[code("E751")]
    PauseGluedToWord,

    /// Transcript carries timing evidence (main-tier bullets or a
    /// positional `%wor` timing sidecar) but no `@Media` header declares
    /// the media timeline the timestamps index. Inverse direction of
    /// [`MediaLinkageWithoutTiming`](Self::MediaLinkageWithoutTiming)
    /// (E544); corresponds to CLAN CHECK error 112.
    #[code("E752")]
    TimingWithoutMedia,

    /// Word consists only of a repetition segment: every material part
    /// sits inside `↫...↫` (U+21AB) segment-repetition delimiters with no
    /// stem outside. The notation marks a repeated segment OF a word, so
    /// a fully wrapped word asserts a repetition of nothing. Adopted from
    /// GUI CLAN CHECK error 151 as a chatter-authority rule; a
    /// word-category prefix marker (`&-` filler etc.) counts as a stem.
    #[code("E753")]
    WordOnlyRepetitionSegments,

    /// The `@l` letter form marks a single spoken letter, but the word's
    /// stem has more than one character. Sequences belong under `@k`
    /// (letter sequence) or `@ls` (letter plural). Replicates CLAN CHECK
    /// error 76 (`check_isOneLetter`); the digraph question (one letter
    /// orthographically, two characters) is deferred, not decided here.
    #[code("E754")]
    LetterFormMultipleLetters,

    /// A `[- CODE]` utterance-level language is not declared in
    /// `@Languages`. Utterance-level presence is substantial, unlike a
    /// word-level `@s:CODE` insertion, which deliberately carries no
    /// declaration requirement (2026-07-15 ruling). Matches CLAN CHECK
    /// error 152.
    #[code("E755")]
    UndeclaredUtteranceLanguage,

    /// User-defined `%x` tier has empty or whitespace-only content: the
    /// line declares an annotation that is not there. Formerly W601 (which
    /// fired as a hard error despite the warning prefix); renumbered
    /// 2026-07-16, rejection unchanged.
    #[code("E756")]
    EmptyUserDefinedTier,

    /// Bracketed code's closing `]` glued to the following content with
    /// no space (`hello [/]x`). Style rule; the parse is unambiguous but
    /// codes are free-standing space-delimited items. Matches CLAN CHECK
    /// error 19.
    #[code("E757")]
    CodeGluedToFollowingContent,

    /// Space between the tier's tab delimiter and the first content item
    /// in a file WITHOUT `@Options: CA` (`*CHI:<tab><space>dog .`). CA
    /// transcripts legitimately column-align with spaces after the tab
    /// (all 457 wild occurrences are CA files, 2026-07-16 scan), so the
    /// CA option exempts the rule. Matches CLAN CHECK error 123.
    #[code("E758")]
    LeadingSpaceOnMainTier,

    // =========================================================================
    // Warnings (Wxxx)
    // =========================================================================
    /// Speaker code not found in `@Participants` (non-fatal).
    #[code("W108")]
    SpeakerNotFoundInParticipants,
    // W210 (missing whitespace before content, e.g. a glued terminator
    // `hello.`) and W211 (missing whitespace after an overlap marker)
    // were RETIRED on 2026-07-16 (maintainer ruling): no production
    // code ever emitted them (their check was removed from the
    // main-tier path long ago, see the leniency-policy book page,
    // Decision 8), real CLAN CHECK accepts the W210 construct, and
    // overlap markers hug their content by design, so W211's shape is
    // valid CA notation. The numbers are retired and not reused.
    // W601 (empty user-defined tier) was RENUMBERED to E756 on 2026-07-16:
    // it always fired as a hard error, so the warning-prefixed code was the
    // bug (maintainer ruling). W601 is retired and not reused.
    // W602 (deprecated %xLABEL) was DELETED the same day: the Phon %x-tier
    // fold routes every known label to typed tier parsers, so the check was
    // dead code. W602 is retired and not reused.
    /// Legacy warning from older CHAT validation.
    #[code("W999")]
    LegacyWarning,

    // =========================================================================
    // Generic/Unknown (MUST be last for fallback in new())
    // =========================================================================
    /// Unknown or unrecognized error code (fallback).
    #[code("E999")]
    UnknownError,
}

/// The Phon `%x` dependent-tier validation codes, as one group.
///
/// Single source of truth for "which error codes are Phon `%x` validation": the
/// word-count cross-checks (E725-E728) plus the content checks (E735-E746). It
/// lives next to the code definitions so the two cannot drift; the CLI exposes
/// it to users under the `xphon` suppress-group name. When you add a Phon `%x`
/// check, add its code here.
pub const XPHON_ERROR_CODES: &[ErrorCode] = &[
    ErrorCode::ModsylModCountMismatch,             // E725
    ErrorCode::PhosylPhoCountMismatch,             // E726
    ErrorCode::PhoalnModCountMismatch,             // E727
    ErrorCode::PhoalnPhoCountMismatch,             // E728
    ErrorCode::SylUnitMalformed,                   // E735
    ErrorCode::SylIllegalConstituentCode,          // E736
    ErrorCode::ModsylReconstructionMismatch,       // E737
    ErrorCode::PhosylReconstructionMismatch,       // E738
    ErrorCode::PhoalnPairMalformed,                // E739
    ErrorCode::PhoalnModReconstructionMismatch,    // E740
    ErrorCode::PhoalnPhoReconstructionMismatch,    // E741
    ErrorCode::XphointBulletInvalid,               // E742
    ErrorCode::XphointIntervalNotMonotonic,        // E743
    ErrorCode::XphointMediaBoundsViolation,        // E744
    ErrorCode::XphointPhoneReconstructionMismatch, // E745
    ErrorCode::XphointGroupCountMismatch,          // E746
];
