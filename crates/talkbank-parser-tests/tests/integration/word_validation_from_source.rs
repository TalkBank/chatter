// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Word-level validation over words the PARSER built, not words a test did.
//!
//! # What this replaces, and why it had to move crates
//!
//! `talkbank-model`'s `validation/word/tests.rs` asserted these same rules
//! against hand-built `Word` values: a raw text `"+word"` beside a content list
//! `[CompoundMarker, Text("word")]`, with nothing forcing the two to agree.
//! That is the input stated twice, and the coverage it produces is
//! fabrication-backed: the validator runs, the regions count as covered, and
//! no CHAT text was ever read.
//!
//! Why it moved crates, stated accurately: not because a test in
//! `talkbank-model` cannot parse. Cargo permits a dev-dependency cycle, an
//! integration test under `talkbank-model/tests/` links against the non-test
//! lib, and this workspace uses that pattern on purpose (`talkbank-derive`
//! dev-depends on `talkbank-model`). What cannot parse is a `#[cfg(test)]
//! mod` inside `src/`, where these tests were: the lib-test target is a second
//! instantiation of the crate, so the parser's `Word` and the test's are
//! different types. This crate is the cheaper home, since it exists for
//! parse-backed tests and already depends on every parser. Measured while
//! moving: `Word` has no checked constructor at all, so there was no in-crate
//! improvement to take first.
//!
//! # What the move found that the originals could not
//!
//! `Word::simple("un+do")` produces content `[Text("un+do")]` with NO
//! `CompoundMarker`, so the deleted `test_valid_compound` never reached the
//! check it was named for and passed vacuously. The two rows that replace it
//! go through `parse_word`, which builds the marker. That is a coverage gain
//! nobody had flagged.
//!
//! # One table, and it is the point
//!
//! Each row is a word AS WRITTEN and what validation must say about it. The
//! structure is whatever `parse_word` builds, which is the structure real CHAT
//! produces, so a row cannot describe a `Word` the parser would never make.
//! The old file's thirty-odd functions each restated the harness around one
//! assertion; the rules they pin are unchanged.

use talkbank_model::model::{LanguageCode, TranscriptName, Word};
use talkbank_model::validation::{Validate, ValidationContext};
use talkbank_model::{ChatParser, ErrorCollector, ParseError, ParseOutcome};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;
use talkbank_parser_tests::from_source::SingleSpeaker;
use talkbank_parser_tests::test_error::TestError;

/// What a row asserts about one code.
///
/// Not a `bool`. Half of these rows are the LEGAL control for the row above
/// them, and a control is a different claim from a violation rather than its
/// negation: "this shape is fine" survives a change to which code fires, and
/// `!reports(E244)` does not.
#[derive(Clone, Copy, Debug)]
enum Expect {
    /// The word breaks this rule and the code must be reported.
    Violates,
    /// The word is a legal instance of the shape the rule is about, so the
    /// code must NOT be reported. Other diagnostics may still apply.
    Legal,
    /// This shape does not parse as a word at all, so the rule cannot be
    /// reached through it.
    ///
    /// A row rather than a deletion, because it is the finding. The hand-built
    /// version asserted the rule on a structure it built itself, so it could
    /// not notice that no parse produces that structure; recording the
    /// negative keeps the fact measured instead of remembered, and turns red
    /// if a grammar change ever makes the shape parse.
    DoesNotParse,
}

/// One word, the rule it is about, and what that rule must say.
struct Row {
    /// The word exactly as a transcriber would write it.
    word: &'static str,
    /// The code this row is about.
    code: &'static str,
    expect: Expect,
    /// The language context, when the rule depends on one.
    ///
    /// `None` is not "no opinion": E220 is deliberately off when no language
    /// is declared, so the absent case is a row of its own below.
    language: Option<&'static str>,
}

const fn row(word: &'static str, code: &'static str, expect: Expect) -> Row {
    Row {
        word,
        code,
        expect,
        language: None,
    }
}

const fn in_language(
    word: &'static str,
    code: &'static str,
    expect: Expect,
    language: &'static str,
) -> Row {
    Row {
        word,
        code,
        expect,
        language: Some(language),
    }
}

/// Every rule the old file pinned, with its own legal control beside it.
const ROWS: &[Row] = &[
    // Compound boundaries: a `+` needs a segment on each side.
    //
    // E232's own shape does not parse under the CANONICAL parser. Measured
    // 2026-09-08 through both routes: `parse_word("+word")` reports E330, and
    // `*CHI:` + tab + `+word .` in a whole file reports E316 and E342. The
    // re2c backend DOES build the `Word` and reports E232 on the same file,
    // which is what `UNDEMONSTRATED` in `error_code_demonstration.rs` records
    // and why the code stays there rather than being called unreachable. The
    // rule's MESSAGE is pinned in `compound_marker_messages_name_the_fault`
    // below through that backend; until 2026-09-08 it was pinned only by an
    // `insta` snapshot over a hand-built `Word`, which `cargo insta accept`
    // would have absorbed a regression into silently. The predecessor here
    // asserted E232 against a `Word` it built itself and so could not see any
    // of this.
    row("+word", "E232", Expect::DoesNotParse),
    row("word+", "E233", Expect::Violates),
    row("un++do", "E233", Expect::Violates),
    row("un+do", "E232", Expect::Legal),
    row("un+do", "E233", Expect::Legal),
    // Stress markers.
    row("\u{2c8}\u{2cc}test", "E244", Expect::Violates),
    row("\u{2c8}syl\u{2cc}lable", "E244", Expect::Legal),
    row("test\u{2c8}", "E245", Expect::Violates),
    row("\u{2c8}test", "E245", Expect::Legal),
    row("\u{2c8}test\u{2c8}word", "E247", Expect::Violates),
    row("\u{2c8}test", "E247", Expect::Legal),
    row("\u{2cc}test", "E250", Expect::Violates),
    row("\u{2c8}test\u{2cc}word", "E250", Expect::Legal),
    // Lengthening needs something to lengthen.
    //
    // The predecessor asserted E246 for `:test`, and that is not what CHAT
    // says today: a leading `:` is a separator glued to what follows, which is
    // E765, and `spec/errors/E246.md` records `:hello` as subsumed by E765 in
    // its first example. The spec agreed with the deleted test until
    // 2026-09-05, when 0.19.0 made that ruling; the test kept asserting the
    // old reading because it built a `Word` carrying a leading `Lengthening`,
    // which no parse produces, so nothing could contradict it. Example 2 of
    // that spec is the real shape.
    row(":test", "E246", Expect::DoesNotParse),
    row("(he):", "E246", Expect::Violates),
    row("hel:o", "E246", Expect::Legal),
    row("ba:nana", "E246", Expect::Legal),
    // A word-internal pause needs material on both sides.
    row("^test", "E252", Expect::Violates),
    row("test^", "E252", Expect::Violates),
    row("rhi^noceros", "E252", Expect::Legal),
    // Digits are a policy per language, so each arm is its own row: the rule
    // is OFF with no declared language, which is a decision rather than an
    // oversight and reads as one only if it is asserted.
    in_language("hello123", "E220", Expect::Violates, "eng"),
    row("hello123", "E220", Expect::Legal),
    // Every language configured to allow numerals, one row each. The
    // predecessor looped a `vec!` of these inside one test, which reports the
    // first failure and stops; as rows, a language that starts reporting E220
    // is named alongside every other that still does not.
    in_language("word123", "E220", Expect::Legal, "zho"),
    in_language("word123", "E220", Expect::Legal, "cym"),
    in_language("word123", "E220", Expect::Legal, "vie"),
    in_language("word123", "E220", Expect::Legal, "tha"),
    in_language("word123", "E220", Expect::Legal, "nan"),
    in_language("word123", "E220", Expect::Legal, "yue"),
    in_language("word123", "E220", Expect::Legal, "min"),
    in_language("word123", "E220", Expect::Legal, "hak"),
];

/// Words that must validate with NO diagnostic of any code.
///
/// Stronger than a `Legal` row, which clears one code. These were `insta`
/// snapshots reading `[No errors]` in `talkbank-model`'s
/// `validation/word/snapshot_tests.rs`, each over a `WordContent` list the
/// test had written by hand; one of them put a stress marker in a content
/// list for the text `WOrd`, which contains none. Here each is the word as a
/// transcriber writes it.
const CLEAN: &[&str] = &[
    // A well-formed compound.
    "ice+cream",
    // A lengthening colon inside a word.
    "wo:rd",
    // Primary then secondary stress, each before a syllable.
    "\u{2c8}syl\u{2cc}lable",
    // A syllable pause inside a word.
    "wo^rd",
    // Nested CA delimiters, balanced: softer around faster.
    "\u{b0}\u{2206}fast\u{2206}\u{b0}",
];

/// Validate one parsed word, in the row's language context.
fn diagnostics(word: &Word, language: Option<&LanguageCode>) -> Vec<ParseError> {
    let declared: Vec<LanguageCode> = language.into_iter().cloned().collect();
    let context = ValidationContext::new()
        .with_declared_languages(declared)
        .with_tier_language(language.cloned());
    let errors = ErrorCollector::new();
    word.validate(&context, &errors);
    errors.into_vec()
}

/// Every row's rule holds for the word the parser actually builds.
///
/// SURVIVES a type change, and says which category: this is behaviour at the
/// PARSER-to-VALIDATOR seam over real CHAT text. No signature relates the
/// bytes `"+word"` to the content list a parse produces for them, and that
/// relation is exactly what the hand-built version could not check.
///
/// Reported in one batch rather than one `#[test]` per row: a failure here is
/// most often a change to what the parser builds, which moves several rows at
/// once, and thirty separate red tests say that worse than one list does.
#[test]
fn every_word_rule_holds_for_a_parsed_word() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let mut wrong = Vec::new();

    for row in ROWS {
        let language = match row.language {
            Some(code) => Some(
                LanguageCode::new(code)
                    .map_err(|err| TestError::Failure(format!("{code}: {err:?}")))?,
            ),
            None => None,
        };

        let parsed = parser.parse_word(row.word);
        let word = match (&parsed, row.expect) {
            (Err(_), Expect::DoesNotParse) => continue,
            (Ok(_), Expect::DoesNotParse) => {
                wrong.push(format!(
                    "{:?}: parses as a word now, and this row says it cannot. That \
                     is a grammar change reaching {}: give the row a real \
                     expectation.",
                    row.word, row.code
                ));
                continue;
            }
            (Err(errors), _) => {
                let codes: Vec<&str> = errors.errors.iter().map(|e| e.code.as_str()).collect();
                wrong.push(format!(
                    "{:?}: does not parse as a word at all ({codes:?}), so the rule \
                     about {} cannot be reached this way",
                    row.word, row.code
                ));
                continue;
            }
            (Ok(word), _) => word,
        };

        let reported = diagnostics(word, language.as_ref());
        let fired = reported.iter().any(|e| e.code.as_str() == row.code);
        let got: Vec<&str> = reported.iter().map(|e| e.code.as_str()).collect();

        // A diagnostic about a word points INSIDE that word. The predecessor
        // asserted this for two rows against hardcoded bounds (`[10, 15]`,
        // `[20, 28]`) it had also written; here the bounds are the parsed
        // word's own span, and every violating row gets the check.
        for error in reported.iter().filter(|e| e.code.as_str() == row.code) {
            let span = error.location.span;
            if span.start < word.span.start || span.end > word.span.end {
                wrong.push(format!(
                    "{:?}: {}'s span {:?} lies outside the word's span {:?}",
                    row.word, row.code, span, word.span
                ));
            }
        }

        match (row.expect, fired) {
            (Expect::Violates, false) => wrong.push(format!(
                "{:?}: expected {} and got {got:?}",
                row.word, row.code
            )),
            (Expect::Legal, true) => wrong.push(format!(
                "{:?}: is a legal instance of {}'s shape and reported it anyway ({got:?})",
                row.word, row.code
            )),
            (Expect::Violates, true) | (Expect::Legal, false) => {}
            // Handled above, before the word exists.
            (Expect::DoesNotParse, _) => {}
        }
    }

    assert!(
        wrong.is_empty(),
        "{} of {} word rules did not hold over a PARSED word:\n  {}",
        wrong.len(),
        ROWS.len(),
        wrong.join("\n  ")
    );
    Ok(())
}

/// E241's MESSAGE names the canonical spelling of the marker that was
/// mistyped, over a word the parser built.
///
/// SURVIVES a type change, and says which category: this is about rendered
/// diagnostic TEXT, which no type pins.
///
/// What it deliberately does NOT own is the vocabulary. Which spellings E241
/// rejects is settled end to end in
/// `crates/chatter/tests/integration/untranscribed_marker_spelling_tests.rs`,
/// at the CLI boundary; a per-token list here would be one more hand-written
/// copy of a vocabulary whose scattering is the reason `ww` went unrejected
/// for years. Two pairs, on different letters, one shortened and one miscased,
/// is what the message claim needs.
#[test]
fn the_e241_message_names_the_marker_that_was_meant() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let mut wrong = Vec::new();

    for (mistyped, canonical) in [("xx", "xxx"), ("WWW", "www")] {
        let word = match parser.parse_word(mistyped) {
            Ok(word) => word,
            Err(errors) => {
                let codes: Vec<&str> = errors.errors.iter().map(|e| e.code.as_str()).collect();
                wrong.push(format!(
                    "{mistyped:?}: does not parse as a word ({codes:?})"
                ));
                continue;
            }
        };
        let named = diagnostics(&word, None)
            .iter()
            .any(|e| e.code.as_str() == "E241" && e.message.contains(canonical));
        if !named {
            wrong.push(format!(
                "{mistyped:?}: expected an E241 whose message names {canonical:?}"
            ));
        }
    }

    assert!(wrong.is_empty(), "{}", wrong.join("\n  "));
    Ok(())
}

/// Every `CLEAN` word validates with no diagnostic at all.
#[test]
fn every_clean_word_validates_clean() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let mut wrong = Vec::new();
    for word in CLEAN {
        match parser.parse_word(word) {
            Ok(parsed) => {
                let got: Vec<String> = diagnostics(&parsed, None)
                    .iter()
                    .map(|e| format!("{} {}", e.code.as_str(), e.message))
                    .collect();
                if !got.is_empty() {
                    wrong.push(format!("{word:?}: {got:?}"));
                }
            }
            Err(errors) => {
                let codes: Vec<&str> = errors.errors.iter().map(|e| e.code.as_str()).collect();
                wrong.push(format!("{word:?}: does not parse as a word ({codes:?})"));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n  "));
    Ok(())
}

/// The compound-marker messages say which end of the word is missing.
///
/// SURVIVES a type change, and says which category: rendered diagnostic
/// TEXT, which no type pins. E233 is reached through the canonical parser;
/// E232 (`+word`) is built by no tree-sitter parse (see the E232 rows) and is
/// pinned here through the independent backend, over a whole file, which is
/// the one route from CHAT text to that message.
#[test]
fn compound_marker_messages_name_the_fault() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let word = parser.parse_word("word+").map_err(|errors| {
        TestError::Failure(format!("\"word+\" does not parse as a word: {errors:?}"))
    })?;
    let trailing = diagnostics(&word, None)
        .into_iter()
        .find(|e| e.code.as_str() == "E233")
        .ok_or_else(|| TestError::Failure("\"word+\" reported no E233".into()))?;
    assert_eq!(
        trailing.message,
        "Compound marker '+' cannot have an empty trailing part"
    );

    let source = SingleSpeaker::english("*CHI:\t+word .").source();
    let errors = ErrorCollector::new();
    let ParseOutcome::Parsed(mut file) = Re2cParser::new().parse_chat_file(&source, 0, &errors)
    else {
        return Err(TestError::Failure(
            "the independent backend rejected the whole file around \"+word\"".into(),
        ));
    };
    file.validate_with_alignment(&errors, TranscriptName::Anonymous);
    let leading = errors
        .into_vec()
        .into_iter()
        .find(|e| e.code.as_str() == "E232")
        .ok_or_else(|| {
            TestError::Failure("the independent backend reported no E232 for \"+word\"".into())
        })?;
    assert_eq!(leading.message, "Compound marker '+' cannot start a word");
    Ok(())
}
