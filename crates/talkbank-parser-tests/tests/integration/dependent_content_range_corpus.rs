//! Source geometry, wire trust and coordinate boundaries for both real producers.
//! Variants are derived from the authored combination reference, not model text.

use talkbank_model::model::DependentTierEntry;
use talkbank_model::model::dependent_tier::DependentTierContentSpan;
use talkbank_model::{
    ChatFile, ChatParser, ErrorCollector, ParseOutcome, SemanticEq, Span, SpanShift,
};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_re2c::Re2cParser;

const CONTROL: &str = include_str!("../../../../corpus/reference/tiers/mor-gra-combinations.cha");

fn parse(parser: &impl ChatParser, source: &str, offset: usize) -> ChatFile {
    let errors = ErrorCollector::new();
    let ParseOutcome::Parsed(file) = ChatParser::parse_chat_file(parser, source, offset, &errors)
    else {
        panic!("reference-derived document rejected: {:?}", errors.to_vec());
    };
    assert!(
        !errors
            .to_vec()
            .iter()
            .any(|error| error.code == talkbank_model::ErrorCode::InternalError),
        "source admission failed: {:?}",
        errors.to_vec()
    );
    file
}

fn verify(parser: &impl ChatParser, backend: &str) -> Vec<String> {
    let mut failures = Vec::new();
    let variants = [
        (CONTROL.to_owned(), "middle naïve", "after résumé"),
        (
            CONTROL.replace("middle naïve", "middle naïve\n\tcontinued résumé  "),
            "middle naïve\n\tcontinued résumé  ",
            "after résumé",
        ),
        (
            CONTROL.replace("%com:\tmiddle naïve", "%com:\t  middle naïve"),
            "middle naïve",
            "after résumé",
        ),
        (CONTROL.replace("after résumé", ""), "middle naïve", ""),
        (
            CONTROL.replace("%com:\tbefore café", "%wor:\tstart .\n%com:\tbefore café"),
            "middle naïve",
            "after résumé",
        ),
        (
            CONTROL.replace("%com:\tmiddle naïve", "%xlonglabel:\tmiddle naïve"),
            "middle naïve",
            "after résumé",
        ),
    ];
    for (variant, (source, middle, last)) in variants.into_iter().enumerate() {
        for ending in ["\n", "\r\n", "\r"] {
            let label = format!("{backend} / variant {variant} / {ending:?}");
            let source = source.replace('\n', ending);
            let middle = middle.replace('\n', ending);
            let local = parse(parser, &source, 0);
            let shifted = parse(parser, &source, 23);
            let mut translated = local.clone();
            translated.shift_spans_after(0, 23);
            if translated != shifted {
                failures.push(format!("{label}: whole-file forward rebase"));
            }
            translated.shift_spans_after(23, -23);
            if translated != local {
                failures.push(format!("{label}: whole-file inverse rebase"));
            }
            let mut seen = 0;
            for (turn_index, turn) in local.utterances().enumerate() {
                for entry in &turn.dependent_tiers {
                    let Some(span) = entry.content_span() else {
                        failures.push(format!("{label}: unknown {} body", entry.kind()));
                        continue;
                    };
                    let Some(body) = source.get(span.start as usize..span.end as usize) else {
                        failures.push(format!("{label}: out-of-source {} body", entry.kind()));
                        continue;
                    };
                    match entry.kind() {
                        "com" | "xlonglabel" => {
                            let expected = match turn_index {
                                0 => "before café",
                                1 => &middle,
                                2 => last,
                                _ => panic!("unexpected turn"),
                            };
                            if body != expected {
                                failures.push(format!(
                                    "{label}: {} body {body:?}, expected {expected:?}",
                                    entry.kind()
                                ));
                            }
                            seen += 1;
                        }
                        "wor" => {
                            if body != "start ." {
                                failures.push(format!("{label}: nested word-tier body {body:?}"));
                            }
                        }
                        "mor" | "gra" => {
                            if body.ends_with(['\r', '\n']) || body.starts_with('%') {
                                failures
                                    .push(format!("{label}: included {} boundary", entry.kind()));
                            }
                        }
                        kind => panic!("unexpected selected tier {kind}"),
                    }
                }
            }
            if seen != 3 {
                failures.push(format!("{label}: retained {seen}/3 comment sentinels"));
            }
        }
    }
    failures
}

/// A missing final newline and @End is malformed input, not a requirement
/// that the native backends recover an identical last tier. Geometry is checked
/// only for a tier actually emitted; re2c's retained EOF tier must end at EOF.
fn verify_eof() {
    let canonical = TreeSitterParser::new().expect("grammar");
    let independent = Re2cParser::new();
    let source = CONTROL.trim_end_matches("@End\n").trim_end_matches('\n');
    for ending in ["\n", "\r\n", "\r"] {
        let source = source.replace('\n', ending);
        let errors = ErrorCollector::new();
        let ParseOutcome::Parsed(native) =
            ChatParser::parse_chat_file(&canonical, &source, 0, &errors)
        else {
            panic!("native recovery must retain independent turns");
        };
        let original = canonical
            .parse_source_incremental(&source, None)
            .expect("native source owner");
        assert!(
            original.root_node().has_error(),
            "actual native recovery evidence"
        );
        assert!(
            !errors.is_empty(),
            "unterminated input must have diagnostics"
        );
        assert!(
            !errors
                .to_vec()
                .iter()
                .any(|error| error.code == talkbank_model::ErrorCode::InternalError)
        );
        // Native document recovery can omit the whole unterminated final
        // utterance. Do not demand a made-up last main or body to match re2c.
        for entry in native.utterances().flat_map(|turn| &turn.dependent_tiers) {
            if let Some(span) = entry.content_span() {
                let body = source
                    .get(span.start as usize..span.end as usize)
                    .expect("emitted caller geometry");
                if span.end as usize == source.len() {
                    assert_eq!(body, "after résumé", "a retained EOF body loses no byte");
                }
            }
        }
        let retained = parse(&independent, &source, 0);
        let last = retained.utterances().nth(2).expect("last main");
        let entry = last
            .dependent_tiers
            .iter()
            .find(|entry| entry.kind() == "com")
            .expect("lexer-owned EOF tier");
        let span = entry.content_span().expect("known EOF range");
        assert_eq!(span.end as usize, source.len());
        assert_eq!(
            &source[span.start as usize..span.end as usize],
            "after résumé"
        );
    }
}

/// Surviving boundary category: native runtime ranges and original source bytes.
#[test]
fn dependent_content_ranges_follow_both_native_producers_and_rebase() {
    let mut failures = verify(&TreeSitterParser::new().expect("grammar"), "tree-sitter");
    failures.extend(verify(&Re2cParser::new(), "re2c"));
    assert!(
        failures.is_empty(),
        "complete body-range failure inventory:\n{}",
        failures.join("\n")
    );
    verify_eof();
}

/// Surviving wire category: provenance is absent even if a payload retains spans.
#[test]
fn dependent_content_ranges_do_not_cross_construction_or_wire_boundaries() {
    for file in [
        parse(&TreeSitterParser::new().expect("grammar"), CONTROL, 0),
        parse(&Re2cParser::new(), CONTROL, 0),
    ] {
        for entry in file.utterances().flat_map(|turn| &turn.dependent_tiers) {
            assert!(entry.content_span().is_some());
            let encoded = serde_json::to_value(entry).expect("entry wire");
            assert_eq!(
                encoded,
                serde_json::to_value(&entry.tier).expect("payload wire")
            );
            let decoded: DependentTierEntry =
                serde_json::from_value(encoded.clone()).expect("entry import");
            assert!(decoded.content_span().is_none());
            assert!(decoded.semantic_eq(entry));
            let constructed =
                DependentTierEntry::with_separator(entry.tier.clone(), entry.separator);
            assert!(constructed.content_span().is_none());
            let mut forged = encoded;
            forged.as_object_mut().expect("entry wire object").insert(
                "source_content".to_owned(),
                serde_json::json!({"start": 1, "end": 2}),
            );
            // Extra fields may be declined or ignored by payload serde; neither
            // path is permitted to turn an imported range into producer proof.
            if let Ok(imported) = serde_json::from_value::<DependentTierEntry>(forged) {
                assert!(imported.content_span().is_none());
            }
        }
    }
}

/// Surviving constructor boundary: caller-supplied geometry is checked, and
/// known empty does not collapse to unknown or to the old DUMMY sentinel.
#[test]
fn dependent_content_ranges_admit_only_ordered_contained_geometry() {
    let tier = Span::new(10, 30);
    assert!(DependentTierContentSpan::try_new(tier, Span::new(15, 15)).is_ok());
    for body in [Span::new(9, 20), Span::new(20, 31), Span::new(25, 15)] {
        assert!(DependentTierContentSpan::try_new(tier, body).is_err());
    }
}

/// Surviving source boundary: token exhaustion is not physical EOF when the
/// native lexer stops at an embedded NUL. Do not mint an endpoint or turn
/// malformed caller input into an internal range-construction failure.
#[test]
fn dependent_content_ranges_do_not_invent_eof_after_lexer_stops() {
    for (kind, original, stopped) in [
        ("mor", "verb|start .", "verb|start\0 ."),
        ("com", "before café", "before\0 café"),
    ] {
        let source = CONTROL.replacen(original, stopped, 1);
        let errors = ErrorCollector::new();
        let outcome = ChatParser::parse_chat_file(&Re2cParser::new(), &source, 0, &errors);
        assert!(
            !errors
                .to_vec()
                .iter()
                .any(|error| error.code == talkbank_model::ErrorCode::InternalError),
            "caller bytes are not a producer fault: {:?}",
            errors.to_vec()
        );
        assert!(
            !errors.is_empty(),
            "stopped malformed source requires diagnostics"
        );
        if let ParseOutcome::Parsed(file) = outcome {
            let mut retained = 0;
            for entry in file.utterances().flat_map(|turn| &turn.dependent_tiers) {
                if entry.kind() == kind {
                    assert!(
                        entry.content_span().is_none(),
                        "no selected ending was consumed"
                    );
                    retained += 1;
                }
            }
            if kind == "com" {
                assert_eq!(
                    retained, 1,
                    "actual retained text-tier geometry stays unknown"
                );
            }
        }
    }
}
