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

//! Tests for `ChatFile::declared_speakers`.
//!
//! The property under test is that the `@Participants` header, not the `@ID`
//! join, decides who is in a transcript. It needs a real parse to be
//! meaningful, since the divergence between the two is created by the parser's
//! participant builder rather than by anything a hand-built model would
//! reproduce, so it lives here rather than in `talkbank-model`.
//!
//! Parse idiom (inherent `parse_chat_file`, not the trait method of the same
//! name): see the header of `utterance_containment.rs`.

use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

/// Parses a file that is deliberately INVALID, keeping the model the parser
/// built rather than discarding it along with the diagnostics.
///
/// `strict_parse` is the right helper for fixtures expected to be clean; these
/// tests are about what the model holds when the file is not, which is what
/// `expect_built` gives: the model regardless of diagnostics.
fn parse_keeping_model(source: &str) -> Result<talkbank_model::model::ChatFile, TestError> {
    Ok(TreeSitterParser::new()?
        .parse_chat_file(source)
        .expect_built())
}

/// A clean file: every declared speaker carries its `@ID` metadata, and the
/// order is the declaration order rather than any incidental map order.
#[test]
fn declared_speakers_are_enriched_and_in_declaration_order() -> Result<(), TestError> {
    let source = "@UTF8\n@Begin\n@Languages:\teng\n\
        @Participants:\tCHI Ruth Target_Child, MOT Mary Mother\n\
        @ID:\teng|corpus|CHI|10;03.||||Target_Child|||\n\
        @ID:\teng|corpus|MOT|||||Mother|||\n\
        *CHI:\thi .\n@End\n";
    let file = talkbank_parser_tests::test_error::strict_parse(
        TreeSitterParser::new()?.parse_chat_file(source),
    )?;

    let declared: Vec<_> = file.declared_speakers().collect();
    assert_eq!(declared.len(), 2, "both declared speakers must appear");

    assert_eq!(declared[0].code().as_str(), "CHI");
    assert_eq!(declared[0].name().map(|n| n.as_str()), Some("Ruth"));
    assert_eq!(declared[0].role().as_str(), "Target_Child");
    assert!(declared[0].id_metadata().is_some());
    assert_eq!(
        declared[0]
            .id_metadata()
            .and_then(|m| m.age())
            .map(|a| a.to_string()),
        Some("10;03.".to_string()),
        "the @ID header's age must reach the caller through the enrichment"
    );

    assert_eq!(declared[1].code().as_str(), "MOT");
    assert!(declared[1].id_metadata().is_some());
    Ok(())
}

/// The reported defect: a speaker declared in `@Participants` with no `@ID`
/// header is absent from the participant map, so the map under-reports the
/// roster. `declared_speakers` reports it, with `id_metadata()` `None`.
#[test]
fn a_speaker_without_an_id_header_is_still_declared() -> Result<(), TestError> {
    // MOT is declared but has no @ID. This file is INVALID (E522); the point
    // is what the model holds about it, not whether it validates.
    let source = "@UTF8\n@Begin\n@Languages:\teng\n\
        @Participants:\tCHI Ruth Target_Child, MOT Mary Mother\n\
        @ID:\teng|corpus|CHI|10;03.||||Target_Child|||\n\
        *CHI:\thi .\n@End\n";
    let file = parse_keeping_model(source)?;

    assert_eq!(
        file.all_participants().len(),
        1,
        "precondition: the @ID join drops MOT, which is the defect being covered"
    );

    let declared: Vec<_> = file.declared_speakers().collect();
    assert_eq!(
        declared.len(),
        2,
        "the roster comes from @Participants, so MOT must still be reported"
    );

    assert_eq!(declared[1].code().as_str(), "MOT");
    assert_eq!(declared[1].name().map(|n| n.as_str()), Some("Mary"));
    assert_eq!(
        declared[1].role().as_str(),
        "Mother",
        "role comes from the declaration, which is the header that establishes it"
    );
    assert!(
        declared[1].id_metadata().is_none(),
        "the missing @ID must be visible to the caller, not silently absent"
    );
    Ok(())
}

/// No `@Participants` header at all yields an empty iterator rather than a
/// panic or a fabricated roster.
#[test]
fn no_participants_header_yields_no_declared_speakers() -> Result<(), TestError> {
    let source = "@UTF8\n@Begin\n*CHI:\thi .\n@End\n";
    let file = parse_keeping_model(source)?;
    assert_eq!(file.declared_speakers().count(), 0);
    Ok(())
}
