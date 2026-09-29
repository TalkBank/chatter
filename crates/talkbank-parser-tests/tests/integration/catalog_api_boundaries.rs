//! Independently supplied diagnostics are an API boundary, not CHAT specimens.
//! Keep this under the separate public-error boundary measurement family.

use talkbank_model::{ErrorCode, ErrorCollector, ParseError, Span, model::TranscriptName};
use talkbank_parser::TreeSitterParser;
use talkbank_transform::splice::catalog_fix;

#[test]
fn catalog_does_not_infer_a_fault_from_a_diagnostic_code_at_a_valid_node() {
    use talkbank_parser::generated_traversal::{AsRawNode, MediaHeaderNode, StandaloneWordNode};
    let parser = TreeSitterParser::new().expect("parser");
    for source in [
        include_str!("../../../../corpus/reference/core/basic-conversation.cha"),
        include_str!("../../../../corpus/reference/core/headers-media-http-url.cha"),
        include_str!("../error_corpus/validation_errors/W109_4.cha"),
    ] {
        let (_, parsed) = parser.parse_chat_file_with_source(source, &ErrorCollector::new());
        let parsed = parsed.expect("source owner");
        let root = parsed.bind(parsed.root_node()).expect("bound root");
        let mut witnesses = 0;
        for node in root.descendants() {
            let node = node.expect("source-bound descendant");
            let code = if node.typed::<StandaloneWordNode>().is_some() {
                ErrorCode::IllegalUntranscribed
            } else if node.typed::<MediaHeaderNode>().is_some() {
                ErrorCode::MediaFilenameNonCanonicalUnicode
            } else {
                continue;
            };
            let range = node.raw_node().byte_range();
            let diagnostic = ParseError::build(code)
                .message("independent caller")
                .at_span(Span::from_usize(range.start, range.end))
                .finish();
            assert!(
                catalog_fix(&diagnostic, &parsed).is_none(),
                "valid node with {code:?}"
            );
            witnesses += 1;
        }
        assert!(witnesses > 0, "each source must exercise a typed target");
    }
}

#[test]
fn catalog_refuses_unbound_locations_and_unsupported_fix_requests() {
    use ErrorCode::*;
    let parser = TreeSitterParser::new().expect("parser");
    let source = include_str!("../../../../corpus/reference/core/basic-conversation.cha");
    let errors = ErrorCollector::new();
    let (file, parsed) = parser.parse_chat_file_with_source(source, &errors);
    let parsed = parsed.expect("source owner");
    let speech = file
        .utterances()
        .next()
        .expect("reference speech")
        .main
        .span;

    // These fixes require a typed enclosing node. An unrelated diagnostic
    // location must not be clamped onto a convenient node and produce an edit.
    for code in [
        IllegalUntranscribed,
        ConsecutiveStressMarkers,
        ConsecutiveCommas,
        CommaAfterNonSpokenContent,
        MissingTerminator,
        EmptyUtterance,
        DuplicateHeader,
        GraWithoutMor,
        SpaceInsideAngleGroup,
        MediaFilenameNonCanonicalUnicode,
    ] {
        for span in [
            Span::new(source.len() as u32, source.len() as u32 + 1),
            Span::new(u32::MAX - 1, u32::MAX),
        ] {
            let diagnostic = ParseError::build(code)
                .message("independent caller")
                .at_span(span)
                .finish();
            assert!(
                catalog_fix(&diagnostic, &parsed).is_none(),
                "{code:?} at {span:?}"
            );
        }
    }

    // Even a real speech extent cannot supply missing facts or determine an
    // unsupported repair policy. These are intentionally synthetic requests.
    for code in [
        UndeclaredSpeaker,
        MissingRequiredHeader,
        EmptyLanguagesHeader,
        MissingMainTier,
        UnbalancedQuotation,
        TimestampBackwards,
        EmptyColon,
        EmptyParticipantsHeader,
        UnclosedBracket,
        UnclosedParenthesis,
        MissingColonAfterSpeaker,
    ] {
        let diagnostic = ParseError::build(code)
            .message("independent caller")
            .at_span(speech)
            .finish();
        assert!(catalog_fix(&diagnostic, &parsed).is_none(), "{code:?}");
    }

    // Positive controls stop an always-refuse implementation from passing.
    for (source, code) in [
        (
            include_str!(
                "../error_corpus/validation_errors/E241_illegal_untranscribed_marker_2.cha"
            ),
            IllegalUntranscribed,
        ),
        (
            include_str!("../error_corpus/validation_errors/E258_1.cha"),
            ConsecutiveCommas,
        ),
    ] {
        let errors = ErrorCollector::new();
        let (file, parsed) = parser.parse_chat_file_with_source(source, &errors);
        file.validate(&errors, TranscriptName::Anonymous);
        let diagnostics = errors.into_vec();
        let diagnostic = diagnostics
            .iter()
            .find(|error| error.code == code)
            .expect("authored finding");
        assert!(catalog_fix(diagnostic, &parsed.expect("source owner")).is_some());
    }
}
