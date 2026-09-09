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
//! Word-language resolution over words the PARSER built, not words a test did.
//!
//! # What this replaces
//!
//! `talkbank-model`'s `validation/word/language/tests.rs` resolved
//! hand-built words: `Word::new_unchecked("ni3@s", "ni3")` with
//! `word.lang = Some(WordLanguageMarker::Shortcut)` set beside it, and a
//! tier language and a declared list each spelled as a `Vec` in the test.
//! Nothing held the raw text, the marker, the tier and the header to the
//! same document: the input stated four times. Here each row is one CHAT
//! document (an `@Languages` value and a main tier), the parser builds the
//! words and their markers, the tier language is the precode the parser
//! read or the document's first language, exactly as
//! `MainTier::whole_utterance_language_switch_target` derives it, and the
//! declared list is the file's own `languages`.
//!
//! # What the move found that the originals could not
//!
//! A parsed document runs every rule, so the rows report what the inline
//! tests never saw beside the resolution they pinned: an utterance whose
//! every word is `@s`-marked is E255 (write it `[- LANG]`), a `[- eng]`
//! precode naming an undeclared language is E755, and `ni3` under `[- eng]`
//! is E220 (numeric, not a word of English). Each is a true statement about
//! that document, measured through `chatter validate` before the row was
//! written; the tier-scoped case has two rows, one with English words and
//! one with the original Chinese digit-words plus their E220, so the
//! property the original pinned (resolution follows the tier, whatever the
//! word's content) is still stated.
//!
//! What the table does NOT assert, and the inline tests did: how many times
//! a code fired. The document verdict is a code SET; an inline test resolving
//! one word could count its diagnostics. No row here has two words that
//! would report the same code, so nothing is lost today, and a row that
//! needs the count should assert it through `diagnostics_of`.
//!
//! # One table
//!
//! Each row is a document, the code set the whole document must report,
//! and the resolution of every word of its main tier in order. The
//! resolution is read through `GoverningMark::of(word, None).resolve`, the
//! same call the switch-target proof and the transform crate make; `None`
//! is honest because no row has a `<...> [@s]` span, and the scoped form is
//! the one to reach for when a row does.

use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::model::{ChatFile, LanguageCode, Line};
use talkbank_model::validation::{GoverningMark, LanguageResolution};
use talkbank_parser_tests::from_source::{Rules, SingleSpeaker, parsed_document};
use talkbank_parser_tests::test_error::TestError;

/// What a word of the row's main tier must resolve to.
#[derive(Debug, Clone, Copy)]
enum Expect {
    /// `LanguageResolution::Single` of this code.
    Single(&'static str),
    /// `LanguageResolution::Unresolved`.
    Unresolved,
}

/// A document and the resolution of each of its main-tier words, in order.
struct Row {
    /// Why the row exists.
    why: &'static str,
    /// The `@Languages` value, or none for a document without the header.
    languages: Option<&'static str>,
    /// The main tier.
    line: &'static str,
    /// Every code the document reports under the default rules.
    codes: &'static [&'static str],
    /// The words in main-tier order: raw text, resolution.
    words: &'static [(&'static str, Expect)],
}

const ROWS: &[Row] = &[
    Row {
        why: "unmarked words take the document's first language",
        languages: Some("zho, eng"),
        line: "*CHI:\tni3 hao3 .",
        codes: &[],
        words: &[
            ("ni3", Expect::Single("zho")),
            ("hao3", Expect::Single("zho")),
        ],
    },
    Row {
        why: "a `[- eng]` precode is the tier language every unmarked word takes",
        languages: Some("zho, eng"),
        line: "*CHI:\t[- eng] hello there .",
        codes: &[],
        words: &[
            ("hello", Expect::Single("eng")),
            ("there", Expect::Single("eng")),
        ],
    },
    Row {
        why: "resolution follows the tier, not the word's content: Chinese digit-words under `[- eng]` resolve to eng, and E220 is what the document then says about them",
        languages: Some("zho, eng"),
        line: "*CHI:\t[- eng] ni3 hao3 .",
        codes: &["E220"],
        words: &[
            ("ni3", Expect::Single("eng")),
            ("hao3", Expect::Single("eng")),
        ],
    },
    Row {
        why: "a shortcut under a tier language the header does not declare is unresolved, never the tier language handed back (the dona@s bug)",
        languages: Some("cat, spa"),
        line: "*CHI:\t[- eng] dona@s .",
        codes: &["E249", "E755"],
        words: &[("dona@s", Expect::Unresolved)],
    },
    Row {
        why: "a shortcut on the primary tier of a two-language document is the secondary language",
        languages: Some("cat, spa"),
        line: "*CHI:\tdona@s .",
        codes: &["E255"],
        words: &[("dona@s", Expect::Single("spa"))],
    },
    Row {
        why: "a shortcut under the secondary tier language is the other declared language",
        languages: Some("zho, eng"),
        line: "*CHI:\t[- eng] ni3@s hao3@s .",
        codes: &["E255"],
        words: &[
            ("ni3@s", Expect::Single("zho")),
            ("hao3@s", Expect::Single("zho")),
        ],
    },
    Row {
        why: "an explicit code overrides the tier language",
        languages: Some("zho, eng"),
        line: "*CHI:\t[- eng] ni3@s:zho hao3@s:zho .",
        codes: &["E255"],
        words: &[
            ("ni3@s:zho", Expect::Single("zho")),
            ("hao3@s:zho", Expect::Single("zho")),
        ],
    },
    Row {
        why: "a shortcut with no secondary language declared is unresolved, E249",
        languages: Some("zho"),
        line: "*CHI:\tni3@s .",
        codes: &["E249"],
        words: &[("ni3@s", Expect::Unresolved)],
    },
    Row {
        why: "a shortcut under a tertiary tier language is unresolved, E248",
        languages: Some("zho, eng, spa"),
        line: "*CHI:\t[- spa] word@s .",
        codes: &["E248"],
        words: &[("word@s", Expect::Unresolved)],
    },
    Row {
        why: "an explicit code absent from `@Languages` resolves silently (maintainer ruling 2026-07-15)",
        languages: Some("zho, eng"),
        line: "*CHI:\tciao@s:ita .",
        codes: &["E255"],
        words: &[("ciao@s:ita", Expect::Single("ita"))],
    },
    Row {
        why: "an explicit code declared in `@Languages` resolves to itself",
        languages: Some("zho, eng"),
        line: "*CHI:\thello@s:eng .",
        codes: &["E255"],
        words: &[("hello@s:eng", Expect::Single("eng"))],
    },
    Row {
        why: "an explicit code resolves even with no `@Languages` header (the header's absence is the file's E504)",
        languages: None,
        line: "*CHI:\thello@s:eng .",
        codes: &["E255", "E504"],
        words: &[("hello@s:eng", Expect::Single("eng"))],
    },
    Row {
        why: "a shortcut with no language context at all is unresolved, E249",
        languages: None,
        line: "*CHI:\tword@s .",
        codes: &["E249", "E504"],
        words: &[("word@s", Expect::Unresolved)],
    },
    Row {
        why: "an unmarked word with no language context is unresolved, and that alone is no word diagnostic",
        languages: None,
        line: "*CHI:\tword .",
        codes: &["E504"],
        words: &[("word", Expect::Unresolved)],
    },
];

/// The row's document, parsed after its code set was verified.
fn parsed(row: &Row) -> Result<ChatFile, TestError> {
    match row.languages {
        Some(languages) => SingleSpeaker {
            languages,
            options: None,
            media: talkbank_parser_tests::from_source::Media::Undeclared,
            lines: row.line,
        }
        .parsed(row.codes),
        None => parsed_document(
            &format!(
                "@UTF8\n@Begin\n@Participants:\tCHI Target_Child\n\
                 @ID:\teng|corpus|CHI|||||Target_Child|||\n{}\n@End\n",
                row.line
            ),
            row.codes,
            Rules::Default,
        ),
    }
}

#[test]
fn every_main_tier_word_resolves_as_the_row_says() -> Result<(), TestError> {
    for row in ROWS {
        let file = parsed(row)?;
        let declared: Vec<LanguageCode> = file.languages.iter().cloned().collect();
        let utterance = file
            .lines
            .iter()
            .find_map(|line| match line {
                Line::Utterance(utterance) => Some(utterance.as_ref()),
                _ => None,
            })
            .ok_or_else(|| TestError::Failure(format!("{}: no utterance", row.why)))?;
        // The tier language as the switch-target proof derives it: the
        // precode the parser read, else the document's first language.
        let tier_language = utterance
            .main
            .content
            .language_code
            .as_ref()
            .or_else(|| declared.first());
        let mut seen: Vec<(String, LanguageResolution)> = Vec::new();
        walk_words(&utterance.main.content.content, None, &mut |item| {
            let word = match item {
                WordItem::Word(word) => word,
                WordItem::ReplacedWord(replaced) => &replaced.word,
                WordItem::Separator(_) => return,
            };
            let outcome = GoverningMark::of(word, None).resolve(tier_language, &declared);
            seen.push((word.raw_text().to_string(), outcome.resolution));
        });
        let want: Vec<(String, LanguageResolution)> = row
            .words
            .iter()
            .map(|(raw, expect)| {
                let resolution = match expect {
                    Expect::Single(code) => LanguageResolution::Single(
                        LanguageCode::new(*code).expect("row codes are non-empty"),
                    ),
                    Expect::Unresolved => LanguageResolution::Unresolved,
                };
                ((*raw).to_string(), resolution)
            })
            .collect();
        if seen != want {
            return Err(TestError::Failure(format!(
                "{}: {:?} resolved as {seen:?}, the row says {want:?}",
                row.why, row.line
            )));
        }
    }
    Ok(())
}
