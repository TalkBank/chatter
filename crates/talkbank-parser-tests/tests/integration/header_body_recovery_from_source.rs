//! A header whose body holds a recovery node lowers the same way under both
//! parsers: one unknown header at the line, with the line's text and the
//! same reason, for every header family.
//!
//! Tree-sitter can parse part of a body and put the rest in an ERROR beside
//! it (`@Situation:` and a tab, then `at`, a NUL and `home`, parses `at` as
//! the body). Lowering that body would build a header from the part before
//! the ERROR and silently drop the rest, while re2c, whose lexer marks the
//! same bytes malformed, keeps the line. Both backends now keep the line as
//! `Header::Unknown`, decided for tree-sitter in one place (the body reader's
//! recovery check) for every family. The spec examples in
//! `E316_unparsable_header_body.md` measure the same lines' diagnostics.

use talkbank_model::model::Header;
use talkbank_parser_tests::test_error::TestError;

/// Where a probed line sits in the fixture: the pre-`@Begin` headers
/// belong before `@Begin`, every other header between two utterances.
#[derive(Clone, Copy)]
enum Placement {
    BeforeBegin,
    InBody,
}

/// The line terminator every line of a fixture ends with.
#[derive(Clone, Copy)]
enum Terminator {
    Lf,
    CrLf,
}

/// The fixture carrying `line` at its placement, every line ending with
/// `terminator`.
fn fixture(line: &str, placement: Placement, terminator: Terminator) -> String {
    let source = match placement {
        Placement::BeforeBegin => format!(
            "@UTF8\n{line}\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
             @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello .\n@End\n"
        ),
        Placement::InBody => format!(
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
             @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello .\n{line}\n\
             *CHI:\tbye .\n@End\n"
        ),
    };
    match terminator {
        Terminator::Lf => source,
        Terminator::CrLf => source.replace('\n', "\r\n"),
    }
}

/// The header each parser lowers at the line starting at `at`, if any.
fn header_at<P: talkbank_model::ChatParser>(parser: &P, source: &str, at: usize) -> Option<Header> {
    let errors = talkbank_model::ErrorCollector::new();
    let file = parser.parse_chat_file(source, 0, &errors).into_option()?;
    file.headers_with_locations()
        .find(|(_, location)| location.span().is_some_and(|span| span.start() == at))
        .map(|(header, _)| header.clone())
}

/// Every line, under both parsers and with either line terminator, is one
/// unknown header whose text is the line exactly as written (no line
/// terminator under either) and whose
/// reason is the same: never a header of its kind built from part of the
/// line, the empty state, a gem's bare marker, or no header at all.
#[test]
fn malformed_header_bodies_keep_one_unknown_header_under_both_parsers() -> Result<(), TestError> {
    use Placement::{BeforeBegin, InBody};
    let tree_sitter = talkbank_parser::TreeSitterParser::new()
        .map_err(|err| TestError::ParserInit(err.to_string()))?;
    let re2c = talkbank_parser_re2c::Re2cParser::new();
    let rows = [
        // An ERROR where a gem's group or label belongs, and a body of only
        // what its grammar refuses.
        ("@G:", InBody),
        ("@G: story", InBody),
        ("@G:\t\u{0}", InBody),
        ("@Page:\t\t", InBody),
        // A recovery node displaced beside a body that holds the part of the
        // content before it, once per header family. The NUL is a control
        // character no header body admits.
        ("@G:\tsto\u{0}ry", InBody),
        ("@Bg:\tsto\u{0}ry", InBody),
        ("@Eg:\tstory\u{0}", InBody),
        // After a gem's bare marker, and in a header the model does not carry.
        ("@Bg\u{0}", InBody),
        ("@Foo:\tba\u{0}r", InBody),
        ("@Date:\t01-JAN\u{0}-2024", InBody),
        ("@Situation:\tat\u{0}home", InBody),
        ("@Situation:\tat home\u{0}", InBody),
        ("@Situation:\tat home\n\tmo\u{0}re", InBody),
        ("@Time Duration:\t10:00\u{0}-11:00", InBody),
        ("@Time Start:\t00:00\u{0}:00", InBody),
        ("@Location:\tPitts\u{0}burgh", InBody),
        ("@Room Layout:\tkit\u{0}chen", InBody),
        ("@Transcriber:\tJ\u{0}D", InBody),
        ("@Warning:\tnoisy\u{0}recording", InBody),
        ("@Activities:\tfree\u{0}play", InBody),
        ("@Bck:\tback\u{0}ground", InBody),
        ("@Videos:\tsam\u{0}ple", InBody),
        ("@T:\tto\u{0}pic", InBody),
        ("@Page:\t1\u{0}2", InBody),
        ("@Page:\t1x", InBody),
        ("@Transcription:\tfu\u{0}ll", InBody),
        ("@Comment:\ta\u{0}comment", InBody),
        ("@Options:\tCA\u{0}", InBody),
        ("@Birth of CHI:\t28-JUN\u{0}-2001", InBody),
        ("@Birthplace of CHI:\tPitts\u{0}burgh", InBody),
        ("@L1 of CHI:\teng\u{0}", InBody),
        // A recovery node inside a body with its own grammar, and beside it.
        ("@Media:\tsam\u{0}ple, audio", InBody),
        ("@Media:\tsample, audio\u{0}", InBody),
        ("@Types:\tlong, toy\u{0}play, TD", InBody),
        ("@Types:\tlong, toy play, TD", InBody),
        // The pre-`@Begin` family. (`@Window` is read by the same reader; a
        // NUL in its geometry happens to make tree-sitter abandon the whole
        // pre-`@Begin` region to a document-level ERROR, so no header node
        // exists to lower: a grammar recovery difference, not partial
        // content.)
        ("@PID:\t11312/t-0000\u{0}0000-1", BeforeBegin),
        ("@Color words:\tred\u{0} 255 0 0", BeforeBegin),
        ("@Font:\tWin95:Cou\u{0}rier:-13:0", BeforeBegin),
    ];
    let mut wrong = Vec::new();
    for ((line, placement), terminator) in rows
        .into_iter()
        .flat_map(|row| [(row, Terminator::Lf), (row, Terminator::CrLf)])
    {
        // A continuation's line break takes the fixture's terminator too.
        let written = match terminator {
            Terminator::Lf => line.to_owned(),
            Terminator::CrLf => line.replace('\n', "\r\n"),
        };
        let source = fixture(line, placement, terminator);
        let line = written.as_str();
        let at = source.find(line).expect("the line");
        let (primary, native) = (
            header_at(&tree_sitter, &source, at),
            header_at(&re2c, &source, at),
        );
        match (&primary, &native) {
            (
                Some(Header::Unknown {
                    text,
                    parse_reason,
                    suggested_fix: None,
                }),
                Some(Header::Unknown {
                    text: native_text,
                    parse_reason: native_reason,
                    suggested_fix: None,
                }),
            ) if text.as_str() == line
                && native_text.as_str() == line
                && parse_reason == native_reason => {}
            _ => wrong.push(format!(
                "{line:?}: tree-sitter {primary:?}, re2c {native:?}"
            )),
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    Ok(())
}
