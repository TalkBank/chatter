/**
 * @file Tree-sitter grammar for the TalkBank CHAT transcription format.
 *
 * CHAT (Codes for the Human Analysis of Transcripts) is a standardized format
 * for transcribing and annotating conversational interactions. This grammar
 * produces a concrete syntax tree (CST) covering the full CHAT specification:
 * document structure, headers, main tiers (utterances), dependent tiers
 * (morphology, phonology, grammatical relations), annotations, overlap markers,
 * and Conversation Analysis (CA) features.
 *
 * Design principles:
 *   - "Parse, don't validate", accept lenient input, let downstream validate
 *   - Opaque word tokens, word-internal structure parsed by a separate pass
 *   - Atomic annotation tokens, bracket annotations like [= text] are single
 *     tokens; the Rust parser extracts their contents
 *   - Explicit whitespace, extras is empty; all whitespace is grammar-visible
 *
 * CHAT manual: https://talkbank.org/0info/manuals/CHAT.html
 * Grammar reference links use anchors in the format CHAT.html#Main_Line
 *
 * @author Franklin Chen <franklinchen@franklinchen.com>
 * @license MIT
 * @see {@link https://talkbank.org/0info/manuals/CHAT.html} CHAT Manual
 * @see {@link https://github.com/TalkBank/chatter} Upstream specs and parsers
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

import {
  CA_DELIMITER_SYMBOLS,
  CA_ELEMENT_SYMBOLS,
  CA_ALL_SYMBOLS,
  EVENT_SEGMENT_FORBIDDEN_BASE,
  EVENT_SEGMENT_FORBIDDEN_COMMON,
  WORD_SEGMENT_FORBIDDEN_START_BASE,
  WORD_SEGMENT_FORBIDDEN_REST_BASE,
  WORD_SEGMENT_FORBIDDEN_COMMON,
} from './src/generated_symbol_sets.js';

// Event descriptions should stop before CA symbols so constructs like &=smack°uh
// are tokenized as event + CA-marked speech, not one giant event token.
// Keep colon allowed (e.g., &=clears:throat).
const EVENT_SEGMENT_FORBIDDEN = EVENT_SEGMENT_FORBIDDEN_BASE
  + CA_ELEMENT_SYMBOLS
  + CA_DELIMITER_SYMBOLS
  + EVENT_SEGMENT_FORBIDDEN_COMMON;

// Shared regex for @ID pipe-delimited fields with leading/trailing whitespace trimming.
// Matches non-empty content that doesn't start or end with whitespace.
// Used by: id_corpus, id_group, id_education, id_custom_field, and id_age catch-all.
const TRIMMED_PIPE_FIELD = /[^ \t\|\r\n]([^\|\r\n]*[^ \t\|\r\n])?/;
const EVENT_SEGMENT_RE = new RegExp(`[^${EVENT_SEGMENT_FORBIDDEN}]+`);

// Word segment character exclusions, built from symbol registry.
// word_segment must contain ONLY spoken text. All structural markers
// are separate children in word_body.
//
// INVARIANT: word_segment is guaranteed clean. No post-processing needed
// to extract overlap markers, CA elements, underline markers, etc.
// First char also excludes 0 (omission prefix → zero token, not word_segment)
const WORD_SEGMENT_FORBIDDEN_FIRST = WORD_SEGMENT_FORBIDDEN_START_BASE
  + CA_ALL_SYMBOLS
  + WORD_SEGMENT_FORBIDDEN_COMMON
  + '0';
const WORD_SEGMENT_FORBIDDEN_REST = WORD_SEGMENT_FORBIDDEN_REST_BASE
  + CA_ALL_SYMBOLS
  + WORD_SEGMENT_FORBIDDEN_COMMON;
const WORD_SEGMENT_FIRST_RE = new RegExp(`[^${WORD_SEGMENT_FORBIDDEN_FIRST}]`);
const WORD_SEGMENT_REST_RE = new RegExp(`[^${WORD_SEGMENT_FORBIDDEN_REST}]*`);

export default grammar({
  name: 'talkbank',

  // Handle whitespace explicitly.
  // Empty extras is the load-bearing design decision of this grammar
  // (2026-03-08, unchanged since): ALL whitespace is grammar-visible.
  // Rationale: the worst bug a CHAT parser can have (and one the legacy
  // Java implementation had) is ignoring whitespace, so that accidental
  // juxtaposition of content items is ACCEPTED as if properly
  // separated: Java Chatter tokenized "hello(.)" correctly as a word
  // and a pause, and that acceptance was precisely the problem: the
  // missing required whitespace was invisible, unflaggable, and the
  // malformed source never got cleaned. With extras empty, every space
  // is a grammar-visible fact, so missing separation between content
  // items is DETECTABLE and rejectable; the rejection itself is
  // enforced by Rust validation (per the contents-rule note below),
  // not by the grammar. Never add anything to extras.
  extras: $ => [],

  conflicts: $ => [
    [$.contents],
    [$.contents, $.word_body],  // overlap_point/ca_element/underline can be standalone content or word-internal
    [$.base_content_item, $.word_body],  // underline_begin can be standalone or word-internal
    [$.word_with_optional_annotations],
    [$.nonword_with_optional_annotations],  // Annotations create ambiguity with following content
    [$.base_annotations],
    [$.final_codes],
  ],

  // The document entry mirrors the CHAT File Format section (#File_Format): an `@UTF8` marker,
  // optional pre-@Begin headers, a `@Begin`, repeated content lines, and an `@End`. This shape
  // ensures downstream tooling can rely on the standard prelude/epilogue order while still
  // allowing leniency in the intermediate headers (e.g., real files sometimes shuffle @PID and @Window).
  // Supertypes: abstract node categories for cleaner tree-sitter queries
  // These are choice rules that define abstract categories (e.g., "terminator" includes period, question, etc.)
  // Queries can match `(terminator)` to get any terminator type
  // Reference: https://tree-sitter.github.io/tree-sitter/creating-parsers#supertype-nodes
  supertypes: $ => [
    $.terminator,           // All utterance-ending punctuation
    $.linker,               // Discourse linkers (++, +<, etc.)
    $.base_annotation,      // Bracket annotations ([!], [= ...], etc.)
    $.dependent_tier,       // All dependent tier types (%mor, %gra, etc.)
    $.header,               // All headers (@Languages, @ID, etc.)
    $.pre_begin_header,     // Headers before @Begin (@PID, @Window, etc.)
  ],

  rules: {
    // ============================================================================
    // DOCUMENT STRUCTURE
    // ============================================================================
    // A CHAT file has this structure:
    //   @UTF8                     - Required encoding marker
    //   [pre-begin headers]       - Optional: @PID, @Color words, @Window, @Font
    //   @Begin                    - Required start of content
    //   [headers and utterances]  - Main content: @Languages, @Participants, @ID, *CHI: etc.
    //   @End                      - Required end of content
    //
    // Reference: https://talkbank.org/0info/manuals/CHAT.html
    // ============================================================================

    // Multi-root grammar: parse full CHAT documents OR individual fragments.
    //
    // CHAT is line-oriented. External tools (Phon, researchers, Python bindings)
    // often want to parse a single line, a main tier, a dependent tier, a header,
    // without constructing a full @UTF8...@Begin...@End document.
    //
    // Precedence resolves ambiguity: a bare "*CHI:\thello ." is parsed as
    // main_tier (not utterance with zero dependent tiers). A multi-line block
    // with dependent tiers is parsed as utterance. A full file with @UTF8/@Begin/@End
    // is parsed as full_document.
    source_file: $ => choice(
      prec(3, $.full_document),    // complete @UTF8...@Begin...@End file
      prec(2, $.utterance),        // main tier + dependent tiers (no document headers)
      prec(1, $.main_tier),        // single *SPEAKER:\tcontent terminator
      prec(1, $.dependent_tier),   // single %tier:\tcontent
      prec(1, $.header),           // single @Header:\tcontent
      prec(1, $.pre_begin_header), // @PID, @Font, @Window, @Color words
      prec(0, $.standalone_word),  // single word token (for fragment parsing)
    ),

    // Full CHAT document structure:
    // 1. @UTF8 (required, must be first non-whitespace content)
    // 2. Optional pre-@Begin headers (@PID, @Color words, @Window, @Font)
    // 3. @Begin (effectively required, but optional for lenient parsing)
    // 4. Main content headers and utterances
    // 5. @End (effectively required, but optional for lenient parsing)
    full_document: $ => seq(
      $.utf8_header,

      // Optional headers that can appear before @Begin
      // Pre-begin header order: pid? colorWords? window? font? BEG
      repeat($.pre_begin_header),

      $.begin_header,

      // Main content: headers and utterances after @Begin
      repeat($.line),

      $.end_header
    ),

    // DESIGN DECISION: Pre-@Begin header order is lenient (choice, not seq)
    // The ANTLR grammar enforces strict order: pid? colorWords? window? font? BEG
    // We allow any order because:
    //   1. "Parse, don't validate" - downstream can enforce ordering
    //   2. Real-world files may have non-standard ordering
    //   3. Strict ordering would reject otherwise-parseable files
    // Downstream consumers should validate ordering if required.
    // Reference: ChatJFlexAntlr4Parser.g4 chatPrelude rule
    // The actual headers permitted in this slot are drawn directly from the CLI manual
    // plan for CHAT (#Pre_Begin_Headers), so this rule only enumerates pid, color words,
    // window, and font markers as per the specification.
    pre_begin_header: $ => choice(
      $.pid_header,
      $.color_words_header,
      $.window_header,
      $.font_header
    ),

    // ============================================================================
    // BASIC TOKENS
    // ============================================================================
    // Low-level tokens used throughout the grammar. CHAT uses explicit whitespace
    // (no implicit extras), so continuation lines (newline + tab), spaces, and
    // tabs are all named tokens.
    // ============================================================================

    continuation: $ => /[\r\n]+\t/,  // Multi-line continuation (newline followed by tab)
    // A single line break (LF, CRLF, or lone CR). Deliberately NOT `[\r\n]+`:
    // fusing consecutive newlines into one token erased blank lines, so they
    // could never be detected. With a single-break token, a blank line is a
    // standalone `newline` that the `blank_line` rule represents structurally.
    newline: $ => /\r\n|[\r\n]/,

    star: $ => '*',       // Main tier prefix marker
    hyphen: $ => '-',     // Used in MOR suffixes and other contexts

    space: $ => ' ',
    tab: $ => '\t',

    // Matches all non-newline characters to end of line.
    // Used as a building block for free-text header values and catch-all content.
    rest_of_line: $ => /[^\r\n]+/,

    // Text segment that doesn't contain bullet markers or newlines
    // Used in text_with_bullets for content between bullets
    text_segment: $ => /[^\u0015\r\n]+/,

    // Media bullet: \u0015START_END\u0015 or \u0015START_END-\u0015 (skip)
    //
    // ONE structured rule used everywhere bullets appear:
    // - `utterance_end` (terminal timing after terminator)
    // - `base_content_item` (inline timing between words)
    // - `text_with_bullets` (dependent tier timing)
    // - `wor_tier_body` (word-level timing)
    //
    // The skip dash (-) before closing NAK is extremely rare (10 occurrences
    // in 99,742 files) but syntactically valid.
    bullet: $ => seq(
      $.bullet_start,
      field('start_time', $.bullet_timestamp),
      '_',
      field('end_time', $.bullet_timestamp),
      $.bullet_end,
    ),
    bullet_start: $ => '\u0015',
    bullet_end: $ => '\u0015',
    bullet_timestamp: $ => /\d+/,

    // Picture URL: \u0015%pic:"filename"\u0015
    // Used in @Comment and %com tiers to reference picture files
    // Original grammar rule:
    //   urlPic: URL_PIC^ BULLET_BEGIN_FILENAME! mediaFilename BULLET_END_FILENAME! BULLET_URL
    // Where URL_PIC contains "%pic:", BULLET_BEGIN/END_FILENAME are quotes, BULLET_URL is \u0015
    // Coarsened to single token, Rust parser extracts filename via text parsing
    inline_pic: $ => token(/\u0015%pic:"[a-zA-Z0-9][a-zA-Z0-9\/\-_'.]*"\u0015/), 

    // Text content with optional inline bullets AND picture references interspersed.
    // LINT NOTE (2026-03-24): text_with_bullets_and_pics, text_with_bullets, free_text,
    // and header_gap are all flagged as degenerate repeats because repeat1(choice(...))
    // can produce minimal parses with subsets of their alternatives. This is intentional,
    // these are text catch-all rules that must accept any combination of their parts.
    //
    // Only @Comment and %com tiers support `%pic:` references; other tiers use
    // text_with_bullets (no pics). This distinction is enforced by rule choice, not validation.
    // Format: text [bullet|pic text]* or bullet|pic [text bullet|pic]*
    // Includes continuation lines (\n\t) to handle multi-line tiers
    text_with_bullets_and_pics: $ => repeat1(choice(
      $.text_segment,
      $.bullet,
      $.inline_pic,
      $.continuation
    )),

    // Text content with optional inline bullets interspersed (no pics)
    // Format: text [bullet text]* or bullet [text bullet]*
    // Used in: %act, %add, %cod, %eng, %err, %exp, %gpx, %ort, %sit, %tim, %x...
    // Includes continuation lines (\n\t) to handle multi-line tiers
    text_with_bullets: $ => repeat1(choice(
      $.text_segment,
      $.bullet,
      $.continuation
    )),

    // ============================================================================
    // TERMINATORS
    // ============================================================================
    // Utterance-ending punctuation marks. Each terminator has specific meaning.
    // Standard: . ? !
    // Extended: +... (trailing off), +/. (interrupted), +//. (self-interrupted), etc.
    // CA: utterance terminators are the standard CHAT terminators only.
    // CA arrows and TCU markers stay in content as separators, while +≈/+≋
    // remain linkers.
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Utterance_Terminator
    // ============================================================================
    terminator: $ => choice(
      $.period,
      $.question,
      $.exclamation,
      $.trailing_off,           // SUTTO = "+..."
      $.interruption,           // SUTI = "+/."
      $.self_interruption,      // SUTSI = "+//."
      $.interrupted_question,   // SUTIQ = "+/?"
      $.broken_question,        // SUTQE = "+!?"
      $.quoted_new_line,        // SUTNL = "+\"/."
      $.quoted_period_simple,   // SUTQP = "+\"."
      $.self_interrupted_question, // SUTSIQ = "+//?"
      $.trailing_off_question,  // SUTTOQ = "+..?"
      $.break_for_coding,       // SUBFC = "+."
    ),

    // Basic terminators
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Period_Terminator
    period: $ => '.',
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#QuestionMark_Terminator
    question: $ => '?',
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#ExclamationMark_Terminator
    exclamation: $ => '!',

    // Special utterance terminators - use token() to avoid conflict with + in words
    // '+' is also used in word compounds (word+word), so we need lexer-level matching
    // All terminators starting with '+' use prec(10) to beat standalone_word (prec 5).
    // Without this, '+' would be consumed as part of a standalone_word.
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#TrailingOff_Terminator
    trailing_off: $ => token(prec(10, '+...')),              // +... Speaker trails off
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Interruption_Terminator
    interruption: $ => token(prec(10, '+/.')),               // +/. Interrupted by another speaker
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#SelfInterruption_Terminator
    self_interruption: $ => token(prec(10, '+//.')),         // +//. Self-interruption
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#QuestionInterruption_Terminator
    interrupted_question: $ => token(prec(10, '+/?')),       // +/? Question interrupted
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#QuestionExclamation_Terminator
    broken_question: $ => token(prec(10, '+!?')),            // +!? Question broken off
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Terminator
    quoted_new_line: $ => token(prec(10, '+\"/.')),          // +"/. Quote continues next line
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#QuotationPrecedes_Terminator
    quoted_period_simple: $ => token(prec(10, '+\".')),      // +". Quote ends with period
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#SelfInterruptedQuestion_Terminator
    self_interrupted_question: $ => token(prec(10, '+//?')), // +//? Self-interrupted question
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#TrailingOffQuestion_Terminator
    trailing_off_question: $ => token(prec(10, '+..?')),     // +..? Trailing off question
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#TranscriptionBreak_Terminator
    break_for_coding: $ => token(prec(10, '+.')),            // +. Break for coding

    // CA TCU markers stay on the separator/linker path.
    // Split into separate rules to preserve +/no-+ distinction in roundtrip.
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#TCU_NoBreak
    ca_no_break: $ => token(prec(10, '≈')),          // ≈ U+2248 - No-break TCU separator
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#NoBreakTCUCompletion_Linker
    ca_no_break_linker: $ => token(prec(10, '+≈')),  // +≈ - No break TCU (as linker)
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#TCU_Technical_Break
    ca_technical_break: $ => token(prec(10, '\u224B')), // ≋ U+224B - Technical break TCU separator
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#TechnicalBreakTCUCompletion_Linker
    ca_technical_break_linker: $ => token(prec(10, '+≋')),  // +≋ - Technical break TCU (as linker)

    // === STRUCTURAL PUNCTUATION ===
    // Named rules for type-safe node dispatch
    pipe: $ => '|',
    ampersand: $ => '&',
    comma: $ => ',',
    semicolon: $ => ';',
    less_than: $ => '<',
    greater_than: $ => '>',
    left_paren: $ => '(' ,
    right_paren: $ => ')',
    right_brace: $ => '}',
    left_bracket: $ => '[',
    right_bracket: $ => ']',

    // === CODE MARKERS ===
    // Named rules for semantic markers
    plus: $ => '+',
    tilde: $ => '~',
    syllable_pause: $ => '^',
    equals: $ => '=',
    dollar: $ => '$',
    hash: $ => '#',
    double_quote: $ => '"',
    slash: $ => '/',
    // Represents omitted words or gestures (CHAT `0word` notation).
    // The `0` prefix is split from the word body so tree-sitter can identify omissions structurally.
    // Example: "0action" → zero + standalone_word("action")
    // Higher precedence than natural_number (prec 2) to ensure '0' is lexed as zero, not natural_number.
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Omitted_Words
    //
    // LINT NOTE (2026-03-24): `zero` at prec(3) shadows `sin_word` regex and `speaker` regex
    // at the DFA level. Both shadows are harmless:
    //   - sin_word explicitly lists $.zero as an alternative, so the shadow is intentional
    //   - speaker only appears after `*` in main_tier context; the parser rule structure
    //     disambiguates (zero never competes with speaker at the same parse position)
    zero: $ => token(prec(3, '0')),

    // ============================================================================
    // SPECIAL CHARACTER MARKERS (Control Characters & Structural Markers)
    // ============================================================================
    // These are non-printing or special characters used for structural purposes.
    // ============================================================================
    event_marker: $ => token('&='),    // &= - Event prefix

    // Free-text content for unstructured header values (e.g., @Location, @Situation).
    // Supports multi-line values via continuation (newline + tab).
    // Chat.flex: ANYWORD = {ANYNONLF}+ | {ANYNONLF}* \[ ({ANYNONLF}+ | {WS})+ \] {ANYNONLF}* | \u2013
    free_text: $ => repeat1(choice(
      $.rest_of_line,
      $.continuation  // External continuation token
    )),

    line: $ => choice(
      $.header,
      $.utterance,
      $.blank_line,
      $.unsupported_line,
    ),

    // A blank line: a line break with no content. CHAT does not allow blank
    // lines anywhere in the transcript (CLAN CHECK 91). Represented as a
    // structural node (the single-break `newline` token no longer fuses
    // consecutive newlines) so the parser can flag it from the tree rather than
    // by scanning source text; the Rust parser emits BlankLineNotAllowed (E747).
    blank_line: $ => $.newline,

    // Catch-all for lines that don't start with *, @, or %, prevents ERROR
    // cascade when junk lines appear in CHAT files.  The Rust parser will
    // report a validation warning and skip these.
    unsupported_line: $ => seq(
      /[^*@%\t\n\r][^\n\r]*/,
      $.newline,
    ),

    // ============================================================================
    // HEADERS
    // ============================================================================
    // Headers provide metadata about the transcript (@Languages, @Participants, @ID, etc.)
    // and about the recording session (@Location, @Date, @Media, etc.)
    //
    // Special structural headers (not in this choice):
    //   - @UTF8, @Begin, @End: fixed positions in document structure
    //   - @PID, @Color words, @Window, @Font: pre_begin_header (before @Begin)
    //
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#File_Headers
    // ============================================================================
    header: $ => choice(
      $.languages_header,
      $.participants_header,
      $.id_header,
      $.birth_of_header,
      $.birthplace_of_header,
      $.l1_of_header,
      $.media_header,
      $.location_header,
      $.number_header,
      $.recording_quality_header,
      $.room_layout_header,
      $.tape_location_header,
      $.time_duration_header,
      $.time_start_header,
      $.transcriber_header,
      $.transcription_header,
      $.warning_header,
      $.activities_header,
      $.bck_header,
      $.bg_header,
      $.blank_header,
      $.comment_header,
      $.date_header,
      $.eg_header,
      $.g_header,
      $.new_episode_header,
      $.situation_header,
      $.page_header,
      $.options_header,
      $.videos_header,
      $.types_header,
      $.thumbnail_header,
      $.t_header,
      $.unsupported_header,
    ),

    // Catch-all for unknown @-headers.  Matches any single-word @Header that
    // doesn't match a known header prefix.  No spaces in the regex, all known
    // multi-word headers (Birth of, New Episode, etc.) already have explicit
    // token() rules that win by longest-match.
    unsupported_header: $ => seq(
      alias(/@[A-Z][A-Za-z]*/, $.unsupported_header_prefix),
      $.header_sep,
      $.rest_of_line,
      $.newline,
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#UTF8_Header
    // OPTIMIZATION: token('@UTF8') reduces states vs seq(at_sign, 'UTF8')
    utf8_header: $ => seq(token('@UTF8'), $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Begin_Header
    // OPTIMIZATION: token('@Begin') reduces states vs seq(at_sign, 'Begin')
    begin_header: $ => seq(token('@Begin'), $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Languages_Header
    // OPTIMIZATION: token('@Languages') reduces states; whitespace handled by header_sep
    languages_header: $ => seq(
      $.languages_prefix,
      $.header_sep,
      $.languages_contents,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Participants_Header
    // OPTIMIZATION: token('@Participants') reduces states; whitespace handled by header_sep
    participants_header: $ => seq(
      $.participants_prefix,
      $.header_sep,
      $.participants_contents,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#ID_Header
    // OPTIMIZATION: token('@ID') reduces states; whitespace handled by header_sep
    id_header: $ => seq(
      $.id_prefix,
      $.header_sep,
      $.id_contents,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#End_Header
    // OPTIMIZATION: token('@End') reduces states vs seq(at_sign, 'End')
    end_header: $ => seq(token('@End'), $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Birth_Header
    // ANTLR: birthof: BIRTHOF whoChecked COLON TAB DATE_ALL NEWLINE
    // Birth of takes a specific date format, not free_text
    birth_of_header: $ => seq(
      $.birth_of_prefix,
      optional($.header_gap),
      $.speaker,
      $.header_sep,
      $.date_contents,  // Structured date parsing, not free_text
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Birthplace_Header
    birthplace_of_header: $ => seq(
      $.birthplace_of_prefix,
      optional($.header_gap),
      $.speaker,
      $.header_sep,
      $.free_text,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#L1_Header
    // ANTLR: l1of: L1OF whoChecked COLON TAB LANGUAGE_CODE NEWLINE
    // L1 of takes a language code, not free_text
    l1_of_header: $ => seq(
      $.l1_of_prefix,
      optional($.header_gap),
      $.speaker,
      $.header_sep,
      $.language_code,  // Structured language code parsing, not free_text
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Media_Header
    media_header: $ => seq($.media_prefix, $.header_sep, $.media_contents, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Location_Header
    location_header: $ => seq($.location_prefix, $.header_sep, $.free_text, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Number_Header
    number_header: $ => seq($.number_prefix, $.header_sep, $.number_option, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Recording_Quality_Header
    recording_quality_header: $ => seq($.recording_quality_prefix, $.header_sep, $.recording_quality_option, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Room_Layout_Header
    room_layout_header: $ => seq($.room_layout_prefix, $.header_sep, $.free_text, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Tape_Location_Header
    tape_location_header: $ => seq($.tape_location_prefix, $.header_sep, $.free_text, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Time_Duration_Header
    time_duration_header: $ => seq($.time_duration_prefix, $.header_sep, $.time_duration_contents, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Time_Start_Header
    time_start_header: $ => seq($.time_start_prefix, $.header_sep, $.time_duration_contents, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Transcriber_Header
    transcriber_header: $ => seq($.transcriber_prefix, $.header_sep, $.free_text, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Transcription_Header
    transcription_header: $ => seq($.transcription_prefix, $.header_sep, $.transcription_option, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Warning_Header
    warning_header: $ => seq($.warning_prefix, $.header_sep, $.free_text, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Activities_Header
    activities_header: $ => seq($.activities_prefix, $.header_sep, $.free_text, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Bck_Header
    bck_header: $ => seq($.bck_prefix, $.header_sep, $.free_text, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Bg_Header
    bg_header: $ => seq($.bg_prefix, optional(seq($.header_sep, $.free_text)), $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Blank_Header
    blank_header: $ => seq($.blank_prefix, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Comment_Header
    comment_header: $ => seq($.comment_prefix, $.header_sep, $.text_with_bullets_and_pics, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Date_Header
    date_header: $ => seq($.date_prefix, $.header_sep, $.date_contents, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Eg_Header
    eg_header: $ => seq($.eg_prefix, optional(seq($.header_sep, $.free_text)), $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#G_Header
    g_header: $ => seq($.g_prefix, $.header_sep, $.free_text, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#New_Episode_Header
    new_episode_header: $ => seq($.new_episode_prefix, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Situation_Header
    situation_header: $ => seq($.situation_prefix, $.header_sep, $.free_text, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Page_Header
    page_header: $ => seq($.page_prefix, $.header_sep, $.page_number, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Options_Header
    options_header: $ => seq($.options_prefix, $.header_sep, $.options_contents, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Font_Header
    font_header: $ => seq($.font_prefix, $.header_sep, $.free_text, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Window_Header
    window_header: $ => seq($.window_prefix, $.header_sep, $.free_text, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#ColorWords_Header
    color_words_header: $ => seq($.color_words_prefix, $.header_sep, $.free_text, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Videos_Header
    videos_header: $ => seq($.videos_prefix, $.header_sep, $.free_text, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Types_Header
    // Allows optional whitespace before commas: "cross, toyplay, SLI" or "long , interview , TD"
    types_header: $ => seq(
      $.types_prefix,
      $.header_sep,
      $.types_design,
      optional($.whitespaces), $.comma, optional($.whitespaces),
      $.types_activity,
      optional($.whitespaces), $.comma, optional($.whitespaces),
      $.types_group,
      $.newline
    ),

    // Design type: cross, long, observ, or custom value
    types_design: $ => /[a-zA-Z0-9]+/,

    // Activity type: toyplay, narrative, meal, pictures, book, interview, etc.
    types_activity: $ => /[a-zA-Z0-9]+/,

    // Group type: TD, SLI, ASD, biling, L2, HL, CI, PD, etc.
    types_group: $ => /[a-zA-Z0-9]+/,

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#PID_Header
    pid_header: $ => seq($.pid_prefix, $.header_sep, $.free_text, $.newline),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Thumbnail_Header
    thumbnail_header: $ => seq($.thumbnail_prefix, $.header_sep, $.free_text, $.newline),

    // @T: is an inline thumbnail marker (shorthand)
    // Reference: depfile.cut - Local Changeable Headers
    t_header: $ => seq($.t_prefix, $.header_sep, $.free_text, $.newline),

    // ============================================================================
    // MAIN TIERS (Utterances)
    // ============================================================================
    // Main tiers contain the actual speech transcription.
    // Format: *SPEAKER:\tContent terminator
    //
    // Components:
    //   - Speaker code: 3-letter code (CHI, MOT, FAT, etc.)
    //   - Optional linkers: discourse connectors (++, +<, etc.)
    //   - Content: words, events, actions, pauses, groups
    //   - Terminator: . ? ! or extended terminators (+..., +/., etc.)
    //   - Optional postcodes: [+ code] annotations
    //   - Optional media bullet: timestamp reference
    //
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Main_Line
    // ============================================================================

    utterance: $ => seq(
      $.main_tier,
      repeat($.dependent_tier)
    ),

    // Main tier content after the tab, content is required
    // Used by: main_tier (*SPEAKER:\t)
    // NOTE: No leading/trailing optional(whitespaces), contents handles its own whitespace
    tier_body: $ => seq(
      field('linkers', optional($.linkers)),
      field('language_code', optional(
        seq(
          $.langcode,
          $.whitespaces
        )
      )),
      field('content', $.contents),
      field('ending', $.utterance_end)
    ),

    // %wor tier body, word-level timing tier.
    // Flat whitespace-separated sequence: words, inline bullets, and separators.
    // Whitespace is always required between items (it's the only separator).
    // The Rust parser pairs words with following inline_bullet by adjacency.
    wor_tier_body: $ => seq(
      field('language_code', optional(
        seq(
          $.langcode,
          $.whitespaces
        )
      )),
      repeat(seq(
        choice($.wor_word_item, $.bullet, $.comma, $.tag_marker, $.vocative_marker),
        $.whitespaces
      )),
      optional($.terminator),
      $.newline
    ),

    // A single word in a %wor tier.
    wor_word_item: $ => $.standalone_word,

    main_tier: $ => seq(
      $.star,
      field('speaker', $.speaker),
      $.colon,
      $.tab,
      $.tier_body
    ),

    // ANTLR: uend: anyTerminator finalCodes? (NEWLINE | url NEWLINE)
    // finalCodes: finalCode+ where finalCode is SSEXT anyWordsAndMedia RBRACKET (postcode annotations)
    // url: URL milliseconds BULLET_UNDERSCORE milliseconds (BULLET_URL | BULLET_URL_SKIP)
    // Utterance ending: terminator + postcodes + media bullet + newline
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Utterance_Terminator
    // NOTE: Space before terminator is consumed by contents rule's trailing whitespace.
    // Space before media_url and trailing whitespace must be handled here (post-terminator).
    utterance_end: $ => seq(
      // Terminator section - optional in CA transcription mode
      optional($.terminator),
      // Postcode annotations: [+ bch], [+ foo], etc.
      optional($.final_codes),
      // Optional media bullet, appears AFTER terminator in CHAT text.
      // Uses the unified `bullet` rule (structured, not opaque).
      optional(seq(optional($.whitespaces), $.bullet)),
      optional($.whitespaces),  // Allow trailing whitespace before newline
      $.newline
    ),

    // ANTLR: finalCodes: finalCode+ where finalCode is SSEXT anyWordsAndMedia RBRACKET
    // SSEXT = "[+ " so finalCode is postcode annotations like [+ bch], [+ foo]
    // Note: SSEXT includes the space, so no additional space needed before each annotation
    // But there can be spaces between multiple postcode annotations
    final_codes: $ => repeat1(
      seq(
        $.whitespaces,
        $.postcode
      )
    ),

    // media_url DELETED, replaced by the unified `bullet` rule (structured,
    // not opaque). See `bullet` definition near `text_with_bullets`.

    // ============================================================================
    // CONTENT ITEMS
    // ============================================================================
    // Content items are the building blocks of utterances: words, events, pauses,
    // groups, annotations, and special markers. The `contents` rule is a flat
    // repeat of content items separated by whitespace.
    //
    // Design: Content type restrictions are lenient, the grammar uses a unified
    // base_content_item for all contexts (main tier, groups, quotations).
    // Context-specific restrictions (e.g., groups cannot nest) are validated
    // downstream by the Rust parser, not by the grammar.
    //
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Words
    // ============================================================================

    // Base content items that can appear in any context (main tier, groups, quotations)
    // NOTE: overlap_point is handled separately as its own content_item
    base_content_item: $ => choice(
      $.underline_begin,
      $.underline_end,
      $.pause_token,
      $.word_with_optional_annotations,
      $.nonword_with_optional_annotations,
      $.other_spoken_event,
      $.long_feature,
      $.nonvocal,
      $.freecode,
      $.bullet,
    ),

    // Separator is its own first-class content item (not attached to preceding content)
    // Uses named leaf nodes for type-safe node dispatch
    non_colon_separator: $ => choice(
      $.comma,
      $.semicolon,
      $.tag_marker,
      $.vocative_marker,
      $.ca_continuation_marker,
      $.unmarked_ending,
      $.uptake_symbol,
      $.ca_no_break,
      $.ca_technical_break,
      $.rising_to_high,
      $.rising_to_mid,
      $.level_pitch,
      $.falling_to_mid,
      $.falling_to_low,
    ),

    // Separator is a standalone content item.
    // Low precedence so that when a separator character appears adjacent to word text,
    // the word token (prec 5) wins. Standalone separators are whitespace-delimited in CHAT.
    separator: $ => prec(-1, choice(
      $.non_colon_separator,
      $.colon,
    )),

    // ============================================================================
    // INTONATION & SPECIAL MARKERS
    // ============================================================================
    // Markers for tagging, vocatives, and CA intonation contours
    // Reference: https://talkbank.org/0info/manuals/CA.html
    // ============================================================================

    // Tag and vocative markers
    // All Unicode separator/marker tokens use prec(10) to beat standalone_word prec(5)
    // when both match a single Unicode character at the same position.
    tag_marker: $ => token(prec(10, '\u201E')),   // „ U+201E DOUBLE LOW-9 QUOTATION MARK - Tag question marker
    vocative_marker: $ => token(prec(10, '\u2021')), // ‡ U+2021 DOUBLE DAGGER - Vocative/address term marker

    // CA separators and intonation contours
    ca_continuation_marker: $ => token('[^c]'), // [^c] - Continuation (multi-char, won't conflict)
    unmarked_ending: $ => token(prec(10, '\u221E')), // ∞ U+221E INFINITY - Unmarked/flat intonation ending
    uptake_symbol: $ => token(prec(10, '\u2261')), // ≡ U+2261 IDENTICAL TO - Uptake/latching symbol
    rising_to_high: $ => token(prec(10, '\u21D7')), // ⇗ U+21D7 NORTH EAST DOUBLE ARROW - Rising to high pitch
    rising_to_mid: $ => token(prec(10, '\u2197')), // ↗ U+2197 NORTH EAST ARROW - Rising to mid pitch
    level_pitch: $ => token(prec(10, '\u2192')), // → U+2192 RIGHTWARDS ARROW - Level/continuing intonation
    falling_to_mid: $ => token(prec(10, '\u2198')), // ↘ U+2198 SOUTH EAST ARROW - Falling to mid pitch
    falling_to_low: $ => token(prec(10, '\u21D8')), // ⇘ U+21D8 SOUTH EAST DOUBLE ARROW - Falling to low pitch

    // ===== CONTENTS =====
    // Flat sequence of content items with explicit boundaries:
    // - Adjacent content_items require whitespace OR an explicit overlap/separator.
    // - Overlap/separator can appear without whitespace.

    // Content item: the semantic unit within contents (words, groups, quotations, inline tiers).
    // Overlap points and separators are handled separately in the contents rule.
    content_item: $ => choice(
      $.base_content_item,
      $.group_with_annotations,
      $.quotation,
      $.illegal_curly_quote,
      $.main_pho_group,
      $.main_sin_group,
    ),

    // Other speaker's speech event, e.g., &*MOT:word, speech attributed to another speaker.
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Local_Event
    other_spoken_event: $ => seq(
      $.ampersand,
      $.star,
      $.speaker,
      $.colon,
      $.standalone_word
    ),

    // Long-feature markers (`&{l=+...}` and `&{l=-...}`) correspond to the scoped spans described in the
    // CHAT Main Tier long-feature section, so we keep their begin/end tokens atomic while capturing the label.
    long_feature: $ => choice(
      $.long_feature_begin,
      $.long_feature_end
    ),

    long_feature_begin: $ => seq(
      $.ampersand,
      $.long_feature_begin_marker,
      $.long_feature_label  // Allows @, %, alphanumeric
    ),

    long_feature_end: $ => seq(
      $.ampersand,
      $.long_feature_end_marker,
      $.long_feature_label  // Allows @, %, alphanumeric
    ),

    nonvocal: $ => choice(
      $.nonvocal_begin,
      $.nonvocal_end,
      $.nonvocal_simple,
    ),

    nonvocal_begin: $ => seq(
      $.ampersand,
      $.nonvocal_begin_marker,
      $.long_feature_label  // Same as long features
    ),

    nonvocal_end: $ => seq(
      $.ampersand,
      $.nonvocal_end_marker,
      $.long_feature_label  // Same as long features
    ),

    nonvocal_simple: $ => seq(
      $.ampersand,
      $.nonvocal_begin_marker,
      $.long_feature_label,  // Same as long features
      $.right_brace
    ),

    // Named retrace leaf nodes for type-safe dispatch
    retrace_complete: $ => token('[//]'),
    retrace_partial: $ => token('[/]'),
    retrace_multiple: $ => token('[///]'),
    retrace_reformulation: $ => token('[/-]'),

    exclude_marker: $ => token('[e]'),

    // ANTLR: annotatedGroup = LESS contents GREATER scopedAnnotations

    // ===== CONTENTS RULE =====
    // Main contents rule
    // DESIGN: Free-floating punctuation (separators) can appear anywhere in content.
    // These are standalone content items, not "separators" in the traditional sense.
    //
    // DISAMBIGUATION - Colons have dual roles:
    // 1. Word-internal prosody markers: a:b:c (no whitespace around colons)
    // 2. Free-floating punctuation: ": hello" or "word :" (whitespace-delimited)
    //
    // SOLUTION: Only allow standalone colons when FOLLOWED by whitespace or at end.
    // Flat repeat of content items. Ordering constraints (e.g., separators must be
    // whitespace-delimited) are enforced by Rust validation, not the grammar.
    // LINT NOTE (2026-03-24): repeat1(choice(...)) is flagged as degenerate because
    // it can produce minimal parses with only whitespaces+overlap_point (no words).
    // This is intentional, CHAT allows bare overlap markers in content, and the
    // grammar delegates ordering/completeness checks to Rust validation.
    contents: $ => repeat1(choice(
      $.whitespaces,
      $.content_item,
      $.separator,
      $.overlap_point,
    )),

    // NOTE: No optional whitespace wrappers, contents handles its own whitespace
    main_sin_group: $ => seq(
      $.sin_begin_group,  // 〔 SIN_GROUP start delimiter
      $.contents,
      $.sin_end_group   // 〕 SIN_GROUP end delimiter
    ),

    // Note: Use restricted content to avoid infinite recursion (no nested PHO groups)
    // NOTE: No optional whitespace wrappers, contents handles its own whitespace
    main_pho_group: $ => seq(
      $.pho_begin_group,  // ‹ PHO_GROUP delimiter
      $.contents,
      $.pho_end_group   // › PHO_GROUP_END delimiter
    ),

    // Postcode annotation: [+ code] - marks utterance properties
    // Example: [+ bch] for babbling, [+ trn] for translation
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Postcodes
    postcode: $ => seq(
      token(prec(8, '[+')),
      $.space,
      field('code', $.annotation_content),
      $.right_bracket,
    ),

    // Freecode annotation: [^ code] - free-form code
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Freecodes
    // MUST stay opaque, structuring [^ as a prefix token conflicts with
    // ca_continuation_marker [^c] at the DFA level (prec(8) on [^ beats
    // the longer [^c] match).
    freecode: $ => token(/\[\^ [^\]\r\n]+\]/),

    // Langcode: [- lang], language code marker
    // Language code is 2-4 lowercase letters (ISO 639)
    langcode: $ => seq(
      token(prec(8, '[-')),
      $.space,
      field('code', $.language_code),
      $.right_bracket,
    ),

    // Free text content inside bracket annotations (postcode, freecode, etc.)
    annotation_content: $ => /[^\]\r\n]+/,

    // ============================================================================
    // OVERLAP POINTS (CA overlaps)
    // ============================================================================
    // Overlap points indicate simultaneous speech between speakers.
    // Top points (⌈⌉) mark the first speaker's overlapped portion.
    // Bottom points (⌊⌋) mark the second speaker's overlapping portion.
    // Optional digit (⌈2, ⌊2) indexes multiple overlaps in same utterance.
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Overlap
    //
    // SIMPLIFIED: Overlap point distinguishes marker types in CST.
    // Each marker can optionally have an index (2-9).
    // The four marker types are preserved as separate CST nodes.
    // ============================================================================
    // Same prec(5) as standalone_word, tie-breaking by rule order prefers overlap_point
    // when standalone (space-separated). When adjacent to text, standalone_word wins by
    // maximal munch (longer match at same prec).
    // Overlap markers: ⌈⌉⌊⌋ with optional digit suffix.
    // Accepts [1-9] at the grammar level ("parse, don't validate").
    // [0] excluded: ⌈0 is overlap_point + zero-word (action without speech).
    // Validator rejects index 1, valid CHAT range is 2-9 (E373).
    overlap_point: $ => token(prec(5, /[\u2308\u2309\u230A\u230B][1-9]?/)),

    // Overlap precedes [<] or [<N], atomic token
    // Handles: [<], [<1], [< ], [<2 ], etc.
    indexed_overlap_precedes: $ => token(prec(8, /\[< ?[1-9]? ?\]/)),

    // Overlap follows [>] or [>N], atomic token
    // Handles: [>], [>1], [> ], [>2 ], etc.
    indexed_overlap_follows: $ => token(prec(8, /\[> ?[1-9]? ?\]/)),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers
    // If they exist, they start an utterance.
    linkers: $ => repeat1(
      seq(
        $.linker,
        $.whitespaces
      )
    ),
    linker: $ => choice(
      $.linker_lazy_overlap,
      $.linker_quick_uptake,
      $.linker_quick_uptake_overlap,
      $.linker_quotation_follows,
      $.linker_self_completion,
      $.ca_technical_break_linker,
      $.ca_no_break_linker
    ),

    // All linkers use prec(10) to beat standalone_word (prec 5) at the tokenizer level.
    // Without this, '+' would be consumed as a 1-char standalone_word.
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#LazyOverlap_Linker
    linker_lazy_overlap: $ => token(prec(10, '+<')),         // +< Lazy overlap
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#QuickUptake_Linker
    linker_quick_uptake: $ => token(prec(10, '++')),         // ++ Quick uptake
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#OtherCompletion_Linker
    linker_quick_uptake_overlap: $ => token(prec(10, '+^')), // +^ Quick uptake with overlap
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#QuotedUtterance_Linker
    linker_quotation_follows: $ => token(prec(10, '+\"')),   // +" Quotation follows
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#SelfCompletion_Linker
    linker_self_completion: $ => token(prec(10, '+,')),      // +, Self-completion

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Pause_Marker
    // Coarsened to single atomic token, Rust parser dispatches on text content.
    // prec(10) ensures pause tokens beat standalone_word (prec 5) at the DFA level.
    // Matches: (.) short, (..) medium, (...) long, (3.5) or (3:2.5) timed
    pause_token: $ => token(prec(10, choice(
      '(.)',
      '(..)',
      '(...)',
      /\(\d+(?::\d+)?\.\d*\)/,
    ))),

    // ============================================================================
    // WORDS, GROUPS & ANNOTATIONS
    // ============================================================================
    // Words and nonwords (events, zero) can carry bracket annotations:
    //   word [replacement] [annotation]*
    //   <group content> [annotation]+    (groups REQUIRE annotations)
    //
    // Annotations are square-bracket constructs like [!], [= text], [//], [*].
    // Most are atomic tokens, the Rust parser extracts their internal content.
    //
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Annotations
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Error_Coding
    // ============================================================================

    // Word with optional replacement and annotations
    // Example: "wurd [: word] [!]" - misspelling with replacement and stressing
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Error_Coding
    word_with_optional_annotations: $ => seq(
      field('word', $.standalone_word),
      optional(seq(
        // A replacement `[: ...]` is a separate token and MUST be preceded by
        // whitespace, exactly like every `base_annotation` (see `base_annotations`
        // below, which already requires `$.whitespaces`). A glued `word[: foo]`
        // is invalid CHAT (CLAN CHECK error 161, "space required before [ code");
        // requiring the space here makes the glued form an ERROR node so the
        // parser rejects it, instead of silently accepting it.
        $.whitespaces,
        field('replacement', $.replacement)
      )),
      field('annotations', optional($.base_annotations))
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Group_Scopes
    // Group with annotations: <content> [annotation]
    // Groups MUST have annotations - cannot be standalone (ANTLR: annotatedGroup)
    // Example: <I dunno> [?] - uncertain transcription of phrase
    // NOTE: No optional whitespace wrappers, contents handles its own whitespace
    group_with_annotations: $ => seq(
      $.less_than,
      field('content', $.contents),
      $.greater_than,
      field('annotations', $.base_annotations)  // REQUIRED - groups must be annotated
    ),

    base_annotations: $ =>
      repeat1(
        seq(
          $.whitespaces,  // Required whitespace before each annotation
          $.base_annotation
        )
      )
    ,

    // Base annotations that can follow words, events, or groups
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Annotations
    base_annotation: $ => choice(
      $.indexed_overlap_precedes,        // [<], [<N] - overlap precedes
      $.indexed_overlap_follows,         // [>], [>N] - overlap follows
      $.scoped_stressing,                // [!] - stressing
      $.scoped_contrastive_stressing,    // [!!] - contrastive stressing
      $.scoped_uncertain,                // [?] - uncertain
      $.explanation_annotation,          // [= text] - explanation
      $.para_annotation,                 // [=! text] - paralinguistic
      $.alt_annotation,                  // [=? text] - alternative transcription
      $.percent_annotation,              // [% text] - percent annotation
      $.error_marker_annotation,         // [*] or [* code] - error marker
      $.retrace_complete,                // [//] - complete retrace
      $.retrace_partial,                 // [/] - partial retrace
      $.retrace_multiple,                // [///] - multiple retrace
      $.retrace_reformulation,           // [/-] - reformulation
      $.exclude_marker,                  // [e] - exclude from analysis
    ),

    // ============================================================================
    // WORD STRUCTURE (Opaque Token)
    // ============================================================================
    // Words are a single opaque token. All internal structure (prosody, CA markers,
    // shortenings, compound markers, form/language suffixes, POS tags) is parsed by
    // the Rust direct parser via parse_word_impl().
    //
    // The regex captures everything between word boundaries:
    //   - Optional prefix: 0 (omission), &~ (nonword), &- (filler), &+ (phon fragment)
    //   - Body: any non-boundary characters
    //
    // Boundary characters (stop the word):
    //   whitespace, [ ] < > \u0015 (bullet), control chars \u0001-\u0004 \u0007 \u0008
    //
    // The & exclusion at body start prevents swallowing events (&=), nonvocals (&{),
    // long features (&[), and other spoken events (&*), the explicit prefix alternatives
    // handle the valid &~, &-, &+ word prefixes.
    //
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Words
    // ============================================================================
    // Word token: opaque leaf, parsed internally by the direct parser.
    // Excludes characters that must tokenize as separate grammar nodes:
    //   whitespace, brackets []<>{}, bullet \u0015, controls \u0003-\u0004\u0007-\u0008,
    //   ASCII terminators/separators .?!,;
    //   intonation/separator ⇗↗→↘⇘ (\u21D7,\u2197,\u2192,\u2198,\u21D8)
    //   CA terminators ≈≋ (\u2248,\u224B), markers ∞≡„‡ (\u221E,\u2261,\u201E,\u2021)
    //   structural delimiters: ‹› (\u2039,\u203A), "" (\u201C,\u201D), 〔〕 (\u3014,\u3015)
    //   long feature/nonvocal braces {}
    //
    // First-char-only exclusions (allowed in continuation):
    //   : (colon), standalone colon is a separator; inside words it's prosodic marker
    //
    // INCLUDES (word-internal, handled by direct parser):
    //   underline controls \u0001,\u0002, split only when standalone (prec(5) tokens, rule-order tie-break)
    //   overlap markers ⌈⌉⌊⌋, split only when standalone (prec(5) tokens, rule-order tie-break)
    //
    // IMPORTANT: underline_begin/underline_end are defined BEFORE standalone_word so that
    // rule-order tie-breaking prefers them when standalone (same prec, same length).
    underline_begin: $ => token(prec(5, '\u0002\u0001')),
    underline_end: $ => token(prec(5, '\u0002\u0002')),

    // Word with full internal structure parsed by tree-sitter.
    // Prefix: &- (filler), &~ (nonword), &+ (fragment), 0 (omission)
    // Body: text segments, shortenings, stress, CA markers, compounds
    // Suffixes: @form, @s:lang, $pos (all use token.immediate, no gap)
    //
    // Zero prefix (0) is inlined directly here instead of going through word_prefix.
    // Tree-sitter's shift-reduce resolution doesn't propagate prec through intermediate
    // rules, so the zero must be at the same level as word_body for the prec to work.
    //
    // prec.right(6) resolves the zero ambiguity: when tree-sitter sees `zero • word_body`,
    // it must choose between nonword(zero) + separate word, or standalone_word(zero, word_body).
    // prec(6) beats nonword's prec(1), so 0word is ONE word.
    // Standalone 0 (no adjacent word_body) can only match nonword (extras:[] prevents
    // whitespace from being skipped between zero and word_body).
    // LINT NOTE (2026-03-24): prec(6) here does not propagate to word_body's children
    // (overlap_point, ca_element, ca_delimiter, etc.) through the word_body intermediate.
    // This is harmless because those children use token(prec(10, ...)) for DFA-level
    // disambiguation; they don't need rule-level prec inheritance from standalone_word.
    // The prec(6) here is purely for rule-level disambiguation: standalone_word wins
    // over nonword (prec 1) when zero is followed by word_body text (e.g., "0die").
    standalone_word: $ => prec.right(6, seq(
      optional(choice($.word_prefix, $.zero)),
      $.word_body,
      optional($.form_marker),
      optional($.word_lang_suffix),
      optional($.pos_tag),
    )),

    word_prefix: $ => choice(
      token('&-'),   // filler
      token('&~'),   // nonword
      token('&+'),   // phonological fragment
    ),

    // Word body: text segments interspersed with shortenings, stress,
    // lengthening colons, compound markers, and overlap markers.
    // Examples: "(be)cause", "ice+cream", "ˈhello", "no:::", "ah:", "butt⌈er⌉"
    //
    // MUST start with word_segment, shortening, stress_marker, or overlap_point.
    // Lengthening and compound marker (+) cannot start a word body.
    // This prevents space-separated ":" from forming a degenerate
    // standalone_word(word_body(lengthening)) instead of separator(colon).
    //
    // Overlap markers (⌈⌉⌊⌋) are first-class children here, not consumed by
    // word_segment. This is required for cross-utterance overlap validation
    // (E347, E348, E373, E704) which must find ALL overlap markers regardless
    // of position.
    // All non-text word content items that can appear inside a word.
    // Each is a structured child, never consumed by word_segment.
    _word_marker: $ => choice(
      $.lengthening,
      $.overlap_point,
      $.ca_element,
      $.ca_delimiter,
      $.underline_begin,
      $.underline_end,
      $.syllable_pause,
      $.tilde,
      '+',  // compound marker
    ),

    word_body: $ => prec.right(choice(
      // Standard start: text, shortening, or stress, then optional continuation
      seq(
        choice($.word_segment, $.shortening, $.stress_marker),
        repeat(choice($.word_segment, $.shortening, $.stress_marker, $._word_marker)),
      ),
      // Marker-initial: ⌈hello⌉, °hello°, °↑hello°, °°hello°°, ^hello, one or more
      // structural markers MUST be followed by text content (prevents standalone
      // markers from forming degenerate words). Multiple markers allow stacked CA
      // notation: °↑ (piano + pitch up), °° (pianissimo), ⌈° (overlap + piano).
      //
      // syllable_pause (^) is accepted in this leading position so a word-initial
      // pause (e.g. ^banana) PARSES into a structured word whose word_body begins
      // with a `syllable_pause` child, instead of failing into a line-level ERROR.
      // The pause stays a separate typed child (word_segment purity is preserved),
      // and it still MUST be followed by spoken material, so a lone `^` does not
      // form a degenerate word. The "pause not between spoken material" rule (E252)
      // is then enforced by the typed-model validator `check_prosodic_markers`,
      // reading the parsed position, NOT by scanning the raw text of an ERROR node.
      seq(
        repeat1(choice(
          $.overlap_point,
          $.ca_element,
          $.ca_delimiter,
          $.underline_begin,
          $.syllable_pause,
        )),
        choice($.word_segment, $.shortening, $.stress_marker),
        repeat(choice($.word_segment, $.shortening, $.stress_marker, $._word_marker)),
      ),
    )),

    // CA elements: individual markers within words (pitch, articulation, etc.)
    // Built from symbol registry CA_ELEMENT_SYMBOLS
    ca_element: $ => token(prec(10, new RegExp(`[${CA_ELEMENT_SYMBOLS}]`))),

    // CA delimiters: paired markers within words (tempo, voice quality, etc.)
    // Built from symbol registry CA_DELIMITER_SYMBOLS
    ca_delimiter: $ => token(prec(10, new RegExp(`[${CA_DELIMITER_SYMBOLS}]`))),

    // A word segment is a run of PURE SPOKEN TEXT characters.
    //
    // INVARIANT: word_segment contains no structural markers. All non-text
    // elements (overlap markers, CA elements, CA delimiters, underline markers,
    // stress, lengthening, etc.) are separate children in word_body.
    //
    // Character exclusions are built from the symbol registry
    // (src/generated_symbol_sets.js), single source of truth.
    // First-char also excludes: 0 (omission prefix), * (speaker), % (dep tier).
    word_segment: $ => token(prec(5, seq(
      WORD_SEGMENT_FIRST_RE,
      WORD_SEGMENT_REST_RE,
    ))),

    shortening: $ => seq('(', $.word_segment, ')'),

    stress_marker: $ => token(choice('\u02C8', '\u02CC')),  // ˈ primary, ˌ secondary

    // Lengthening: one or more colons after a vowel (no:, no:::)
    // Separate from word_segment so it's visible in the CST.
    lengthening: $ => token(prec(5, /:{1,}/)),

    // Form marker: @letter codes with optional :suffix.
    // Examples: @b, @c, @z:grm, @n:eng, @fp:is
    // The full set from the CHAT manual plus @z (user-defined).
    // Form marker: @b, @c, @z:grm, @n:eng, @fp:is, etc.
    // Consumes ALL alphabetic chars after @ so invalid markers like @dima
    // become a single token (validated by Rust parser, not silently split).
    // Excludes bare @s which must go to word_lang_suffix.
    // Valid: @u @b @c @d @f @fp @g @i @k @l @ls @n @o @p @q @sas @si @sl @t @wp @x @z
    // Invalid but parsed: @dima @ap @junk (flagged by Rust as E203)
    form_marker: $ => token.immediate(
      /@(?:s[a-zA-Z][-a-zA-Z]*|[a-rt-zA-RT-Z][-a-zA-Z]*)(?::[a-zA-Z0-9_]+)?/
    ),
    // Language suffix: @s or @s:eng or @s:eng+zho+fra or @s:eng&zho&fra
    // Single immediate token to prevent colon/& from being consumed by other rules.
    word_lang_suffix: $ => token.immediate(
      /@s(?::[a-z]{2,3}(?:[+&][a-z]{2,3})*)?/
    ),

    // POS tag: $n, $v, $adj, etc.
    pos_tag: $ => prec.right(seq(
      token.immediate('$'),
      /[a-zA-Z:]+/,
    )),
    // Label token for long features and nonvocal markers
    // Allows alphanumeric plus @ % _ - symbols
    // Used in: long_feature_begin, long_feature_end, nonvocal_begin, nonvocal_end, nonvocal_simple
    long_feature_label: $ => /[A-Za-z0-9@%_-]+/,

    // Event description text after `&=`. Stops before characters in EVENT_SEGMENT_FORBIDDEN
    // (CA symbols, brackets, bullets, newlines) so that constructs like `&=smack°uh` are
    // tokenized as event + CA-marked speech, not one giant event token.
    // prec(1), lowest word-level precedence; standalone_word (prec 5) wins for ordinary words.
    // Example: &=clears:throat, &=laughs, &=707{b}
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Local_Event
    event_segment: $ => token(prec(1, EVENT_SEGMENT_RE)),

    // Named leaf nodes for type-safe dispatch
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Stressing_Scope
    scoped_stressing: $ => token('[!]'),
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#ContrastiveStressing_Scope
    scoped_contrastive_stressing: $ => token('[!!]'),
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#UnclearRetracing_Scope
    scoped_uncertain: $ => token('[?]'),

    // Explanation annotation: [= text]
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Explanation_Scope
    explanation_annotation: $ => seq(
      token(prec(8, '[=')),
      $.space,
      field('text', $.annotation_content),
      $.right_bracket,
    ),

    // Paralinguistic annotation: [=! text]
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#ParalinguisticMaterial_Scope
    para_annotation: $ => seq(
      token(prec(8, '[=!')),
      $.space,
      field('text', $.annotation_content),
      $.right_bracket,
    ),

    // Alternative transcription: [=? text]
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#AlternativeTranscription_Scope
    alt_annotation: $ => seq(
      token(prec(8, '[=?')),
      $.space,
      field('text', $.annotation_content),
      $.right_bracket,
    ),

    // Error marker annotation: [*] or [* code]
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Error_Coding
    // Kept as opaque token, optional content after [* makes structuring risky
    error_marker_annotation: $ => token(prec(8, /\[\*[^\]]*\]/)),

    // Replacement annotation: [: replacement]
    // Example: wanna [: want to] - replacement for non-standard form
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Replacement_Scope
    replacement: $ => seq(
      $.left_bracket,
      $.colon,
      repeat1(
        seq(
          optional($.whitespaces),
          $.standalone_word
        )
      ),
      $.right_bracket
    ),

    // Percent annotation: [% text]
    percent_annotation: $ => seq(
      token(prec(8, '[%')),
      $.space,
      field('text', $.annotation_content),
      $.right_bracket,
    ),

    // Nonword with optional annotations
    // Unifies events (&=action) and zero/action (0) with optional annotations
    // Example: &=laughs [% comment], 0 [= points]
    nonword_with_optional_annotations: $ => seq(
      field('nonword', $.nonword),
      field('annotations', optional($.base_annotations))
    ),

    // Nonword: events (&=action) and standalone zero/action (0).
    // NOT other_spoken_event (&*SPK), that's a separate rule.
    //
    // prec(1) on nonword vs prec(6) on standalone_word resolves the zero ambiguity:
    // when zero is followed by word_body (0die), standalone_word wins (one word).
    // When zero is standalone (0 die), only nonword matches (extras:[] prevents
    // whitespace from being skipped between word_prefix and word_body).
    nonword: $ => prec(1, choice(
      $.event,
      $.zero,
    )),

    // Event marker: &=action
    // Example: &=clears:throat, &=laughs
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Local_Event
    event: $ => seq(
      $.event_marker,
      field('description', $.event_segment)
    ),

    // ============================================================================
    // DEPENDENT TIERS
    // ============================================================================
    // Dependent tiers provide linguistic annotations aligned with main tier content.
    // Format: %CODE:\tAnnotation content
    //
    // Major tier types:
    //   - %mor: morphological analysis (POS tags, stems, affixes)
    //   - %gra: grammatical relations (dependency parsing)
    //   - %pho: phonological transcription (IPA)
    //   - %sin: signed language / gestures
    //   - %com: comments
    //   - %act, %exp, %sit: action, explanation, situation
    //   - %eng, %gls: translations
    //   - %x*: user-defined tiers
    //
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers
    // ============================================================================
    dependent_tier: $ => choice(
      $.mor_dependent_tier,
      $.pho_dependent_tier,
      $.mod_dependent_tier,
      $.sin_dependent_tier,    // %sin gesture/sign tier
      $.gra_dependent_tier,
      $.ort_dependent_tier,
      $.com_dependent_tier,
      $.cod_dependent_tier,
      $.gls_dependent_tier,    // %gls target gloss
      $.eng_dependent_tier,    // %eng english translation
      $.int_dependent_tier,    // %int intonation
      $.act_dependent_tier,    // %act action tier - can contain bullets
      $.add_dependent_tier,    // %add addressee tier - can contain bullets
      $.err_dependent_tier,    // %err error coding tier - can contain bullets
      $.exp_dependent_tier,    // %exp explanation tier - can contain bullets
      $.gpx_dependent_tier,    // %gpx gestural/proxemic tier - can contain bullets
      $.sit_dependent_tier,    // %sit situation tier - can contain bullets
      $.tim_dependent_tier,    // %tim time tier - can contain bullets
      $.wor_dependent_tier,    // %wor word tier - can contain bullets (for timing alignment)
      $.x_dependent_tier,      // %xLABEL user-defined tiers (includes %xpho, %xmod, etc.)
      $.alt_dependent_tier,    // %alt alternative
      $.coh_dependent_tier,    // %coh cohesion
      $.def_dependent_tier,    // %def salt
      $.fac_dependent_tier,    // %fac facial
      $.flo_dependent_tier,    // %flo flow
      $.modsyl_dependent_tier, // %modsyl syllabified model phonology (Phon)
      $.phosyl_dependent_tier, // %phosyl syllabified actual phonology (Phon)
      $.phoaln_dependent_tier, // %phoaln phonological alignment (Phon)
      $.xphoint_dependent_tier, // %xphoint per-phone time intervals (Phon)
      $.par_dependent_tier,    // %par paralinguistics
      $.spa_dependent_tier,    // %spa speech act
      $.unsupported_dependent_tier,  // catch-all for unknown %label tiers
    ),

    // Catch-all for unknown dependent tiers (%custom, %foo, etc.).
    // Single greedy token so that known string prefixes (token('%mor'), etc.)
    // win by tree-sitter's "string beats regex at same length" rule, and
    // x_dependent_tier wins by prec(1) > prec(0) at same length.
    // LINT NOTE (2026-03-24): x_dependent_tier shadows unsupported_dependent_tier.
    // Intentional, %xLABEL is more specific than %LABEL catch-all.
    unsupported_dependent_tier: $ => seq(
      alias(/%[a-zA-Z][a-zA-Z0-9]*/, $.unsupported_tier_prefix),
      $.tier_sep,
      /[^\n\r]*/,
      $.newline,
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier
    mor_dependent_tier: $ => seq(
      $.mor_tier_prefix,
      $.tier_sep,
      $.mor_contents,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier
    pho_dependent_tier: $ => seq(
      $.pho_tier_prefix,
      $.tier_sep,
      $.pho_groups,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Model_Tier
    mod_dependent_tier: $ => seq(
      $.mod_tier_prefix,
      $.tier_sep,
      $.pho_groups,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Sign_Tier
    sin_dependent_tier: $ => seq(
      $.sin_tier_prefix,
      $.tier_sep,
      $.sin_groups,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier
    gra_dependent_tier: $ => seq(
      $.gra_tier_prefix,
      $.tier_sep,
      $.gra_contents,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Orthography_Tier
    // Tiers that can contain text with inline bullets
    ort_dependent_tier: $ => seq(
      $.ort_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Comment_Tier
    com_dependent_tier: $ => seq(
      $.com_tier_prefix,
      $.tier_sep,
      $.text_with_bullets_and_pics,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Coding_Tier
    cod_dependent_tier: $ => seq(
      $.cod_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#User_Tier
    // Single greedy token for the full %xLABEL prefix (e.g. %xfoo, %xpho).
    // prec(1) beats the unsupported_dependent_tier regex (prec 0) at same length.
    // Rust parser extracts the label by stripping the "%x" prefix from the token text.
    x_dependent_tier: $ => seq(
      alias(token(prec(1, /%x[a-zA-Z][a-zA-Z0-9]*/)), $.x_tier_prefix),
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Gloss_Tier
    gls_dependent_tier: $ => seq(
      $.gls_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#English_Tier
    eng_dependent_tier: $ => seq(
      $.eng_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Intonational_Tier
    int_dependent_tier: $ => seq(
      $.int_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Action_Tier
    // New tiers that can contain text with inline bullets
    act_dependent_tier: $ => seq(
      $.act_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Addressee_Tier
    add_dependent_tier: $ => seq(
      $.add_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Error_Tier
    err_dependent_tier: $ => seq(
      $.err_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Explanation_Tier
    exp_dependent_tier: $ => seq(
      $.exp_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Gestural_Tier
    gpx_dependent_tier: $ => seq(
      $.gpx_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Situation_Tier
    sit_dependent_tier: $ => seq(
      $.sit_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Timing_Tier
    tim_dependent_tier: $ => seq(
      $.tim_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Word_Tier
    // %wor tier: word-level timing annotations.
    // Uses wor_tier_body (flat structure), %wor is always a flat list of
    // words + timing bullets. Complex %wor from legacy CLAN data is a data
    // quality error that the validator detects; the parser drops broken tiers.
    wor_dependent_tier: $ => seq(
      $.wor_tier_prefix,
      $.tier_sep,
      $.wor_tier_body
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Alternate_Tier
    alt_dependent_tier: $ => seq(
      $.alt_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Cohesion_Tier
    coh_dependent_tier: $ => seq(
      $.coh_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Definitions_Tier
    def_dependent_tier: $ => seq(
      $.def_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#FacialGesture_Tier
    fac_dependent_tier: $ => seq(
      $.fac_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Flow_Tier
    flo_dependent_tier: $ => seq(
      $.flo_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Phon project syllable-level phonological tiers
    modsyl_dependent_tier: $ => seq(
      $.modsyl_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),
    phosyl_dependent_tier: $ => seq(
      $.phosyl_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),
    phoaln_dependent_tier: $ => seq(
      $.phoaln_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),
    // %xphoint: per-phone time intervals. Captured as flat text_with_bullets
    // (the 0x15 timing bullets and " / " group separators are parsed into the
    // structured XphointTier model in the Rust dispatch, like the other Phon
    // tiers).
    xphoint_dependent_tier: $ => seq(
      $.xphoint_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Paralinguistics_Tier
    par_dependent_tier: $ => seq(
      $.par_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Reference: https://talkbank.org/0info/manuals/CHAT.html#SpeechAct_Tier
    spa_dependent_tier: $ => seq(
      $.spa_tier_prefix,
      $.tier_sep,
      $.text_with_bullets,
      $.newline
    ),

    // Quotation: "quoted speech"
    // Uses curly/smart quotes to delimit quoted material
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Quotation
    // NOTE: No optional whitespace wrappers, contents handles its own whitespace
    quotation: $ => seq(
      $.left_double_quote,
      $.contents,
      $.right_double_quote
    ),

    // Quotation mark delimiters
    left_double_quote: $ => token(prec(10, '\u201C')),   // " U+201C LEFT DOUBLE QUOTATION MARK
    right_double_quote: $ => token(prec(10, '\u201D')),  // " U+201D RIGHT DOUBLE QUOTATION MARK

    // Illegal curly single quotation marks (U+2018/U+2019). These are NOT
    // legal CHAT word characters: CHAT requires the ASCII apostrophe ('),
    // and the curly singles are typically introduced by autocorrect or ASR.
    // word_segment already excludes them (symbol registry), so they surface
    // here as a recognized illegal-character node rather than a bare ERROR;
    // the Rust parser emits E256 (CLAN CHECK 138/139). Unlike the curly
    // doubles above, these are never legal delimiters, so there is no group.
    illegal_curly_quote: $ => token(prec(10, choice('\u2018', '\u2019'))),

    colon: $ => ':',

    // Chat.flex: WHO = [A-Za-z0-9_\'+\-]+ (exact match)
    speaker: $ => /[A-Za-z0-9_\'+\-]+/, 

    // ============================================================================
    // MORPHOLOGY (MOR) TIER INTERNALS
    // ============================================================================
    // The %mor tier contains UD-style morphological analysis aligned word-by-word
    // with the main tier. Each MOR word has this structure:
    //
    //   POS|lemma[-Feature]*
    //
    // Words can be connected by post-clitics (MWT expansions from Stanza):
    //   - Post-clitics: ~word  (e.g., pron|I~aux|be-Fin-Ind-Pres-S1)
    //
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier
    // ============================================================================

    // Can have zero content with only terminator.
    // Terminator is optional - aligner validates matching with main tier.
    mor_contents: $ => seq(
      choice(
        // Pattern 1: Items with optional terminator
        seq(
          $.mor_content,                              // First item (required)
          repeat(seq($.whitespaces, $.mor_content)),  // Additional items
          optional(seq($.whitespaces, $.terminator))  // Optional terminator
        ),
        // Pattern 2: Just terminator (e.g., "%mor:\t.")
        $.terminator
      ),
      optional($.whitespaces)  // Trailing whitespace before newline
    ),

    // MOR content: main word + optional post-clitics (NO pre-clitics, NO translation)
    mor_content: $ => seq(
      field('main', $.mor_word),
      field('post_clitics', repeat($.mor_post_clitic))
    ),

    // morPostClitic = TILDE morWord
    mor_post_clitic: $ => seq($.tilde, $.mor_word),

    // mor_word: POS|lemma[-Feature]*
    mor_word: $ => seq(
      $.mor_pos,                  // UPOS tag (simple identifier)
      $.pipe,                     // |
      $.mor_lemma,                // lemma (opaque Unicode string)
      repeat($.mor_feature)       // -Feature chains
    ),

    // POS tag, allows colons for subcategories (pro:sub, v:aux, det:art, n:prop).
    // POS subcategories are fundamental in CHAT, CLAN's MOR generates them by default.
    // Allows: Unicode letters, digits, colon (subcategory separator)
    // Excludes: | - ~ + . ? ! # $ @ % = & , [ ] < > ( ) space
    mor_pos: $ => /[^\.\?\!\|\+~\$#@%=&\[\]<>()\-,\s\r\n\u201C\u201D]+/,

    // Lemma, opaque string, allows = (Estonian compound boundary), ! (Basque
    // derivational boundary), apostrophe, underscore, en-dash, colon, comma,
    // and all Unicode letters/digits.
    // First char excludes = & # ! to prevent legacy-format ambiguity.
    // Excludes: | (POS/lemma sep), - (feature sep), ~ (clitic), + . ? space, and CHAT markers
    mor_lemma: $ => /[^\.\?\!\|\+~\$#@%=&\[\]<>()\-\s\r\n\u201C\u201D][^\.\?\|\+~\-\s\r\n]*/,

    // Feature: -Value (commas allowed within value, e.g., -Int,Rel)
    mor_feature: $ => seq($.hyphen, $.mor_feature_value),
    mor_feature_value: $ => /[^\.\?\|\+~\-\s\r\n]+/,

    // ============================================================================
    // GRAMMATICAL RELATIONS (GRA) TIER
    // ============================================================================
    // The %gra tier encodes dependency relations as: index|head|RELATION
    // Example: 1|2|SUBJ 2|0|ROOT 3|2|OBJ, word 1 is subject of word 2 (the root)
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Grammatical_Relations
    // ============================================================================

    gra_contents: $ => seq(
      $.gra_relation,
      repeat(
        seq(
          $.whitespaces,
          $.gra_relation
        )
      )
    ),

    // GRA relation: index|head|relation (dependency structure)
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Grammatical_Relations
    // Example: 1|2|SUBJ means word 1 is subject of word 2
    gra_relation: $ => seq(
      field('index', $.gra_index),
      $.pipe,
      field('head', $.gra_head),
      $.pipe,
      field('relation', $.gra_relation_name)
    ),

    gra_index: $ => /[0-9]+/, 

    gra_head: $ => /[0-9]+/, 

    // Chat.flex: GRA_RELATION = [A-Z] [A-Z0-9\-]*
    gra_relation_name: $ => /[A-Z][A-Z0-9\-]*/,

    // ============================================================================
    // PHONOLOGY (PHO) & SIGN (SIN) TIERS
    // ============================================================================
    // %pho: phonological transcription in IPA, one word per main tier word.
    //   Words can be grouped with ‹...› delimiters for multi-word grouping.
    //   Plus (+) joins compound words.
    //
    // %sin: sign/gesture notation, one entry per main tier word.
    //   Words can be grouped with 〔...〕 delimiters.
    //   Supports colon-separated structures (e.g., g:toy:dpoint).
    //
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Sign_Tier
    // ============================================================================

    pho_groups: $ => seq(
      $.pho_group,
      repeat(
        seq(
          $.whitespaces,
          $.pho_group
        )
      )
    ),

    // Enhanced PHO grouping support with proper delimiter handling
    pho_group: $ => choice(
      $.pho_words,
      seq(
        $.pho_begin_group,  // ‹ PHO_GROUP delimiter
        $.pho_grouped_content,
        $.pho_end_group   // › PHO_GROUP_END delimiter
      )
    ),

    // PHO grouped content with proper space handling between pho_words
    pho_grouped_content: $ => seq(
      $.pho_words,
      repeat(
        seq(
          $.whitespaces,
          $.pho_words
        )
      )
    ),

    pho_words: $ => seq(
      $.pho_word,
      repeat(
        seq(
          $.plus,
          $.pho_word
        )
      )
    ),

    // PHO_WORD = {PHONE}+ from Chat.flex
    // PHONE = {IPA_PHON_ALLOWED} | {COVER_PHO_CHAR} | [\(\.\)\^\u0335]
    // Simplified for common phonological symbols
    // Split large Unicode range to exclude PHO grouping delimiters \u2039 (‹) and \u203A (›)
    pho_word: $ => /[a-zA-Z0-9\u0061-\u007a\u00e6-\u2038\u203b-\ua71c\u0250-\u02af\u1d00-\u1dbf\u2016\u203f\u207f\u2197-\u2198\u2c71\u2e28-\u2e29\ua71b-\ua71cCGVSX\(\.\)\^\u0335*]+/,

    sin_groups: $ => seq(
      $.sin_group,
      repeat(
        seq(
          $.whitespaces,
          $.sin_group
        )
      )
    ),

    // Enhanced SIN grouping support with proper delimiter handling
    // Uses Unicode brackets 〔...〕 for grouping as seen in examples
    sin_group: $ => choice(
      $.sin_word,
      seq(
        $.sin_begin_group,  // 〔 SIN_GROUP start delimiter
        $.sin_grouped_content,
        $.sin_end_group   // 〕 SIN_GROUP end delimiter
      )
    ),

    // SIN grouped content with proper space handling between sin_words
    sin_grouped_content: $ => seq(
      $.sin_word,
      repeat(
        seq(
          $.whitespaces,
          $.sin_word
        )
      )
    ),

    // SIN_WORD from ANTLR - handles gesture/sign notation
    // Based on examples: g:toy:dpoint, 0, b, c, d, etc.
    // Supports colon-separated structure and zero markers
    sin_word: $ => choice(
      $.zero,  // Zero marker for no gesture/sign
      /[a-zA-Z0-9:_-]+/  // General sin word pattern including colon-separated structures
    ),

    // ============================================================================
    // HEADER CONTENT PARSING
    // ============================================================================
    // Structured content rules for specific headers. Each header with structured
    // content (not just $.free_text) has a dedicated _contents rule that validates
    // the expected format at parse time.
    // ============================================================================

    // @Languages: comma-separated ISO 639 language codes (2-4 chars)
    languages_contents: $ => seq(
      $.language_code,
      repeat(seq(
        optional($.whitespaces),   // Zero or more whitespace before comma
        $.comma,
        $.whitespaces,             // One or more whitespace after comma (required)
        $.language_code
      ))
    ),

    // Language codes like "eng", "fra", "deu", "zho", "ind", "jav" etc.
    // Lower precedence to avoid conflicts with word_segment in main content
    language_code: $ => /[a-z]{2,4}/, 

    // ANTLR: mediaInfo: mediaHeaderFilename COMMA_SPACE mediaTypes
    // mediaHeaderFilename: MEDIA_URL | BULLET_FILENAME
    // mediaTypes: (MEDIA_TYPE COMMA_SPACE)* MEDIA_TYPE
    media_contents: $ => seq(
      $.media_filename,
      $.comma,
      $.whitespaces,
      $.media_type,
      optional(seq(
        $.comma,
        $.whitespaces,
        $.media_status
      ))
    ),

    // Media filename - can be quoted URLs or simple filenames
    media_filename: $ => choice(
      seq($.double_quote, /[^"\r\n]+/, $.double_quote),  // Quoted URLs like "https://..."
      /[a-zA-Z0-9_-]+/         // Simple filenames like "media-file"
    ),

    media_type: $ => choice($.video_value, $.audio_value, $.missing_value, $.generic_media_type),
    generic_media_type: $ => /[a-zA-Z]+/,

    media_status: $ => choice($.missing_value, $.unlinked_value, $.notrans_value, $.generic_media_status),
    generic_media_status: $ => /[a-zA-Z]+/,

    // Options contents - comma-separated list of chat options
    options_contents: $ => seq(
      $.option_name,
      repeat(seq(
        $.comma,
        $.whitespaces,
        $.option_name
      ))
    ),

    // Chat.flex: OPTION = "CA" | "NoAlign" | <generic>
    // Known values are kept as distinct CST nodes for syntax highlighting.
    // Unrecognized values match generic_option_name and are flagged by the validator.
    option_name: $ => choice('CA', 'NoAlign', $.generic_option_name),
    generic_option_name: $ => /[^\s,\r\n\t]+/,

    // ANTLR: dateHeader: DATE TAB DATE_ALL NEWLINE
    // Chat.flex: DATE_ALL = {DATE_DAY}"-"{DATE_MONTH}"-"{DATE_YEAR}
    // Strict match for valid DD-MMM-YYYY dates; generic catch-all for malformed dates
    // that the validator flags as E518.
    date_contents: $ => choice($.strict_date, $.generic_date),
    strict_date: $ => token(/(?:0[1-9]|[1-2][0-9]|3[0-1])-(?:JAN|FEB|MAR|APR|MAY|JUN|JUL|AUG|SEP|OCT|NOV|DEC)-[1-2][0-9]{3}/),
    generic_date: $ => /[^\r\n]+/,

    // Age format: years;months or years;months.days
    // MUST remain a single token, structuring into seq() causes tree-sitter
    // to hang on error recovery because `;` and `.` are also used elsewhere
    // in the grammar (semicolon separator, period terminator).
    // Rust parses via tokens::parse_age_format_token().
    age_format: $ => token(/[0-9]+;[0-9]{1,2}(\.[0-9]{0,2})?/),
    // ANTLR: PAGE_N for page numbers
    page_number: $ => /[0-9]+/, 

    // Chat.flex: DUR_TIMES = ({N} | [:\-;,])+
    // Patterns: "17:30-18:00", "8:30:31"
    // Strict match for digit-and-separator patterns; generic catch-all for malformed
    // values that the validator flags as E541/E542.
    time_duration_contents: $ => choice($.strict_time, $.generic_time),
    strict_time: $ => token(/[0-9:\-;,]+/),
    generic_time: $ => /[^\r\n]+/,

    // Header substructure

    // @ID header content: |lang|corpus|speaker|age|sex|group|ses|role|education|custom|
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#ID_Header
    // Split into hidden subrules for state count optimization
    // NOTE: field() declarations cause 5x state explosion - access children by type instead
    id_contents: $ => seq(
      $._id_identity_fields,
      $._id_demographic_fields,
      $._id_role_fields
    ),

    // Identity fields: language, corpus, speaker - who is this participant?
    // optional($.whitespaces) around optional fields so the grammar consumes
    // leading/trailing spaces instead of including them in field text.
    _id_identity_fields: $ => seq(
      $.id_languages,
      $.pipe,
      optional($.whitespaces), optional($.id_corpus), optional($.whitespaces),
      $.pipe,
      $.id_speaker,
      $.pipe
    ),

    // Demographic fields: age, sex, group, SES - participant characteristics
    _id_demographic_fields: $ => seq(
      optional($.whitespaces), optional($.id_age), optional($.whitespaces),
      $.pipe,
      optional($.whitespaces), optional($.id_sex), optional($.whitespaces),
      $.pipe,
      optional($.whitespaces), optional($.id_group), optional($.whitespaces),
      $.pipe,
      optional($.whitespaces), optional($.id_ses), optional($.whitespaces),
      $.pipe
    ),

    // Role fields: role, education, custom - participation details
    _id_role_fields: $ => seq(
      $.id_role,
      $.pipe,
      optional($.whitespaces), optional($.id_education), optional($.whitespaces),
      $.pipe,
      optional($.whitespaces), optional($.id_custom_field), optional($.whitespaces),
      $.pipe
    ),

    // More flexible patterns for ID fields that can contain various characters
    id_languages: $ => choice(
      $.languages_contents,              // Reuse structured comma-separated language codes
      /[^|\r\n]*/                       // Fallback for other formats
    ),
    // Optional fields use a trimming regex that requires at least one non-space
    // character and does not match leading/trailing whitespace.  This lets the
    // surrounding optional($.whitespaces) consume spaces while the field captures
    // only the meaningful content.
    // Pattern: /[^ \t|\r\n]([^|\r\n]*[^ \t|\r\n])?/
    //   - first char: non-space, non-tab, non-pipe, non-newline
    //   - optional middle + last: anything non-pipe/newline, ending non-space/tab
    id_corpus: $ => TRIMMED_PIPE_FIELD,
    id_speaker: $ => /[^|\r\n]*/,         // Required, no trimming needed
    id_age: $ => choice(
      $.age_format,
      /[^ \t|\r\n]([^|\r\n]*[^ \t|\r\n])?/  // Trimming catch-all
    ),
    // Known sex values + generic catch-all for unknown values
    // that the validator flags as E542.
    id_sex: $ => choice(
      $.male_value,
      $.female_value,
      $.generic_id_sex
    ),
    generic_id_sex: $ => /[^ \t|\r\n]([^|\r\n]*[^ \t|\r\n])?/,
    id_group: $ => TRIMMED_PIPE_FIELD,
    // Known SES values (ethnicity, socioeconomic code, or combined) + generic catch-all
    // for unknown values that the validator flags as E546.
    // Ethnicity: White Black Latino Asian Pacific Native Multiple Unknown
    // SES codes: WC UC MC LI
    // Combined: "White,MC" or "White MC"
    //
    // Uses token(prec(1, ...)) with regex (not string literals) to avoid creating
    // global keywords. Words like White, Black, Native, Multiple, Unknown appear
    // commonly in utterance text and would cause widespread parse failures as keywords.
    // Combined format is a single token to beat generic_id_ses in length-based
    // lexer disambiguation.
    // LINT NOTE (2026-03-24): ethnicity_value, ses_code_value, and ses_combined at
    // prec(1) shadow generic_id_ses at prec(0). This is the intentional strict+catch-all
    // pattern, known values get named nodes, unknown values fall to the generic catch-all
    // and are flagged by the Rust validator (E546).
    id_ses: $ => choice(
      $.ses_combined,
      $.ses_code_value,
      $.ethnicity_value,
      $.generic_id_ses
    ),
    // Single token matching "Ethnicity[, ]SesCode", must be one token to beat
    // the generic catch-all in tree-sitter's longest-match lexer.
    ses_combined: $ => token(prec(1,
      /(White|Black|Latino|Asian|Pacific|Native|Multiple|Unknown)[, ](WC|UC|MC|LI)/
    )),
    ethnicity_value: $ => token(prec(1,
      /White|Black|Latino|Asian|Pacific|Native|Multiple|Unknown/
    )),
    ses_code_value: $ => token(prec(1, /WC|UC|MC|LI/)),
    generic_id_ses: $ => /[^ \t|\r\n]([^|\r\n]*[^ \t|\r\n])?/,
    // Role values are common English words (Child, Adult, Group, Text, etc.)
    // that would conflict as global tree-sitter keywords. Vocabulary is validated
    // in Rust via is_allowed_participant_role() (E532).
    id_role: $ => /[^|\r\n]*/,            // Required, no trimming needed
    id_education: $ => TRIMMED_PIPE_FIELD,
    id_custom_field: $ => TRIMMED_PIPE_FIELD,

    // Participant entry: CODE Name Role
    // Example: CHI Child, MOT Mother
    // Reference: https://talkbank.org/0info/manuals/CHAT.html#Participants_Header
    participant: $ => seq(
      field('code', $.speaker), // 3-letter speaker code (CHI, MOT, FAT, etc.)
      repeat(seq(
        $.whitespaces,
        $.participant_word // Name and/or role words
      )),
      // May want to make strict.
      optional($.whitespaces)
    ),

    participant_word: $ => /[^, \r\n\t]+/, 

    // @Participants header: comma-separated participant info, e.g. 
    // "CHI OptionalChildsName Child, MOT Mother, FAT Father"
    // ANTLR: participants: PARTIES TAB participant (COMMA_SPACE participant)* NEWLINE
    participants_contents: $ => seq(
      $.participant,
      repeat(seq(
        $.comma,
        $.whitespaces,
        $.participant
      ))
    ),

    // Chat.flex: RECORDING_OPTION = "1" | "2" | "3" | "4" | "5" | <generic>
    recording_quality_option: $ => choice('1', '2', '3', '4', '5', $.generic_recording_quality),
    generic_recording_quality: $ => /[^\s\r\n\t]+/,

    // Chat.flex: TRANSCRIPTION_OPTION = known values | <generic>
    transcription_option: $ => choice(
      'eye_dialect',
      'partial',
      'full',
      'detailed',
      'coarse',
      'checked',
      'anonymized',
      $.generic_transcription
    ),
    generic_transcription: $ => /[^\s\r\n\t]+/,

    // Chat.flex: NUMBER_OPTION = known values | <generic>
    number_option: $ => choice('1', '2', '3', '4', '5', 'more', 'audience', $.generic_number),
    generic_number: $ => /[^\s\r\n\t]+/,

    // Named whitespace token, required because `extras: []` means tree-sitter does NOT
    // skip whitespace automatically. All whitespace in CHAT must be grammar-visible so that
    // continuation lines (newline + tab for multi-line content) are handled correctly.
    // This token matches one or more spaces or continuation sequences.
    whitespaces: $ => token(repeat1(choice(' ', /[\r\n]+\t/))),

    // Header and tier separators: ":" plus a single tab.
    header_sep: $ => seq($.colon, $.tab),
    tier_sep: $ => seq($.colon, $.tab),
    header_gap: $ => repeat1(choice($.space, $.tab)),

    // ============================================================================
    // HEADER & TIER PREFIXES
    // ============================================================================
    // Literal string tokens for header (@...) and tier (%...) prefixes.
    // Using token() for these ensures the DFA matches them as single units,
    // winning over regex-based catch-all rules by tree-sitter's "string beats
    // regex at same length" rule. Multi-word headers (e.g., '@Birth of') use
    // token() to win by longest-match over single-word @-header patterns.
    // ============================================================================

    birth_of_prefix: $ => token('@Birth of'),
    birthplace_of_prefix: $ => token('@Birthplace of'),
    l1_of_prefix: $ => token('@L1 of'),
    languages_prefix: $ => token('@Languages'),
    participants_prefix: $ => token('@Participants'),
    id_prefix: $ => token('@ID'),
    media_prefix: $ => token('@Media'),
    location_prefix: $ => token('@Location'),
    number_prefix: $ => token('@Number'),
    recording_quality_prefix: $ => token('@Recording Quality'),
    room_layout_prefix: $ => token('@Room Layout'),
    tape_location_prefix: $ => token('@Tape Location'),
    time_duration_prefix: $ => token('@Time Duration'),
    time_start_prefix: $ => token('@Time Start'),
    transcriber_prefix: $ => token('@Transcriber'),
    transcription_prefix: $ => token('@Transcription'),
    warning_prefix: $ => token('@Warning'),
    activities_prefix: $ => token('@Activities'),
    bck_prefix: $ => token('@Bck'),
    bg_prefix: $ => token('@Bg'),
    blank_prefix: $ => token('@Blank'),
    comment_prefix: $ => token('@Comment'),
    date_prefix: $ => token('@Date'),
    eg_prefix: $ => token('@Eg'),
    g_prefix: $ => token('@G'),
    new_episode_prefix: $ => token('@New Episode'),
    situation_prefix: $ => token('@Situation'),
    page_prefix: $ => token('@Page'),
    options_prefix: $ => token('@Options'),
    font_prefix: $ => token('@Font'),
    window_prefix: $ => token('@Window'),
    color_words_prefix: $ => token('@Color words'),
    videos_prefix: $ => token('@Videos'),
    types_prefix: $ => token('@Types'),
    pid_prefix: $ => token('@PID'),
    thumbnail_prefix: $ => token('@Thumbnail'),
    t_prefix: $ => token('@T'),

    mor_tier_prefix: $ => token('%mor'),
    pho_tier_prefix: $ => token('%pho'),
    mod_tier_prefix: $ => token('%mod'),
    sin_tier_prefix: $ => token('%sin'),
    gra_tier_prefix: $ => token('%gra'),
    ort_tier_prefix: $ => token('%ort'),
    com_tier_prefix: $ => token('%com'),
    cod_tier_prefix: $ => token('%cod'),
    gls_tier_prefix: $ => token('%gls'),
    eng_tier_prefix: $ => token('%eng'),
    int_tier_prefix: $ => token('%int'),
    act_tier_prefix: $ => token('%act'),
    add_tier_prefix: $ => token('%add'),
    err_tier_prefix: $ => token('%err'),
    exp_tier_prefix: $ => token('%exp'),
    gpx_tier_prefix: $ => token('%gpx'),
    sit_tier_prefix: $ => token('%sit'),
    tim_tier_prefix: $ => token('%tim'),
    wor_tier_prefix: $ => token('%wor'),
    alt_tier_prefix: $ => token('%alt'),
    coh_tier_prefix: $ => token('%coh'),
    def_tier_prefix: $ => token('%def'),
    fac_tier_prefix: $ => token('%fac'),
    flo_tier_prefix: $ => token('%flo'),
    // Phon project tiers: accept both the historical non-x names and the
    // x-prefixed extension names Phon actually emits (%xmodsyl, ...). prec(2)
    // outranks the generic x_dependent_tier regex (prec(1)) at equal length, so
    // these route to their structured tier kinds instead of a generic %x tier.
    modsyl_tier_prefix: $ => token(prec(2, choice('%modsyl', '%xmodsyl'))),
    phosyl_tier_prefix: $ => token(prec(2, choice('%phosyl', '%xphosyl'))),
    phoaln_tier_prefix: $ => token(prec(2, choice('%phoaln', '%xphoaln'))),
    xphoint_tier_prefix: $ => token(prec(2, choice('%phoint', '%xphoint'))),
    par_tier_prefix: $ => token('%par'),
    spa_tier_prefix: $ => token('%spa'),

    // Values
    male_value: $ => 'male',
    female_value: $ => 'female',
    video_value: $ => 'video',
    audio_value: $ => 'audio',
    missing_value: $ => 'missing',
    unlinked_value: $ => 'unlinked',
    notrans_value: $ => 'notrans',

    // NOTE: SES ethnicity/code named values are defined inline in id_ses above
    // using token(prec(1, regex)) to avoid global keyword conflicts.
    
    // ============================================================================
    // DEPENDENT TIER GROUP DELIMITERS
    // ============================================================================
    // Delimiters for phonological (%pho) and sin (%sin) tier groupings
    // ============================================================================

    // Long feature markers (extended prosodic features)
    long_feature_begin_marker: $ => '{l=',  // Start of long feature span
    long_feature_end_marker: $ => '}l=',    // End of long feature span

    // Nonvocal markers (non-speech sounds)
    nonvocal_begin_marker: $ => '{n=',      // Start of nonvocal span
    nonvocal_end_marker: $ => '}n=',        // End of nonvocal span

    // PHO tier group delimiters (phonological tier)
    pho_begin_group: $ => token(prec(10, '\u2039')),   // ‹ U+2039 SINGLE LEFT-POINTING ANGLE QUOTATION MARK
    pho_end_group: $ => token(prec(10, '\u203A')),     // › U+203A SINGLE RIGHT-POINTING ANGLE QUOTATION MARK

    // SIN tier group delimiters (situational tier)
    sin_begin_group: $ => token(prec(10, '〔')),    // 〔 U+3014 LEFT TORTOISE SHELL BRACKET
    sin_end_group: $ => token(prec(10, '〕')),      // 〕 U+3015 RIGHT TORTOISE SHELL BRACKET

  },
});
