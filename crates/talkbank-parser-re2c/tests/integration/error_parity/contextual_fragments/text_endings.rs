//! Endings and continuations applied to the retained text-tier boundary controls.

use super::{
    CaseReport, Detection, ExampleOrdinal, INVOCATIONS, SpecLabel, clean, deck, parse_pair,
    source_findings,
};
use deck::Api;

#[test]
fn text_failure_is_distinct_from_genuine_empty_source_content() {
    let (_, control, structural_fault) = deck::BASE
        .iter()
        .find(|(api, _, _)| matches!(api, Api::Com))
        .unwrap();
    for invalid in [
        format!("{control}\n\n"),
        (*structural_fault).to_owned(),
        format!("{control}\0ignored"),
    ] {
        let errors = talkbank_model::ErrorCollector::new();
        assert!(
            talkbank_parser_re2c::parser::parse_text_tier_with_errors(&invalid, &errors)
                .is_rejected()
        );
        assert!(!errors.into_vec().is_empty());
    }

    let errors = talkbank_model::ErrorCollector::new();
    let empty = talkbank_parser_re2c::parser::parse_text_tier_with_errors("", &errors)
        .into_option()
        .expect("actual source emptiness is representable");
    assert!(empty.segments.is_empty());
    assert!(errors.into_vec().is_empty());
}

#[test]
fn comment_continuation_has_an_independent_model_witness() {
    let primary = talkbank_parser::TreeSitterParser::new().unwrap();
    let native = talkbank_parser_re2c::Re2cParser::new();
    let (_, control, _) = deck::BASE
        .iter()
        .find(|(api, _, _)| matches!(api, Api::Com))
        .unwrap();
    let input = format!("{control}\n\t{control}\n");
    let left = talkbank_model::ErrorCollector::new();
    let right = talkbank_model::ErrorCollector::new();
    let primary = talkbank_model::ChatParser::parse_com_tier(&primary, &input, 0, &left);
    let native = talkbank_model::ChatParser::parse_com_tier(&native, &input, 0, &right);
    for tier in [primary, native] {
        let tier = tier.into_option().expect("clean comment control");
        assert_eq!(
            tier.content
                .segments
                .iter()
                .filter(|segment| matches!(
                    segment,
                    talkbank_model::BulletContentSegment::Continuation
                ))
                .count(),
            1,
            "the source continuation must survive as a model segment"
        );
    }
    assert!(left.into_vec().is_empty());
    assert!(right.into_vec().is_empty());
}

#[test]
fn text_tier_bodies_retain_continuations_but_do_not_erase_blank_endings() -> Result<(), String> {
    let primary = talkbank_parser::TreeSitterParser::new().map_err(|error| error.to_string())?;
    let native = talkbank_parser_re2c::Re2cParser::new();
    let mut failures = Vec::new();
    let mut routes = 0;
    let mut controls = 0;
    let mut required = 0;
    for (index, (api, control, _)) in deck::BASE.iter().enumerate() {
        match api {
            Api::Act
            | Api::Cod
            | Api::Com
            | Api::Exp
            | Api::Add
            | Api::Gpx
            | Api::Int
            | Api::Spa
            | Api::Sit => {}
            Api::Header
            | Api::Id
            | Api::Participant
            | Api::Utterance
            | Api::Main
            | Api::Word
            | Api::MorTier
            | Api::MorWord
            | Api::GraTier
            | Api::GraRelation
            | Api::PhoTier
            | Api::PhoWord
            | Api::Sin
            | Api::Wor
            | Api::Dependent => continue,
        }
        routes += 1;
        for invocation in INVOCATIONS {
            for input in [
                (*control).to_owned(),
                format!("{control}\n"),
                format!("{control}\r\n"),
                format!("{control}\r"),
                format!("{control}\n\t{control}\n"),
                format!("{control}\r\n\t{control}\r\n"),
                // The grammar's continuation owns the complete newline run
                // before its tab; this is not a standalone blank-line ending.
                format!("{control}\n\n\t{control}\n"),
            ] {
                for sample in parse_pair((&primary, &native), *api, invocation, &input) {
                    controls += 1;
                    let label = format!("{api:?} {invocation:?} legal ending/continuation");
                    clean(&label, &sample, &mut failures);
                    source_findings(&label, &input, &sample, &mut failures);
                }
            }
            for ending in ["\n", "\r\n", "\r"] {
                let input = format!("{control}{ending}{ending}");
                for sample in parse_pair((&primary, &native), *api, invocation, &input) {
                    let label = format!("{api:?} {invocation:?} extra blank ending {ending:?}");
                    source_findings(&label, &input, &sample, &mut failures);
                    let [tree_sitter, re2c] = sample.reports;
                    let case = CaseReport {
                        label: SpecLabel::new(
                            "text-tier-endings",
                            ExampleOrdinal::from_zero_based(index),
                        ),
                        tree_sitter,
                        re2c,
                    };
                    match case.detection() {
                        Detection::Reported => required += 1,
                        Detection::NotRequired
                        | Detection::Missing(_)
                        | Detection::ToolFailure { .. } => {
                            failures.push(format!(
                                "{label} at {}: {}",
                                sample.origin,
                                case.detail()
                            ));
                        }
                    }
                }
            }
        }
    }
    if routes != 9 || controls != 630 || required != 270 {
        failures.push(format!("incomplete ending inventory: {routes} routes, {controls} clean controls, {required} detections"));
    }
    eprintln!(
        "text-tier endings: {routes} routes, {controls} clean semantic/source controls, {required} required detections, {} failures",
        failures.len()
    );
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}
