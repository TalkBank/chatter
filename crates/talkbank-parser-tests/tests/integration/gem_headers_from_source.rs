//! What a gem header lowers to, well-formed and not, through the parsed
//! verdict.
//!
//! `@G` requires a `free_text` label; `@Bg` and `@Eg` carry an optional
//! `seq(header_sep, free_text)`. A label is the text of the `free_text`
//! pieces joined, a continuation line contributing one space. Until
//! 2026-09-09 a header whose typed read found no label fell back to slicing
//! the header's raw text after its first colon; these rows pin that the
//! typed position is the only read: a header with nothing after its tab
//! has no label and its recovery is named by the whole-tree pass (E342 for
//! the MISSING required label of `@G`; E316 where `@Bg`/`@Eg` lost their
//! optional group); a bare `@G:` with no tab is E303 and no gem header at
//! all. The codes are measured through `chatter validate`, and the lone
//! `@Bg`/`@Eg` rows also carry their unmatched-gem verdicts (E526/E527).

use talkbank_model::model::{GemLabel, Header, Line};
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
        .find(|header| {
            matches!(
                header,
                Header::BeginGem { .. } | Header::EndGem { .. } | Header::LazyGem { .. }
            )
        }))
}

fn label(text: &str) -> Option<GemLabel> {
    Some(GemLabel::new(text))
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
        ("@G:\t", &["E342"], Some(Header::LazyGem { label: None })),
        ("@G:", &["E303"], None),
        (
            "@Bg:\t",
            &["E316", "E526"],
            Some(Header::BeginGem { label: None }),
        ),
        (
            "@Eg:\t",
            &["E316", "E527"],
            Some(Header::EndGem { label: None }),
        ),
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
