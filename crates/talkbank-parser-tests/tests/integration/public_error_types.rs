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

//! API-surface guard: every public fallible constructor's error type must
//! itself be publicly nameable by downstream crates, through a path rooted
//! at the SAME crate that exposes the constructor.
//!
//! Regression for the 2026-07-08 finding (BUG-3): `LanguageCode::new`
//! returned `Result<_, LanguageCodeError>` but `LanguageCodeError` was not
//! re-exported, so the first real downstream consumer (batchalign3) could
//! not store it in a `thiserror` `#[source]` field and had to stringify at
//! the boundary. The bug's exit criterion is a compile-test that names
//! EVERY public constructor error type; this file is that test. If it stops
//! compiling, a public constructor's error type has become unnameable from
//! its crate root again.
//!
//! Coverage is enumerated from the 2026-07-17 audit of every public
//! `-> Result<_, E>` / `impl TryFrom` across the published library crates
//! (talkbank-model, talkbank-parser, talkbank-transform). A new public
//! fallible constructor MUST add its error type here.

use talkbank_model::model::{LanguageCode, LanguageCodeError};

#[path = "catalog_api_boundaries.rs"]
mod catalog_api_boundaries;

#[path = "json_api_boundaries.rs"]
mod json_api_boundaries;

#[path = "filesystem_api_boundaries.rs"]
mod filesystem_api_boundaries;

#[path = "validation_failure_boundaries.rs"]
mod validation_failure_boundaries;

#[path = "semantic_eq_api_boundaries.rs"]
mod semantic_eq_api_boundaries;

/// Naming `T` in a turbofish forces its path to resolve at compile time, so
/// a type that becomes `pub(crate)` or loses its re-export breaks this test.
/// No trait bound: the bug is about NAMEABILITY. The stronger property (the
/// error type impls `std::error::Error`, so it can be a `#[source]`) is
/// demonstrated separately by `language_code_error_is_nameable_and_sourceable`.
fn assert_nameable<T>() {}

/// Typestate controls presence; this checks the public field/default policy.
#[test]
fn diagnostic_builder_preserves_fields_in_either_order() {
    use talkbank_model::{DiagnosticLocation, ErrorCode, ParseError, Severity, Span};
    let span = Span::new(1, 3);
    let error = ParseError::build(ErrorCode::ParseFailed)
        .message("initial")
        .severity(Severity::Warning)
        .at_span(span)
        .suggestion("repair")
        .message("final")
        .finish();
    assert_eq!(error.message, "final");
    assert_eq!(error.severity, Severity::Warning);
    assert_eq!(error.location.span().expect("located diagnostic"), span);
    assert_eq!(error.suggestion.as_deref(), Some("repair"));
    assert_eq!(
        error.help_url,
        Some(ErrorCode::ParseFailed.documentation_url())
    );
    let other = ParseError::build(ErrorCode::ParseFailed)
        .location(DiagnosticLocation::new(span))
        .message("final")
        .finish();
    assert_eq!(other.location.span().expect("located diagnostic"), span);
    assert_eq!(other.severity, Severity::Error);
    assert!(other.context.is_none());
}

/// A downstream producer's out-of-range location is API-failure evidence, not
/// a new CHAT error specimen. Admission must preserve evidence, not repair it.
#[test]
fn diagnostic_source_admission_preserves_evidence_and_inclusive_eof() {
    use talkbank_model::{
        DiagnosticSourceError, ErrorCode, ParseError, SourceIndex, Span, enhance_errors_with_index,
    };

    for source in ["ascii", "é", "字", "🙂", "a\né", "a\n"] {
        let end = source.len();
        // An inverted range is refused at admission, so it can never reach projection.
        assert!(Span::try_new(end, 0).is_err());
        let index = SourceIndex::new(source);
        for input in [
            Span::new(0, 0),
            Span::new(end, end),
            Span::new(end + 1, end + 2),
            Span::new(0, end + 1),
        ] {
            let errors = [ParseError::build(ErrorCode::InternalError)
                .message("downstream location boundary")
                .at_span(input)
                .finish()];
            let rendered: Vec<_> = enhance_errors_with_index(&errors, &index).collect();
            let error = &rendered[0];
            assert_eq!(error.original(), &errors[0]);
            match error.presentation() {
                Ok(presentation) => {
                    assert!(
                        input == Span::new(0, 0) || input == Span::new(end, end),
                        "only real source boundaries are admitted"
                    );
                    assert_eq!(
                        presentation.location().span().expect("located diagnostic"),
                        input
                    );
                    let context = presentation.context();
                    assert_eq!(
                        context.span().start(),
                        context.span().end(),
                        "insertion points stay zero-width even before Unicode"
                    );
                    assert!(
                        context
                            .source_text()
                            .get(context.span().to_range())
                            .is_some()
                    );
                }
                Err(reason) => {
                    assert!(
                        matches!(reason, DiagnosticSourceError::PrimaryRange { span, .. } if *span == input)
                    );
                    assert!(error.location().is_err());
                    assert!(error.context().is_err());
                }
            }
        }
        for middle in 0..end {
            if source.is_char_boundary(middle) {
                continue;
            }
            for span in [Span::new(middle, end), Span::new(0, middle)] {
                let raw = ParseError::build(ErrorCode::InternalError)
                    .at_span(span)
                    .message("UTF-8 boundary")
                    .finish();
                let shown = talkbank_model::SourceDiagnostic::with_index(raw.clone(), &index);
                assert_eq!(shown.original(), &raw);
                assert!(matches!(
                    shown.presentation(),
                    Err(DiagnosticSourceError::PrimaryRange { .. })
                ));
            }
        }
    }
}

/// Invalid secondary evidence invalidates the whole presentation, not the CHAT
/// finding. No partial relabeling or stale producer line can survive admission.
#[test]
fn diagnostic_source_labels_are_admitted_atomically() {
    use talkbank_model::{
        DiagnosticLocation, DiagnosticSourceError, ErrorCode, ErrorLabel, ParseError,
        SourceDiagnostic, SourceIndex, Span,
    };
    let source = "first\né🙂\n";
    let index = SourceIndex::new(source);
    // An inverted label cannot be built at all (`Span::try_new` refuses it);
    // out-of-source and non-boundary labels remain the rejected cases.
    for span in [Span::new(0, 100), Span::new(7, 8), Span::new(8, 9)] {
        let raw = ParseError::build(ErrorCode::UnparsableContent)
            .location(DiagnosticLocation::known_at(
                Span::new(0, 5),
                talkbank_model::LineColumn::one_based(99, 88).expect("one-based position"),
            ))
            .message("original finding")
            .label(ErrorLabel::new(Span::new(0, 5), "valid label"))
            .label(ErrorLabel::new(span, "invalid label"))
            .finish();
        let shown = SourceDiagnostic::with_index(raw.clone(), &index);
        assert_eq!(shown.original(), &raw);
        assert!(
            matches!(shown.presentation(), Err(DiagnosticSourceError::LabelRange { index: 1, span: rejected, .. }) if *rejected == span)
        );
        assert!(shown.location().is_err());
        assert!(!shown.to_string().contains("line 99"));
        assert!(shown.to_string().contains("source context unavailable"));
    }
    let raw = ParseError::build(ErrorCode::MissingEndHeader)
        .at_span(Span::new(0, 0))
        .message("empty buffer finding")
        .finish();
    let shown = SourceDiagnostic::with_index(raw.clone(), &SourceIndex::new(""));
    assert_eq!(shown.original(), &raw);
    assert_eq!(
        shown.presentation().expect_err("no source window"),
        &DiagnosticSourceError::EmptySource
    );
}

/// The canonical downstream shape: a domain error carrying the upstream
/// construction error as a typed source. This is what batchalign3 wants
/// to write; it must always be possible for at least the anchor type.
#[derive(Debug, thiserror::Error)]
enum DownstreamError {
    #[error("invalid language code {lang:?}")]
    InvalidLanguageCode {
        lang: String,
        #[source]
        source: LanguageCodeError,
    },
}

#[test]
fn language_code_error_is_nameable_and_sourceable() {
    let err = LanguageCode::new("").expect_err("empty code must fail");
    let wrapped = DownstreamError::InvalidLanguageCode {
        lang: String::new(),
        source: err,
    };
    assert!(std::error::Error::source(&wrapped).is_some());
}

/// talkbank-model: every public fallible constructor's error type, named
/// through its crate-root-reachable path.
#[test]
fn talkbank_model_constructor_error_types_are_nameable() {
    assert_nameable::<talkbank_model::model::annotation::EmptyReplacementWords>();
    assert_nameable::<talkbank_model::LanguageCodeError>();
    assert_nameable::<talkbank_model::SemanticWordIndexError>();
    assert_nameable::<talkbank_model::SourceLocationError>();
    assert_nameable::<talkbank_model::ParseErrorBuilder>();
    assert_nameable::<talkbank_model::ParseErrorBuilder<String, talkbank_model::DiagnosticLocation>>(
    );
    assert_nameable::<talkbank_model::SylWordError>();
    assert_nameable::<talkbank_model::XphointParseError>();
    assert_nameable::<talkbank_model::PhoalnParseError>();
    // `PositionCode::try_from(char)` has error type `char` (a primitive):
    // trivially nameable, and deliberately NOT asserted as an Error impl.
    // That it returns a bare `char` rather than a domain error is a minor
    // pre-existing API smell tracked separately, not part of BUG-3.
}

/// Empty collection admission is an API/wire refusal, not a fabricated CHAT model.
#[test]
fn empty_replacement_collections_are_rejected_at_all_admission_boundaries() {
    use talkbank_model::model::annotation::{EmptyReplacementWords, ReplacementWords};
    assert_eq!(
        ReplacementWords::new(Vec::new()),
        Err(EmptyReplacementWords)
    );
    assert_eq!(
        ReplacementWords::try_from(Vec::new()),
        Err(EmptyReplacementWords)
    );
    assert!(serde_json::from_str::<ReplacementWords>("[]").is_err());
    assert!(serde_json::from_str::<talkbank_model::model::Replacement>(r#"{"words":[]}"#).is_err());
}

/// Public duration strings have a wider domain than CHAT pause tokens.
/// Unsupported numeric projections preserve input; they do not certify CHAT syntax.
#[test]
fn pause_wire_admission_preserves_integer_and_unsupported_projection_states() {
    use talkbank_model::model::PauseTimedDuration;

    for (text, components) in [
        ("0", Some((0, None))),
        ("2", Some((2, None))),
        ("4294967295", Some((u32::MAX, None))),
        ("1:2", Some((62, None))),
        ("4294967296", None),
        ("4294967296.5", None),
        ("4294967296:1.5", None),
        ("1:4294967296.5", None),
        ("1:bad", None),
        ("1.é", None),
        ("1.12x", None),
        ("1.1234x", None),
        ("", None),
    ] {
        let wire = serde_json::json!({"seconds": text});
        let decoded: PauseTimedDuration = serde_json::from_value(wire.clone())
            .expect("string-valued duration payload is preserved");
        assert_eq!(decoded, PauseTimedDuration::new(text), "{text:?}");
        assert_eq!(decoded.as_str(), text);
        assert_eq!(
            serde_json::to_value(&decoded).expect("duration output"),
            wire
        );
        match (&decoded, components) {
            (PauseTimedDuration::Parsed(parsed), Some((seconds, millis))) => {
                assert_eq!(parsed.seconds(), seconds, "{text:?}");
                assert_eq!(parsed.millis(), millis, "{text:?}");
                assert_eq!(parsed.as_str(), text);
                assert_eq!(decoded.total_millis(), Some(u64::from(seconds) * 1000));
            }
            (PauseTimedDuration::Unsupported(raw), None) => {
                assert_eq!(raw, text);
                assert_eq!(decoded.total_millis(), None);
            }
            _ => panic!("wrong projection admission for {text:?}: {decoded:?}"),
        }
    }
}

/// Coordinate admission is a public API boundary, not a huge CHAT specimen.
#[test]
fn fragment_source_admission_checks_byte_extent_and_reports_once() {
    use talkbank_model::{
        ErrorCode, ErrorCollector, FragmentRangeError, FragmentSource, ParseOutcome, Span,
    };
    let limit = talkbank_model::MAX_PARSER_INPUT_BYTES;
    // Document origins are `usize`; only the parser input length is bounded.
    for (origin, len) in [
        (0, limit),
        (limit, 0),
        (limit - 2, 2),
        (limit, 1),
        (limit - 1, 2),
    ] {
        assert!(FragmentRangeError::check(origin, len).is_ok());
    }
    assert_eq!(
        FragmentRangeError::check(0, limit + 1),
        Err(FragmentRangeError::InputTooLarge { len: limit + 1 })
    );
    assert_eq!(
        FragmentRangeError::check(usize::MAX, 1),
        Err(FragmentRangeError::RangeOverflows {
            origin: usize::MAX,
            len: 1
        })
    );
    let input = "é";
    let errors = ErrorCollector::new();
    let ParseOutcome::Parsed(source) = FragmentSource::admit(input, limit - 2, &errors) else {
        panic!("exact byte extent must fit");
    };
    assert_eq!(source.input(), input);
    assert!(errors.is_empty());
    assert_eq!(
        source.rebase(Span::new(0, 2)).expect("contained span"),
        Span::new(limit - 2, limit)
    );
    assert_eq!(
        source
            .rebase(talkbank_model::Location::Unlocated)
            .expect("unlocated value"),
        talkbank_model::Location::Unlocated
    );
    // A range whose end does not exist is refused once, without a location.
    assert!(matches!(
        FragmentSource::admit(input, usize::MAX, &errors),
        ParseOutcome::Rejected
    ));
    let diagnostics = errors.to_vec();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, ErrorCode::ParseFailed);
    assert_eq!(
        diagnostics[0].location,
        talkbank_model::DiagnosticLocation::Unlocated
    );
    assert!(diagnostics[0].context.is_none());
}

/// Extreme caller coordinates are a boundary contract, not canonical CHAT
/// coverage. The parser consumes ordinary source-bound reference tier slices.
#[test]
fn reference_tier_parsers_honor_the_full_admitted_coordinate_range() {
    use talkbank_model::{ChatParser, ErrorCode, ErrorCollector, ParseOutcome, SemanticEq, Span};
    let parser = talkbank_parser::TreeSitterParser::new().expect("parser");
    let mut witnessed = std::collections::BTreeSet::new();
    for source in [
        include_str!("../../../../corpus/reference/tiers/mor-gra.cha"),
        include_str!("../../../../corpus/reference/tiers/pho-groupings.cha"),
        include_str!("../../../../corpus/reference/annotation/groups-sign.cha"),
    ] {
        let file = talkbank_parser_tests::test_error::strict_parse(parser.parse_chat_file(source))
            .expect("reference parses");
        for entry in file.utterances().flat_map(|u| &u.dependent_tiers) {
            let span = entry.location();
            let input = source
                .get(span.span().expect("located").start()..span.span().expect("located").end())
                .expect("source-bound tier");
            // Origins past the former 32-bit coordinate limit are admitted;
            // only a range whose end overflows is refused.
            let former_limit = u32::MAX as usize - input.len();
            for origin in [
                i32::MAX as usize,
                i32::MAX as usize + 1,
                former_limit,
                former_limit + 1,
            ] {
                let errors = ErrorCollector::new();
                let ParseOutcome::Parsed(tier) =
                    ChatParser::parse_dependent_tier(&parser, input, origin, &errors)
                else {
                    panic!(
                        "admitted {} tier rejected at {origin}: {:?}",
                        entry.kind(),
                        errors.to_vec()
                    );
                };
                assert!(errors.is_empty(), "{:?}", errors.to_vec());
                assert!(
                    tier.semantic_eq(&entry.tier),
                    "coordinate placement cannot change payload"
                );
                assert_eq!(
                    tier.location(),
                    talkbank_model::Location::Located(Span::new(origin, origin + input.len()))
                );
                witnessed.insert(entry.kind().to_owned());
            }
            let errors = ErrorCollector::new();
            assert!(matches!(
                ChatParser::parse_dependent_tier(
                    &parser,
                    input,
                    usize::MAX - input.len() + 1,
                    &errors
                ),
                ParseOutcome::Rejected
            ));
            let diagnostics = errors.to_vec();
            assert_eq!(
                diagnostics.len(),
                1,
                "one admission refusal, not parser recovery"
            );
            assert_eq!(diagnostics[0].code, ErrorCode::ParseFailed);
            assert_eq!(
                diagnostics[0].location,
                talkbank_model::DiagnosticLocation::Unlocated
            );
            assert!(diagnostics[0].context.is_none());
        }
    }
    assert_eq!(
        witnessed,
        ["mor", "gra", "pho", "sin"].map(str::to_owned).into()
    );
}

/// Document coordinates move; snippet-local context and diagnostic content do not.
#[test]
fn fragment_source_rebases_labels_without_rebasing_snippets() {
    use talkbank_model::{
        ErrorCode, ErrorCollector, ErrorLabel, ErrorSink, FragmentSource, ParseError, Severity,
        Span,
    };
    let source = FragmentSource::new("a é", 17).expect("small admitted source");
    let original = ParseError::from_source_span(
        ErrorCode::ParseFailed,
        Severity::Error,
        Span::new(2, 4),
        "a é",
        "é",
        "boundary witness",
    )
    .with_label(ErrorLabel::new(Span::new(0, 1), "related"))
    // An unlocated value gets no label.
    .with_label_at(talkbank_model::Location::Unlocated, "unknown location");
    let errors = ErrorCollector::new();
    source.error_sink(&errors).report(original.clone());
    let diagnostics = errors.to_vec();
    assert_eq!(diagnostics.len(), 1);
    let actual = &diagnostics[0];
    assert_eq!(
        actual.location.span().expect("located diagnostic"),
        Span::new(19, 21)
    );
    assert_eq!(actual.labels[0].span, Span::new(17, 18));
    assert_eq!(actual.labels.len(), 1, "an unlocated label is not attached");
    assert_eq!(actual.context, original.context);
    assert_eq!(actual.message, original.message);
    assert_eq!(actual.code, original.code);

    // A wrapper far into the document rebases without allocating a giant source.
    let wrapper_start = i32::MAX as usize + 100;
    assert_eq!(
        source
            .rebase_from(
                Span::new(wrapper_start + 2, wrapper_start + 4),
                wrapper_start
            )
            .expect("contained synthetic span"),
        Span::new(19, 21)
    );
    let zero = FragmentSource::new("a é", 0).expect("zero-origin source");
    assert_eq!(
        zero.rebase(Span::new(2, 4)).expect("contained span"),
        Span::new(2, 4)
    );
}

/// talkbank-parser: the parser's public methods return
/// `ParseResult<T> = Result<T, ParseErrors>`. Both must be nameable from
/// `talkbank_parser`'s OWN root, so a crate that depends only on
/// talkbank-parser (not talkbank-model) can name the error type of a method
/// it calls. This is the BUG-3 defect the fix closes.
#[test]
fn talkbank_parser_constructor_error_types_are_nameable() {
    assert_nameable::<talkbank_parser::ParserInitError>();
    assert_nameable::<talkbank_parser::ParseErrors>();
    assert_nameable::<talkbank_parser::ParseResult<()>>();
}

/// talkbank-transform: every public fallible constructor / serializer error
/// type, named through its crate-root-reachable path.
#[test]
fn talkbank_transform_constructor_error_types_are_nameable() {
    assert_nameable::<talkbank_transform::JsonError>();
    assert_nameable::<talkbank_transform::build_chat::BuildChatError>();
    assert_nameable::<talkbank_transform::validate::ValidationError>();
    assert_nameable::<talkbank_transform::adjudication::AdjudicationError>();
    assert_nameable::<talkbank_transform::rediarize::InvertedSpan>();
    assert_nameable::<talkbank_transform::rediarize::TurnsJsonError>();
    assert_nameable::<talkbank_transform::speaker_id::SpeakerIdError>();
    assert_nameable::<talkbank_transform::speaker_id::OverrideFileError>();
    assert_nameable::<talkbank_transform::speaker_id::SessionContextError>();
    assert_nameable::<talkbank_transform::speaker_id::BlankLabelError>();
}
