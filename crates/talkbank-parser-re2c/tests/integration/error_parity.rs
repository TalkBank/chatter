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

//! One-way error detection for the independent parser backends.
//!
//! Every authored error-spec example is measured, including legal claims and
//! deferred codes. If tree-sitter reports an error, re2c must report some error.
//! Codes, wording, counts, order, locations, recovery and rejection stage may
//! differ. Actual severities and separate parser/validator observations survive
//! measurement; warnings and internal tool failure cannot satisfy rejection.
//!
//! The former named diagnostic-set baseline admitted silent invalid input and
//! rejected acceptable generic errors. It no longer controls acceptance. Dated
//! observations remain in the parity report and Git history.
//!
//! This finite file-level check is not specification completeness or fragment
//! API acceptance. Isolated-fault controls remain necessary: an unrelated error
//! in a multi-fault example can conceal a missing detection.

mod contextual_fragments;
mod empty_containers;
mod fragments;
mod model;
mod native_locations;
mod remaining_fragments;
mod spec_corpus;

use talkbank_model::ErrorCollector;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::gate::{listing, report};

use crate::fixture_utils::LineEnding;
use model::{CaseReport, Detection};
use spec_corpus::{SpecCorpus, load_spec_corpus};

/// Run both native backends once per variant, retaining every example and phase.
fn measure(corpus: &SpecCorpus, ending: LineEnding) -> Result<Vec<CaseReport>, String> {
    let parser = TreeSitterParser::new()
        .map_err(|error| format!("cannot load the CHAT grammar: {error}"))?;
    Ok(corpus
        .cases
        .iter()
        .map(|case| {
            let input = case.input.with_line_ending(ending);
            CaseReport {
                label: case.label.clone(),
                tree_sitter: input
                    .measure_with(|input, errors| parser.parse_chat_file_streaming(input, errors)),
                re2c: input.measure_with(talkbank_parser_re2c::parser::parse_chat_file_to_model),
            }
        })
        .collect())
}

/// Produce the complete failure inventory and its verdict in the same pass.
fn audit() -> Result<String, String> {
    let corpus = load_spec_corpus()?;
    let mut summaries = Vec::new();
    let mut refusals = Vec::new();
    for ending in LineEnding::ALL {
        // Keep every transport variant's complete inventory, even if an
        // earlier variant fails. A later success cannot erase that refusal.
        match audit_variant(&corpus, ending) {
            Ok(summary) => summaries.push(summary),
            Err(refusal) => refusals.push(refusal),
        }
    }
    if refusals.is_empty() {
        Ok(summaries.join("\n"))
    } else {
        Err(refusals.join("\n"))
    }
}

fn audit_variant(corpus: &SpecCorpus, ending: LineEnding) -> Result<String, String> {
    let reports = measure(corpus, ending)?;
    let mut detected = 0;
    let mut not_required = 0;
    let mut equal_codes = 0;
    let mut missing = Vec::new();
    let mut failures = Vec::new();
    for measured in &reports {
        if measured.tree_sitter.codes() == measured.re2c.codes() {
            equal_codes += 1;
        }
        match measured.detection() {
            Detection::Reported => detected += 1,
            Detection::NotRequired => not_required += 1,
            Detection::Missing(finding) => missing.push(format!(
                "{}; required by {} at {:?}",
                measured.detail(),
                finding.diagnostic().code,
                finding
                    .diagnostic()
                    .location
                    .span()
                    .expect("located diagnostic")
            )),
            Detection::ToolFailure { backend, finding } => failures.push(format!(
                "{}: {backend} tool failure {} at {:?}",
                measured.label,
                finding.code,
                finding.location.span().expect("located diagnostic")
            )),
        }
    }
    let summary = format!(
        "{ending:?}: {} example(s) over {} spec file(s); {detected} required detections satisfied, {} missing, {not_required} without tree-sitter errors, {} tool failures\n     {} legal-claim examples and {} not-implemented spec files INCLUDED\n     {equal_codes} equal code sets, {} different (informational only)",
        reports.len(),
        corpus.files_scanned,
        missing.len(),
        failures.len(),
        corpus.legal,
        corpus.not_implemented,
        reports.len() - equal_codes,
    );
    if missing.is_empty() && failures.is_empty() {
        Ok(summary)
    } else {
        Err(report(vec![
            listing(
                "MISSED TREE-SITTER ERRORS: warnings and silence are not errors:",
                &missing,
            ),
            listing(
                "BROKEN MEASUREMENT: internal failure is not input rejection:",
                &failures,
            ),
            format!("Measured over: {summary}"),
        ]))
    }
}

/// Survives: independent implementations must meet the one-way error policy.
#[test]
fn backends_report_every_tree_sitter_error() -> Result<(), String> {
    println!("ok  one-way backend error detection: {}", audit()?);
    Ok(())
}

/// Survives: malformed authored input must not panic during native recovery.
#[test]
fn re2c_never_panics_on_invalid_input() -> Result<(), String> {
    let corpus = load_spec_corpus()?;
    for case in &corpus.cases {
        let errors = ErrorCollector::new();
        let _ = talkbank_parser_re2c::parser::parse_chat_file_to_model(case.input.text(), &errors);
    }
    println!(
        "ok  {} authored example(s) parsed without panic",
        corpus.cases.len()
    );
    Ok(())
}

/// Surviving behavior: lexer locations and fragment rebasing must agree with
/// the source spec; the type system cannot decide which byte is the comma.
#[test]
fn participants_recovery_preserves_entries_and_fragment_locations() {
    use talkbank_model::model::Header;
    use talkbank_model::{ChatParser, ErrorCode, ParseOutcome};
    // The same token-state parser handles legal continuations and malformed
    // separators. Exercise its fragment route with the committed reference;
    // the reference comparison covers the full-file route separately.
    let reference = include_str!("../../../../corpus/reference/core/multiline-continuation.cha");
    let start = reference.find("@Participants:").unwrap();
    let end = start + reference[start..].find("@ID:").unwrap();
    let ts = talkbank_parser::TreeSitterParser::new().unwrap();
    let re2c = talkbank_parser_re2c::Re2cParser::new();
    for line_ending in ["\n", "\r\n", "\r"] {
        let header = reference[start..end].replace('\n', line_ending);
        let ts_errors = ErrorCollector::new();
        let re2c_errors = ErrorCollector::new();
        let ParseOutcome::Parsed(ts_header) =
            ChatParser::parse_header(&ts, &header, start, &ts_errors)
        else {
            panic!("valid reference participant header must parse");
        };
        let ParseOutcome::Parsed(re2c_header) = re2c.parse_header(&header, start, &re2c_errors)
        else {
            panic!("re2c must retain valid multiline participants");
        };
        assert!(ts_errors.into_vec().is_empty(), "{line_ending:?}");
        assert!(re2c_errors.into_vec().is_empty(), "{line_ending:?}");
        assert!(
            talkbank_model::SemanticEq::semantic_eq(&ts_header, &re2c_header),
            "{line_ending:?}"
        );
        let Header::Participants { entries } = re2c_header else {
            panic!("participant header kind must be retained");
        };
        assert_eq!(entries.len(), 3);
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let specs = talkbank_parser_tests::error_specs::load(root).unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E550.md")
        .unwrap();
    let source = spec.examples()[0].chat.as_str();
    let header = source
        .lines()
        .find(|line| line.starts_with("@Participants:"))
        .unwrap();
    let offset = source.find(header).unwrap();
    let comma = offset + header.rfind(',').unwrap();
    let parser = talkbank_parser_re2c::Re2cParser::new();
    let errors = ErrorCollector::new();
    let ParseOutcome::Parsed(Header::Participants { entries }) =
        parser.parse_header(header, offset, &errors)
    else {
        panic!("trailing-comma recovery must retain a participant header");
    };
    assert_eq!(entries.len(), 2);
    let diagnostic = errors
        .into_vec()
        .into_iter()
        .find(|error| error.code == ErrorCode::TrailingCommaInParticipants)
        .unwrap();
    assert_eq!(
        diagnostic.location.span().expect("located diagnostic"),
        talkbank_model::Span::new(comma, comma + 1)
    );
}

/// Wire behavior: both backends must preserve utterances and locate the same
/// blank logical line across the line-ending forms declared by the spec.
#[test]
fn blank_line_recovery_matches_for_each_spec_line_ending() {
    use talkbank_model::{ChatParser, ErrorCode};
    use talkbank_spec_vocabulary::frontmatter::Claim;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let specs = talkbank_parser_tests::error_specs::load(root).unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E747.md")
        .unwrap();
    let canonical = TreeSitterParser::new().unwrap();
    let re2c = talkbank_parser_re2c::Re2cParser::new();
    fn parse<P: ChatParser>(parser: &P, input: &str) -> Vec<talkbank_model::Span> {
        let errors = ErrorCollector::new();
        let talkbank_model::ParseOutcome::Parsed(file) = parser.parse_chat_file(input, 0, &errors)
        else {
            panic!("blank-line recovery must preserve the parsed document");
        };
        assert_eq!(file.utterances().count(), 2);
        errors
            .into_vec()
            .into_iter()
            .filter(|error| error.code == ErrorCode::BlankLineNotAllowed)
            .map(|error| error.location.span().expect("located diagnostic"))
            .collect()
    }
    for example in spec.examples() {
        let canonical_spans = parse(&canonical, example.chat.as_str());
        let re2c_spans = parse(&re2c, example.chat.as_str());
        let expected = usize::from(matches!(example.claim, Claim::Violates));
        for (backend, spans) in [("tree-sitter", &canonical_spans), ("re2c", &re2c_spans)] {
            assert_eq!(spans.len(), expected, "{backend}: {:?}", example.chat);
        }
        assert_eq!(canonical_spans, re2c_spans, "{:?}", example.chat);
    }
}

/// Separator admission is independent of whether the malformed tier has content.
#[test]
fn malformed_tier_prefixes_report_the_complete_source_line() {
    use talkbank_model::{ChatParser, ErrorCode, ParseOutcome, Span};
    use talkbank_spec_vocabulary::frontmatter::Claim;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let specs = talkbank_parser_tests::error_specs::load(root).unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E602.md")
        .unwrap();
    let parser = talkbank_parser_re2c::Re2cParser::new();
    for example in spec.examples() {
        let source = example.chat.as_str();
        let line = source.lines().find(|line| line.starts_with('%')).unwrap();
        let start = source.find(line).unwrap();
        let errors = ErrorCollector::new();
        let ParseOutcome::Parsed(file) = parser.parse_chat_file(source, 0, &errors) else {
            panic!("tier recovery must retain the preceding utterance");
        };
        assert_eq!(file.utterances().count(), 1);
        let spans: Vec<_> = errors
            .into_vec()
            .into_iter()
            .filter(|error| error.code == ErrorCode::MalformedTierHeader)
            .map(|error| error.location.span().expect("located diagnostic"))
            .collect();
        match example.claim {
            // The canonical parser now rejects these through E316. re2c owns
            // a lexer-level malformed-header token and retains E602 honestly.
            Claim::Violates | Claim::SubsumedBy(_) => {
                assert_eq!(spans, vec![Span::new(start, start + line.len())]);
            }
            Claim::Legal => assert!(spans.is_empty()),
        }
    }
}

/// Report the grammar-owned opening token, without imitating CST recovery.
#[test]
fn glued_replacement_reports_its_own_opening_token() {
    use talkbank_model::{ChatParser, ErrorCode, Span};
    use talkbank_spec_vocabulary::frontmatter::Claim;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let specs = talkbank_parser_tests::error_specs::load(root).unwrap();
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E375.md")
        .unwrap();
    let re2c = talkbank_parser_re2c::Re2cParser::new();
    fn diagnostics(parser: &impl ChatParser, source: &str) -> Vec<(String, Span)> {
        let errors = ErrorCollector::new();
        let _ = parser.parse_chat_file(source, 0, &errors);
        let mut result: Vec<_> = errors
            .into_vec()
            .into_iter()
            .filter(|error| {
                matches!(
                    error.code,
                    ErrorCode::UnparsableContent | ErrorCode::ContentAnnotationParseError
                )
            })
            .map(|error| {
                (
                    error.code.to_string(),
                    error.location.span().expect("located diagnostic"),
                )
            })
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }
    let examples: Vec<_> = spec
        .examples()
        .iter()
        .filter(|example| example.chat.as_str().contains("[: foo]"))
        .collect();
    assert_eq!(
        examples.len(),
        2,
        "the source spec must retain the violation and its legal control"
    );
    for example in examples {
        let source = example.chat.as_str();
        let expected = if matches!(example.claim, Claim::Legal) {
            Vec::new()
        } else {
            let start = source.find("[: foo]").expect("authored replacement opener");
            vec![(
                ErrorCode::ContentAnnotationParseError.to_string(),
                Span::new(start, start + 2),
            )]
        };
        assert_eq!(diagnostics(&re2c, source), expected);
    }
}
