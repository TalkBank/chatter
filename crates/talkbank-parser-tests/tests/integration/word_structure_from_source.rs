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
//! What a parsed word, a parsed main tier and a parsed utterance carry: the
//! three claims of `talkbank-model`'s `model/mod.rs` inline tests, over what
//! the parser builds from the text instead of over hand-built values.
//!
//! # What this replaces
//!
//! `test_word_structure` built `Word::new_unchecked("hel(lo)@b", "hello")`
//! and then set the content to `[Text("hel"), Shortening("lo")]` by hand:
//! the raw text, the cleaned text and the content list stated separately,
//! with nothing forcing them to describe one word. `test_json_serialization`
//! serialized a hand-built main tier. `test_simple_utterance_model` built a
//! main tier from `Word::new_unchecked` values with `Span::DUMMY` positions
//! and asserted the speaker, the word count and the absence of a `%mor`
//! tier on the utterance `Utterance::new` wrapped around it. Each claim is
//! made here once, over a parsed fixture.
//!
//! `word_type.rs`'s `cleaned_text_tests` (four tests, moved here on
//! 2026-09-09) built `Word::new_unchecked("↫sch↫schaap", "ignored")` and
//! then set the pieces by hand: a segment-repetition delimiter, a text, the
//! delimiter again, a text. The spelling and the pieces were two statements
//! of one word with nothing holding them together, and the cleaned text the
//! constructor was handed was literally "ignored". Here each spelling is
//! parsed and the cleaned text is what the word computes from what the
//! parser lexed.

use talkbank_model::content::word::{FormType, WordContent};
use talkbank_model::model::OverlapPointKind;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::from_source::SingleSpeaker;
use talkbank_parser_tests::test_error::TestError;

/// A shortening inside a form-marked word: the raw text is the source, the
/// cleaned text is the shortening expanded, the content is the two pieces the
/// parser lexed, and the form marker is typed.
#[test]
fn a_shortened_form_marked_word_carries_its_pieces() -> Result<(), TestError> {
    let parser = TreeSitterParser::new()?;
    let word = parser
        .parse_word("hel(lo)@b")
        .map_err(|err| TestError::Failure(format!("hel(lo)@b did not parse: {err:?}")))?;

    assert_eq!(word.raw_text(), "hel(lo)@b");
    assert_eq!(word.cleaned_text(), "hello");
    assert_eq!(word.form_type, Some(FormType::B));
    let pieces: Vec<&str> = word
        .content()
        .iter()
        .map(|item| match item {
            WordContent::Text(text) => text.as_ref(),
            WordContent::Shortening(shortening) => shortening.as_ref(),
            piece @ (WordContent::Phonetic(_)
            | WordContent::OverlapPoint(_)
            | WordContent::CAElement(_)
            | WordContent::CADelimiter(_)
            | WordContent::StressMarker(_)
            | WordContent::Lengthening(_)
            | WordContent::SyllablePause(_)
            | WordContent::UnderlineBegin(_)
            | WordContent::UnderlineEnd(_)
            | WordContent::CompoundMarker(_)
            | WordContent::CliticBoundary(_)) => {
                panic!("hel(lo)@b lexed an unexpected piece: {piece:?}")
            }
        })
        .collect();
    assert_eq!(pieces, ["hel", "lo"]);
    Ok(())
}

/// Serde wiring on the main tier: a wire-format smoke test, which no type
/// pins, over the tier the parser built.
#[test]
fn a_parsed_main_tier_serializes_its_speaker_and_words() -> Result<(), TestError> {
    let main = SingleSpeaker::english("*CHI:\thello .").main_tier(&[])?;
    let json = serde_json::to_string_pretty(&main)
        .map_err(|err| TestError::Failure(format!("serializing the main tier: {err}")))?;
    assert!(json.contains("CHI"), "speaker missing from {json}");
    assert!(json.contains("hello"), "word missing from {json}");
    Ok(())
}

/// An utterance with only a main tier: the speaker and the word count are
/// the parsed line's, and no `%mor` tier is present until one is written.
#[test]
fn an_utterance_with_only_a_main_tier_has_its_speaker_and_no_mor() -> Result<(), TestError> {
    let utterance = SingleSpeaker::english("*CHI:\thello .").utterance(&[])?;
    assert_eq!(utterance.main.speaker.as_str(), "CHI");
    assert_eq!(utterance.main.content.content.len(), 1);
    assert!(
        utterance.mor_tier().is_none(),
        "no %mor line was written, yet a %mor tier is present"
    );
    Ok(())
}

/// The cleaned text of words carrying CA delimiters: a `↫` pair brackets a
/// REPEATED segment, which is not part of the lexical word (CHAT manual,
/// Disfluency Transcription: "the ↫ brackets the repetition"), whether the
/// repetition is an initial onset, an iterated onset or a final segment; a
/// non-repetition delimiter such as `∆` (faster speech) wraps material that
/// was spoken, which stays.
#[test]
fn a_ca_delimited_word_cleans_to_its_spoken_text() -> Result<(), TestError> {
    let parser = TreeSitterParser::new()?;
    let rows = [
        ("\u{21ab}sch\u{21ab}schaap", "schaap"),
        ("\u{21ab}b-b-b\u{21ab}boy", "boy"),
        ("like\u{21ab}ike-ike\u{21ab}", "like"),
        ("\u{2206}fast\u{2206}", "fast"),
    ];
    let mut wrong = Vec::new();
    for (spelling, cleaned) in rows {
        let word = match parser.parse_word(spelling) {
            Ok(word) => word,
            Err(err) => {
                wrong.push(format!("{spelling}: did not parse: {err:?}"));
                continue;
            }
        };
        if word.raw_text() != spelling {
            wrong.push(format!("{spelling}: raw text {:?}", word.raw_text()));
        }
        if word.cleaned_text() != cleaned {
            wrong.push(format!(
                "{spelling}: expected cleaned {cleaned:?}, got {:?} from pieces {:?}",
                word.cleaned_text(),
                word.content()
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    Ok(())
}

/// An overlap point inside a word (`butt⌈er⌉`) is word content, decoded by
/// the same function as a standalone one: the begin and the end marker, in
/// order, around the text they split.
#[test]
fn an_overlap_inside_a_word_is_word_content() -> Result<(), TestError> {
    let parser = TreeSitterParser::new()?;
    let word = parser
        .parse_word("butt\u{2308}er\u{2309}")
        .map_err(|err| TestError::Failure(format!("butt⌈er⌉ did not parse: {err:?}")))?;
    let kinds: Vec<OverlapPointKind> = word
        .content()
        .iter()
        .filter_map(|piece| match piece {
            WordContent::OverlapPoint(point) => Some(point.kind),
            _ => None,
        })
        .collect();
    assert_eq!(
        kinds,
        [
            OverlapPointKind::TopOverlapBegin,
            OverlapPointKind::TopOverlapEnd
        ],
        "pieces were {:?}",
        word.content()
    );
    assert_eq!(word.cleaned_text(), "butter");
    Ok(())
}
