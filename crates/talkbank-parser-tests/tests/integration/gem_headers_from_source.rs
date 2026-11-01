//! What a gem header lowers to, well-formed and not, through the parsed
//! verdict.
//!
//! All gem kinds carry an optional `seq(header_sep, free_text)`.
//! A label is the text of the `free_text`
//! pieces joined, a continuation line contributing one space. Until
//! 2026-09-09 a header whose typed read found no label fell back to slicing
//! the header's raw text after its first colon; these rows pin that the
//! typed position is the only read: a header with nothing after its tab is
//! the one empty-content state (E553), not a gem with no label and not the
//! bare marker nobody wrote, so it opens and closes no gem scope. A
//! colon-bearing `@G:` with no tab is not the legal bare `@G`: the
//! whole-tree pass rejects the malformed suffix (E316) and the line is kept
//! as an unknown header (E525), the same under both parsers.

use talkbank_model::model::{
    GemKind, GemLabel, Header, HeaderKind, Line, PlainKind, WrittenSeparator,
};
use talkbank_parser_tests::from_source::{Rules, parsed_document};
use talkbank_parser_tests::test_error::TestError;

/// The gem header the fixture carries between two utterances, if `line`
/// lowered as one.
fn gem_header(line: &str, codes: &[&str]) -> Result<Option<Header>, TestError> {
    let source = format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
         @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello .\n{line}\n\
         *CHI:\tbye .\n@End\n"
    );
    let file = parsed_document(&source, codes, Rules::Default)?;
    Ok(file
        .lines
        .into_iter()
        .filter_map(|entry| match entry {
            Line::Header { header, .. } => Some(*header),
            _ => None,
        })
        .find(|header| matches!(header.kind(), HeaderKind::Plain(PlainKind::Gem(_)))))
}

/// The empty colon form of a gem header.
fn empty(kind: GemKind) -> Header {
    PlainKind::Gem(kind).empty(WrittenSeparator::admit(":\t").expect("written separator"))
}

fn label(text: &str) -> Option<GemLabel> {
    Some(GemLabel::try_new(text).expect("a nonempty label"))
}

#[test]
fn paired_gems_retain_source_locations_and_unlocated_wire_state() -> Result<(), TestError> {
    use talkbank_model::Location;
    use talkbank_model::model::{ChatFile, GemEvent, GemState};

    const SOURCE: &str =
        include_str!("../../../../corpus/reference/edge-cases/multiline-paired-gems.cha");
    let file = parsed_document(SOURCE, &[], Rules::Default)?;
    let wire = serde_json::to_string(&file).expect("AST wire");
    let decoded: ChatFile = serde_json::from_str(&wire).expect("AST wire readback");
    enum ExpectedLocations {
        Source,
        Unlocated,
    }
    for (source_file, expected) in [
        (&file, ExpectedLocations::Source),
        (&decoded, ExpectedLocations::Unlocated),
    ] {
        let mut state = GemState::new();
        let mut pairs = Vec::new();
        for (header, location) in source_file.headers_with_locations() {
            match state.observe(header, location) {
                None | Some(GemEvent::Opened) => {}
                Some(GemEvent::Closed(pair)) => pairs.push(pair),
                event => panic!("clean reference has an unexpected gem event: {event:?}"),
            }
        }
        assert_eq!(state.finish().count(), 0);
        assert_eq!(pairs.len(), 1);
        let pair = &pairs[0];
        assert_eq!(
            pair.label().map(|label| label.as_str()),
            Some("morning café play")
        );
        match expected {
            ExpectedLocations::Source => {
                assert_eq!(
                    pair.begin().span().expect("parsed begin").start(),
                    SOURCE.find("@Bg:").unwrap()
                );
                assert_eq!(
                    pair.end().span().expect("parsed end").start(),
                    SOURCE.find("@Eg:").unwrap()
                );
            }
            ExpectedLocations::Unlocated => {
                assert_eq!(pair.begin(), Location::Unlocated);
                assert_eq!(pair.end(), Location::Unlocated);
            }
        }
    }
    Ok(())
}

/// A gem label is the typed `free_text` read and nothing else: a header
/// with no label text has `None`, never a label sliced from raw text.
#[test]
fn a_gem_label_is_read_only_from_its_free_text_position() -> Result<(), TestError> {
    let rows: [(&str, &[&str], Option<Header>); 6] = [
        (
            "@G:\tlabel one",
            &[],
            Some(Header::LazyGem {
                label: label("label one"),
            }),
        ),
        (
            "@G:\tlabel\n\tmore",
            &[],
            Some(Header::LazyGem {
                label: label("label more"),
            }),
        ),
        ("@G:\t", &["E553"], Some(empty(GemKind::Lazy))),
        // A colon with no tab is an ERROR in the group position: unparsable
        // text (E316), kept as an unknown header (E525), never a gem header
        // (in particular, never the bare marker nobody wrote).
        ("@G:", &["E316", "E525"], None),
        ("@Bg:\t", &["E553"], Some(empty(GemKind::Begin))),
        ("@Eg:\t", &["E553"], Some(empty(GemKind::End))),
    ];
    let mut wrong = Vec::new();
    for (line, codes, expected) in rows {
        match gem_header(line, codes) {
            Ok(header) if header == expected => {}
            Ok(header) => wrong.push(format!(
                "{line:?} lowered as {header:?}, expected {expected:?}"
            )),
            Err(err) => wrong.push(format!("{line:?}: {err}")),
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    Ok(())
}
