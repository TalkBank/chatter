// Test code: the panic-family clippy lints are relaxed by policy.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! `DependentTierEntry::content_span` must agree with the source it describes.
//!
//! The span is DERIVED from the label length and the separator rather than
//! recorded by the parser, so it is only as good as that arithmetic. A derived
//! value that quietly disagrees with the bytes is worse than no accessor at
//! all, which is what these tests exist to prevent.

use talkbank_model::model::Line;
use talkbank_parser::{ParseProduct, TreeSitterParser};

const HEAD: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI||female|||Target_Child|||\n";

/// Every dependent tier's `content_span` slices to exactly its content text.
fn assert_content_spans(source: &str, expected: &[(&str, &str)]) {
    let parser = TreeSitterParser::new().expect("parser");
    let ParseProduct::Built { file, .. } = parser.parse_chat_file(source) else {
        panic!("source must parse: {source:?}");
    };
    let mut seen: Vec<(String, String)> = Vec::new();
    for line in file.lines.as_slice() {
        if let Line::Utterance(utterance) = line {
            for entry in &utterance.dependent_tiers {
                let span = entry.content_span().expect("a real span");
                let text = source
                    .get(span.start as usize..span.end as usize)
                    .expect("the span must be in bounds and on char boundaries");
                seen.push((entry.kind().to_owned(), text.to_owned()));
            }
        }
    }
    let expected: Vec<(String, String)> = expected
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    assert_eq!(seen, expected);
}

/// Surviving category: behaviour a signature cannot describe. `-> Option<Span>`
/// cannot say the span lands on the content and not on the label.
#[test]
fn content_span_excludes_the_label_and_the_newline() {
    assert_content_spans(
        &format!("{HEAD}*CHI:\thello .\n%mor:\tco|hello .\n@End\n"),
        &[("mor", "co|hello .")],
    );
}

/// A label of a different length, so the arithmetic cannot be coincidence.
#[test]
fn content_span_is_right_for_labels_of_other_lengths() {
    assert_content_spans(
        &format!("{HEAD}*CHI:\thello .\n%com:\ta remark\n%xfoo:\tcustom text\n@End\n"),
        &[("com", "a remark"), ("xfoo", "custom text")],
    );
}

/// The separator carries illegal trailing whitespace, so the content starts
/// after it rather than one byte past the colon.
#[test]
fn content_span_starts_after_an_illegal_trailing_space() {
    assert_content_spans(
        &format!("{HEAD}*CHI:\thello .\n%com:\t  spaced remark\n@End\n"),
        &[("com", "spaced remark")],
    );
}

/// The last tier before `@End`, in case the whole-line span ends differently.
#[test]
fn content_span_is_right_for_the_final_tier_of_a_file() {
    assert_content_spans(
        &format!("{HEAD}*CHI:\thi .\n%gra:\t1|0|INCROOT\n@End\n"),
        &[("gra", "1|0|INCROOT")],
    );
}
