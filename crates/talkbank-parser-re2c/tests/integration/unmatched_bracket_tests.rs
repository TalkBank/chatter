//! A `[` opened inside a replacement must not be swallowed.
//!
//! The lexer's replacement rule took any content up to the first `]`, so
//! `[: unclosed replacement [* error]` lexed as a COMPLETE replacement whose
//! text happened to contain `[* error`. That is not what the grammar says: a
//! replacement's content is `standalone_word`s, and `[` cannot appear in a
//! word.
//!
//! Two consequences, both grounded against real CLAN CHECK (2026-08-07 bundle)
//! in `docs/audits/2026-08-11-utterance-initial-annotation-adjudication.md`:
//!
//! - on `[: unclosed replacement [* error] .`, CHECK reports "Unmatched `[`
//!   found on the tier" and the tree-sitter parser reports an unclosed
//!   replacement bracket, while re2c reported that the annotation lacked
//!   preceding text: a plausible-sounding wrong reason;
//! - on `word [: a [* b] .`, which CHECK rejects, re2c was SILENT.
//!
//! Silence on invalid input is the worst outcome for an oracle whose job is to
//! disagree when the canonical parser is wrong.

use talkbank_model::ChatParser;
use talkbank_model::ErrorCollector;
use talkbank_parser_re2c::Re2cParser;

/// Parse a one-utterance document and return the diagnostic codes.
fn codes_for(main_tier_line: &str) -> Vec<String> {
    let source = format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
         @ID:\teng|corpus|CHI|||||Target_Child|||\n{main_tier_line}\n@End\n"
    );
    let parser = Re2cParser::new();
    let errors = ErrorCollector::new();
    if let talkbank_model::ParseOutcome::Parsed(mut chat_file) =
        parser.parse_chat_file(&source, 0, &errors)
    {
        chat_file
            .validate_with_alignment(&errors, talkbank_model::model::TranscriptName::Anonymous);
    }
    errors
        .into_vec()
        .iter()
        .map(|error| error.code.as_str().to_owned())
        .collect()
}

/// A `[` inside a replacement makes the construct invalid, and re2c must say so.
#[test]
fn a_bracket_inside_a_replacement_is_not_silently_swallowed() {
    let codes = codes_for("*CHI:\tword [: a [* b] .");
    assert!(
        !codes.is_empty(),
        "re2c was SILENT on input real CLAN CHECK rejects. An oracle that \
         accepts what the canonical parser rejects cannot do its job."
    );
}

/// The utterance-initial case must not be diagnosed as "no preceding text",
/// which is true of a DIFFERENT construct from the one that was written.
#[test]
fn an_unclosed_replacement_is_not_reported_as_a_missing_antecedent() {
    let codes = codes_for("*CHI:\t[: unclosed replacement [* error] .");
    assert!(
        !codes.is_empty(),
        "the construct is invalid and must be reported; got nothing"
    );
    assert!(
        !codes.iter().any(|code| code == "E759"),
        "E759 says the annotation has nothing to scope over, which describes a \
         well-formed `[: x]` in the wrong place. Here the bracket is never \
         closed, which CHECK reports as an unmatched `[`. Got: {codes:?}"
    );
}
