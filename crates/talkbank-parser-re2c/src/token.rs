//! Token types emitted by the re2c lexer.
//!
//! Each variant corresponds to a grammar.js rule or token.
//! Names are taken directly from grammar.js where possible.

mod inspection;
mod postcode;
mod prefix;
pub use inspection::{LexError, LexResult};
pub use postcode::PostcodeToken;
pub use prefix::{DependentBodyKind, DependentPrefixToken, PrefixToken};

use serde::Serialize;
use strum::EnumDiscriminants;

/// Token returned by the lexer. Each variant carries
/// a borrowed slice of the source input (zero-copy).
#[derive(Clone, Debug, PartialEq, EnumDiscriminants, Serialize)]
pub enum Token<'a> {
    // ── Structure ───────────────────────────────────────────
    /// BOM (byte order mark) at start of file.
    BOM(&'a str),
    /// One logical line break: /\r\n|[\r\n]/.
    Newline(&'a str),
    /// Continuation: /[\r\n]+\t/ (newline followed by tab)
    Continuation(&'a str),
    /// Whitespace: one or more spaces.
    Whitespace(&'a str),

    // ── Headers ─────────────────────────────────────────────
    /// Header prefix: @HeaderName:\t (includes the colon+tab for structured headers)
    /// Or just @HeaderName for no-content headers or catch-all.
    HeaderPrefix(PrefixToken<'a>),
    /// Header separator: ":\t" (only for unknown headers via HEADER_AFTER_NAME)
    HeaderSep(PrefixToken<'a>),
    /// Header content: free text after colon+tab
    HeaderContent(&'a str),

    /// `@Birth of SPK:\t`, carries tag-extracted speaker code.
    HeaderBirthOf(PrefixToken<'a>),
    /// `@Birthplace of SPK:\t`, carries tag-extracted speaker code.
    HeaderBirthplaceOf(PrefixToken<'a>),
    /// `@L1 of SPK:\t`, carries tag-extracted speaker code.
    HeaderL1Of(PrefixToken<'a>),

    // ── No-content headers (distinct tokens) ────────────────
    /// @UTF8 header (must be first line)
    HeaderUtf8(&'a str),
    /// @Begin header (marks start of main content)
    HeaderBegin(&'a str),
    /// @End header (marks end of file)
    HeaderEnd(&'a str),
    /// @Blank header
    HeaderBlank(&'a str),
    /// @New Episode header
    HeaderNewEpisode(&'a str),

    // ── Structured header content ───────────────────────────
    /// @ID rich token: 10 pipe-delimited fields extracted by tags.
    IdFields {
        language: &'a str,
        corpus: &'a str,
        speaker: &'a str,
        age: &'a str,
        sex: &'a str,
        group: &'a str,
        ses: &'a str,
        role: &'a str,
        education: &'a str,
        custom: &'a str,
    },

    /// @Types rich token: 3 comma-separated fields extracted by tags.
    TypesFields {
        design: &'a str,
        activity: &'a str,
        group: &'a str,
    },

    // ── Main tier ───────────────────────────────────────────
    /// Star: '*' (main tier marker)
    Star(&'a str),
    /// Speaker code: /[A-Za-z0-9_\'+\-]+/
    Speaker(&'a str),

    // ── Dependent tier ──────────────────────────────────────
    /// Complete dependent tier prefix, including the required colon and tab.
    TierPrefix(DependentPrefixToken<'a>),
    /// A dependent tier label whose required colon-tab separator did not match.
    IncompleteTierPrefix(&'a str),
    /// Tier separator: ":\t" (colon + tab, after tier label)
    TierSep(PrefixToken<'a>),

    // ── Terminators (grammar.js: terminator supertype) ──────
    /// grammar.js: period = '.'
    Period(&'a str),
    /// grammar.js: question = '?'
    Question(&'a str),
    /// grammar.js: exclamation = '!'
    Exclamation(&'a str),
    /// grammar.js: trailing_off = token(prec(10, '+...'))
    TrailingOff(&'a str),
    /// grammar.js: interruption = token(prec(10, '+/.'))
    Interruption(&'a str),
    /// grammar.js: self_interruption = token(prec(10, '+//.'))
    SelfInterruption(&'a str),
    /// grammar.js: interrupted_question = token(prec(10, '+/?'))
    InterruptedQuestion(&'a str),
    /// grammar.js: broken_question = token(prec(10, '+!?'))
    BrokenQuestion(&'a str),
    /// grammar.js: quoted_new_line = token(prec(10, '+"/.'))
    QuotedNewLine(&'a str),
    /// grammar.js: quoted_period_simple = token(prec(10, '+"..'))
    QuotedPeriodSimple(&'a str),
    /// grammar.js: self_interrupted_question = token(prec(10, '+//?'))
    SelfInterruptedQuestion(&'a str),
    /// grammar.js: trailing_off_question = token(prec(10, '+..?'))
    TrailingOffQuestion(&'a str),
    /// grammar.js: break_for_coding = token(prec(10, '+.'))
    BreakForCoding(&'a str),
    /// grammar.js: ca_no_break = token(prec(10, '≈'))
    CaNoBreak(&'a str),
    /// grammar.js: ca_technical_break = token(prec(10, '≋'))
    CaTechnicalBreak(&'a str),

    // ── Linkers ─────────────────────────────────────────────
    /// grammar.js: linker_lazy_overlap = token(prec(10, '+<'))
    LinkerLazyOverlap(&'a str),
    /// grammar.js: linker_quick_uptake = token(prec(10, '++'))
    LinkerQuickUptake(&'a str),
    /// grammar.js: linker_quick_uptake_overlap = token(prec(10, '+^'))
    LinkerQuickUptakeOverlap(&'a str),
    /// grammar.js: linker_quotation_follows = token(prec(10, '+"'))
    LinkerQuotationFollows(&'a str),
    /// grammar.js: linker_self_completion = token(prec(10, '+,'))
    LinkerSelfCompletion(&'a str),
    /// grammar.js: ca_no_break_linker = token(prec(10, '+≈'))
    CaNoBreakLinker(&'a str),
    /// grammar.js: ca_technical_break_linker = token(prec(10, '+≋'))
    CaTechnicalBreakLinker(&'a str),

    // ── Annotations (atomic brackets) ───────────────────────
    /// grammar.js: retrace_complete = token('[//]')
    RetraceComplete(&'a str),
    /// grammar.js: retrace_partial = token('[/]')
    RetracePartial(&'a str),
    /// grammar.js: retrace_multiple = token('[///]')
    RetraceMultiple(&'a str),
    /// grammar.js: retrace_reformulation = token('[/-]')
    RetraceReformulation(&'a str),
    /// grammar.js: scoped_stressing = token('[!]')
    ScopedStressing(&'a str),
    /// grammar.js: scoped_contrastive_stressing = `token('[!!]')`
    ScopedContrastiveStressing(&'a str),
    /// grammar.js: scoped_uncertain = token('[?]')
    ScopedUncertain(&'a str),
    /// grammar.js: exclude_marker = `token('[e]')`
    ExcludeMarker(&'a str),
    /// grammar.js: code_switch_annotation, bare form `[@s]`.
    ///
    /// Two tokens rather than one with an optional payload, mirroring
    /// `CodeSwitchSpan`'s two variants: the bare form is not a MISSING code,
    /// it is its own resolution rule, so an `Option` here would invite the
    /// same confusion the model type exists to prevent.
    CodeSwitchShortcut(&'a str),
    /// grammar.js: code_switch_annotation, explicit form `[@s:code]`.
    /// Rich token: the tag marks the language code alone.
    CodeSwitchExplicit(&'a str),
    /// grammar.js: freecode = token(/\[\^ [^\]\r\n]+\]/)
    /// Rich token: [^ content] with tag marking content boundaries.
    Freecode(&'a str),
    /// grammar.js: ca_continuation_marker = token('[^c]')
    CaContinuationMarker(&'a str),
    /// grammar.js: error_marker_annotation = token(prec(8, /\[\*[^\]]*\]/))
    ErrorMarkerAnnotation(&'a str),

    // ── Annotations with content ────────────────────────────
    /// grammar.js: indexed_overlap_precedes = token(prec(8, /\[< ?[1-9]? ?\]/))
    OverlapPrecedes(&'a str),
    /// grammar.js: indexed_overlap_follows = token(prec(8, /\[> ?[1-9]? ?\]/))
    OverlapFollows(&'a str),
    /// [= text], explanation
    ExplanationAnnotation(&'a str),
    /// [=! text], paralinguistic
    ParaAnnotation(&'a str),
    /// [=? text], alternative transcription
    AltAnnotation(&'a str),
    /// [% text], percent annotation
    PercentAnnotation(&'a str),
    /// [+ code], postcode
    Postcode(PostcodeToken<'a>),
    /// [- lang], language code
    Langcode(&'a str),
    /// [: replacement words], replacement
    Replacement(&'a str),

    // ── Pauses ──────────────────────────────────────────────
    /// grammar.js: token(prec(10, '(...)'))
    PauseLong(&'a str),
    /// grammar.js: token(prec(10, '(..)'))
    PauseMedium(&'a str),
    /// grammar.js: token(prec(10, '(.)'))
    PauseShort(&'a str),
    /// grammar.js: token(prec(10, /\(\d+(?::\d+)?\.\d*\)/))
    PauseTimed(&'a str),

    // ── Word (rich token) ─────────────────────────────────
    /// A complete word matched by the lexer as a single token.
    /// Tags mark coarse field boundaries; the parser handles body internals.
    ///
    /// `raw_text` is the full word text from source (prefix + body + suffixes).
    /// `body` is the word body slice (parser splits into segments, compounds, CA, etc.).
    /// Suffix fields carry tag-extracted content (no delimiters).
    Word {
        /// Full word text (everything matched by the word rule).
        raw_text: &'a str,
        /// Category prefix if present: `&-`, `&~`, `&+`, or `0`.
        prefix: Option<&'a str>,
        /// Word body: text segments, shortenings, compounds, CA markers, etc.
        /// Parser handles fine-grained body parsing.
        body: &'a str,
        /// Form marker content: `f`, `z:grm`, etc. (without `@` prefix).
        form_marker: Option<&'a str>,
        /// Language suffix codes: `eng`, `eng+zho`, etc. (without `@s:` prefix).
        /// `None` means absent; bare `@s` shortcut carries `Some("")`.
        lang_suffix: Option<&'a str>,
        /// POS tag content: `n`, `adj`, etc. (without `$` prefix).
        pos_tag: Option<&'a str>,
    },

    // ── Word structure (sub-tokens for body parsing) ─────
    /// A word text segment (from WORD_SEGMENT regex).
    WordSegment(&'a str),
    /// grammar.js: shortening = seq('(', word_segment, ')')
    Shortening(&'a str),
    /// grammar.js: lengthening = token(prec(5, /:{1,}/))
    Lengthening(&'a str),
    // ── Stress markers (typed) ──
    /// ˈ primary stress (U+02C8)
    StressPrimary(&'a str),
    /// ˌ secondary stress (U+02CC)
    StressSecondary(&'a str),
    // ── Overlap points (typed, one variant per OverlapPointKind) ──
    /// ⌈ with optional digit
    OverlapTopBegin(&'a str),
    /// ⌉ with optional digit
    OverlapTopEnd(&'a str),
    /// ⌊ with optional digit
    OverlapBottomBegin(&'a str),
    /// ⌋ with optional digit
    OverlapBottomEnd(&'a str),
    /// grammar.js: syllable_pause = '^'
    SyllablePause(&'a str),
    /// grammar.js: tilde = '~'
    Tilde(&'a str),
    /// Compound marker: '+' between word segments.
    CompoundMarker(&'a str),
    /// grammar.js: underline_begin = token(prec(5, '\u0002\u0001'))
    UnderlineBegin(&'a str),
    /// grammar.js: underline_end = token(prec(5, '\u0002\u0002'))
    UnderlineEnd(&'a str),

    // ── Word prefixes ───────────────────────────────────────
    /// grammar.js: token('&-'), filler
    PrefixFiller(&'a str),
    /// grammar.js: token('&~'), nonword
    PrefixNonword(&'a str),
    /// grammar.js: token('&+'), fragment
    PrefixFragment(&'a str),
    /// grammar.js: event = seq(event_marker, event_segment+)
    /// Complete event token: the tag-extracted event description text
    /// (e.g., "laughs" from `&=laughs`, "clears:throat" from `&=clears:throat`).
    Event(&'a str),
    /// grammar.js: zero = token(prec(3, '0'))
    Zero(&'a str),

    /// Rich other_spoken_event: &*SPK:word
    /// Fields extracted by tags: speaker (t1..t2), text (t3..end).
    OtherSpokenEvent {
        speaker: &'a str,
        text: &'a str,
    },

    // ── Word suffixes ───────────────────────────────────────
    /// The `@` marker on a word, WITHOUT the `@`: `b`, `fp`, `z:grm`.
    ///
    /// The set of codes is generated into `src/generated_form_markers.re`
    /// from `spec/form_markers/form_marker_registry.json`. A list here used
    /// to enumerate it and had already drifted: it omitted `@z` and quoted a
    /// `grammar.js` regex that no longer exists.
    FormMarker(&'a str),
    /// grammar.js: word_lang_suffix = `token.immediate(/@s(?::[a-z]{2,3}(?:[+&][a-z]{2,3})*)? /)`
    /// `@s` (bare shortcut) carries `None`; `@s:eng+zho` carries `Some("eng+zho")`.
    WordLangSuffix(Option<&'a str>),
    /// grammar.js: pos_tag = seq(token.immediate('$'), /[a-zA-Z:]+/)
    PosTag(&'a str),

    // ── Separators ──────────────────────────────────────────
    /// grammar.js: comma = ','
    Comma(&'a str),
    /// grammar.js: semicolon = ';'
    Semicolon(&'a str),
    /// grammar.js: colon = ':' (standalone separator, not word-internal lengthening)
    Colon(&'a str),
    /// grammar.js: tag_marker = '\u201E' (double low-9 quotation mark)
    TagMarker(&'a str),
    /// grammar.js: vocative_marker = '\u2021' (double dagger)
    VocativeMarker(&'a str),
    /// grammar.js: unmarked_ending = '\u221E' (infinity)
    UnmarkedEnding(&'a str),
    /// grammar.js: uptake_symbol = '\u2261' (identical to)
    UptakeSymbol(&'a str),

    // ── Intonation contours ─────────────────────────────────
    /// grammar.js: rising_to_high = '\u21D7'
    RisingToHigh(&'a str),
    /// grammar.js: rising_to_mid = '\u2197'
    RisingToMid(&'a str),
    /// grammar.js: level_pitch = '\u2192'
    LevelPitch(&'a str),
    /// grammar.js: falling_to_mid = '\u2198'
    FallingToMid(&'a str),
    /// grammar.js: falling_to_low = '\u21D8'
    FallingToLow(&'a str),

    // ── Groups ──────────────────────────────────────────────
    /// grammar.js: less_than = '<'
    LessThan(&'a str),
    /// grammar.js: greater_than = '>'
    GreaterThan(&'a str),
    /// grammar.js: left_double_quote = '\u201C'
    LeftDoubleQuote(&'a str),
    /// grammar.js: right_double_quote = '\u201D'
    RightDoubleQuote(&'a str),
    /// grammar.js: illegal_curly_quote = choice(U+2018, U+2019).
    ///
    /// A curly single quotation mark used as a word character. This is a
    /// *recognized* token (not a lexer error fallback), mirroring the
    /// tree-sitter `illegal_curly_quote` node. The file-level parser emits
    /// E256 for it (CLAN CHECK 138/139) and drops it from the content, so
    /// the surrounding words survive. It never opens a group, and it is
    /// not a lexer-classification failure, so `is_err()` stays false.
    IllegalCurlyQuote(&'a str),
    /// grammar.js: pho_begin_group = '‹' (U+2039)
    PhoGroupBegin(&'a str),
    /// grammar.js: pho_end_group = '›' (U+203A)
    PhoGroupEnd(&'a str),
    /// grammar.js: sin_begin_group = '〔' (U+3014)
    SinGroupBegin(&'a str),
    /// grammar.js: sin_end_group = '〕' (U+3015)
    SinGroupEnd(&'a str),

    // ── Misc structural ─────────────────────────────────────
    /// grammar.js: long_feature_begin = seq('&', '{l=', label)
    LongFeatureBegin(&'a str),
    /// grammar.js: long_feature_end = seq('&', '}l=', label)
    LongFeatureEnd(&'a str),
    /// grammar.js: nonvocal_begin = seq('&', '{n=', label)
    NonvocalBegin(&'a str),
    /// grammar.js: nonvocal_end = seq('&', '}n=', label)
    NonvocalEnd(&'a str),
    /// grammar.js: nonvocal_simple = seq('&', '{n=', label, '}')
    NonvocalSimple(&'a str),

    /// grammar.js: ampersand = '&'
    Ampersand(&'a str),
    /// An annotation whose marker this parser does not recognise: the text
    /// BETWEEN the brackets of a `[...]` that matched no specific rule.
    ///
    /// Exists so an unknown marker is a validity question rather than a parse
    /// failure. `*CHI:\thello [@ xyz] world .` used to lex as a bare
    /// `LeftBracket` and take the whole utterance down with E321 "unparsable",
    /// while tree-sitter parsed it and reported E207 "unknown annotation",
    /// which is what `spec/errors/E207.md` says should happen.
    UnknownAnnotation(&'a str),
    /// grammar.js: left_bracket = '['
    LeftBracket(&'a str),
    /// grammar.js: right_bracket = ']'
    RightBracket(&'a str),
    /// grammar.js: media_url = token(/\u0015\d+_\d+-?\u0015/)
    /// Media bullet with tag-extracted timestamps.
    /// Pattern: `\u{0015}start_end-?\u{0015}`, tags mark start (t1..t2) and end (t3..t4).
    /// `raw_text` carries the full original slice including NAK delimiters,
    /// so no downstream reconstruction is needed.
    MediaBullet {
        raw_text: &'a str,
        start_time: &'a str,
        end_time: &'a str,
    },

    // ── CA elements (typed, one variant per CAElementType) ──
    CaBlockedSegments(&'a str), // ≠
    CaConstriction(&'a str),    // ∾
    CaHardening(&'a str),       // ⁑
    CaHurriedStart(&'a str),    // ⤇
    CaInhalation(&'a str),      // ∙
    CaLaughInWord(&'a str),     // Ἡ
    CaPitchDown(&'a str),       // ↓
    CaPitchReset(&'a str),      // ↻
    CaPitchUp(&'a str),         // ↑
    CaSuddenStop(&'a str),      // ⤆

    // ── CA delimiters (typed, one variant per CADelimiterType) ──
    CaUnsure(&'a str),            // ⁇
    CaPrecise(&'a str),           // §
    CaCreaky(&'a str),            // ⁎
    CaSofter(&'a str),            // °
    CaSegmentRepetition(&'a str), // ↫
    CaFaster(&'a str),            // ∆
    CaSlower(&'a str),            // ∇
    CaWhisper(&'a str),           // ∬
    CaSinging(&'a str),           // ∮
    CaLowPitch(&'a str),          // ▁
    CaHighPitch(&'a str),         // ▔
    CaLouder(&'a str),            // ◉
    CaSmileVoice(&'a str),        // ☺
    CaBreathyVoice(&'a str),      // ♋
    CaYawn(&'a str),              // Ϋ

    // ── %mor tier ───────────────────────────────────────────
    /// Rich MorWord: POS and lemma+features extracted by tags.
    /// Example: pos="verb", lemma_features="want-Fin-Ind-Pres"
    MorWord {
        pos: &'a str,
        lemma_features: &'a str,
    },
    /// Tilde in %mor: '~' (clitic separator between mor words)
    MorTilde(&'a str),

    // ── %gra tier ───────────────────────────────────────────
    /// Rich GraRelation: all 3 fields extracted by tags.
    /// Example: index="1", head="2", relation="SUBJ"
    GraRelation {
        index: &'a str,
        head: &'a str,
        relation: &'a str,
    },

    // ── %pho/%mod tier ────────────────────────────────────────
    /// PHO word: IPA phonological transcription segment.
    PhoWord(&'a str),
    /// Plus joining compound phonological words.
    PhoPlus(&'a str),

    // ── %sin tier ───────────────────────────────────────────
    /// SIN word: sign/gesture notation segment.
    SinWord(&'a str),

    // ── @Languages content ────────────────────────────────────
    /// Language code: /[a-z]{2,4}/ (e.g., "eng", "fra", "zho")
    LanguageCode(&'a str),

    // ── @Participants content ───────────────────────────────
    /// Participant word (speaker code, name, or role word)
    ParticipantWord(&'a str),

    // ── @Media content ──────────────────────────────────────
    /// Quoted media filename: "filename.mp4"
    MediaFilename(&'a str),
    /// Media word: unquoted filename, type (audio/video), or status
    MediaWord(&'a str),

    // ── Generic tier content ────────────────────────────────
    /// grammar.js: text_segment = /[^\u0015\r\n]+/
    TextSegment(&'a str),
    /// grammar.js: inline_pic = /\u0015%pic:"filename"\u0015/
    InlinePic(&'a str),

    // ── Errors (context-specific, one per condition) ───────
    //
    // Each error token tells the parser WHERE the error occurred.
    // The lexer always makes progress (consumes at least 1 char)
    // and stays in the same condition, so lexing never stops.
    /// Unrecognized character (global fallback, should rarely fire).
    ErrorUnrecognized(&'a str),
    /// Invalid line at top level (INITIAL condition).
    ErrorLine(&'a str),
    /// Junk after header name (expected :\t or newline).
    ErrorHeaderAfterName(&'a str),
    /// Invalid speaker code (SPEAKER condition).
    ErrorSpeaker(&'a str),
    /// Junk after tier label (TIER_AFTER_LABEL, expected :\t).
    ErrorTierAfterLabel(&'a str),
    /// Junk after speaker (TIER_SEP, expected :\t).
    ErrorTierSep(&'a str),
    /// Unclosed parenthesis in main content.
    ErrorUnclosedParen(&'a str),
    /// Unexpected char in main tier body (MAIN_CONTENT).
    ErrorInMainContent(&'a str),
    /// Unexpected char in %mor body (MOR_CONTENT).
    ErrorInMorContent(&'a str),
    /// Unexpected char in %gra body (GRA_CONTENT).
    ErrorInGraContent(&'a str),
    /// Unexpected char in %pho body (PHO_CONTENT).
    ErrorInPhoContent(&'a str),
    /// Unexpected char in %sin body (SIN_CONTENT).
    ErrorInSinContent(&'a str),
    /// Unexpected char in generic tier body (TIER_CONTENT).
    ErrorInTierContent(&'a str),
    /// Unexpected char in header value (HEADER_CONTENT).
    ErrorInHeaderContent(&'a str),
    /// Malformed @ID content (ID_CONTENT).
    ErrorInIdContent(&'a str),
    /// Malformed @Types content (TYPES_CONTENT).
    ErrorInTypesContent(&'a str),
    /// Unexpected char in @Languages content.
    ErrorInLanguagesContent(&'a str),
    /// Unexpected char in @Participants content.
    ErrorInParticipantsContent(&'a str),
    /// Unexpected char in @Media content.
    ErrorInMediaContent(&'a str),
}
