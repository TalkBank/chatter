//! Body faults and ending tokens consume the same admitted source capabilities.
//! Native carrier observations govern retention, not unchanged raw-line guesses.

use super::admitted::EndingField;
use super::*;
use talkbank_parser::generated_traversal::{
    BulletNode, NodeSlot, PostcodeNode, TierBodyNode, UtteranceEndNode,
};

const CONTROL: &str = include_str!("../../../../corpus/reference/content/body-ending-controls.cha");
const WORDS: [&str; 4] = ["alder", "maple", "pine", "larch"];

#[derive(Clone, Copy)]
enum BulletBoundary {
    SyntaxRecovery,
    DecoderRefusal,
}

#[derive(Clone, Copy)]
enum TailPolicy {
    /// Preserve the actual healthy ending's payload, not an earlier raw tail.
    NativeCarrier,
    MissingTerminator,
    RejectedBullet(BulletBoundary),
    Recovery,
}

fn check_candidate(
    candidate: &Candidate<'_>,
    stem: &str,
    index: usize,
    verdict: Verdict,
    policy: TailPolicy,
    failures: &mut Vec<String>,
) {
    let path = candidate.fixture(stem, index);
    let file = observe(candidate, verdict, NativeNeighbors::Isolated, failures);
    let parser = TreeSitterParser::new().expect("parser");
    let native = parser
        .parse_source_incremental(&candidate.source, None)
        .expect("source-owned native observation");
    let root = native.root().expect("source owner");
    let mut main = None;
    let mut body = None;
    let mut ending = None;
    for node in root.descendants() {
        let node = node.expect("native range admission");
        let raw = node.raw_node();
        if raw.start_byte() == candidate.tier.start
            && let Some(typed) = node.typed::<MainTierNode>()
        {
            main = Some(typed);
        }
        if candidate.tier.start <= raw.start_byte() && raw.end_byte() <= candidate.tier.end {
            if let Some(typed) = node.typed::<TierBodyNode>() {
                body = Some(typed);
            }
            if let Some(typed) = node.typed::<UtteranceEndNode>() {
                ending = Some(typed);
            }
        }
    }
    let main = main.expect("this deck retains a main carrier");
    let body = body.expect("this deck retains a body carrier");
    let ending = ending.expect("this deck retains an ending carrier");
    let main_slots = main.extract();
    let SourceSlotView::Present(positioned_body) = main_slots.field_child_5().slot().view() else {
        panic!("this finite deck has a positioned body, not displaced-body evidence");
    };
    assert_eq!(
        positioned_body.read().expect("body owner").raw_node(),
        body.raw_node()
    );
    let body_slots = body.extract();
    assert!(matches!(
        body_slots.field_content_2().slot().view(),
        SourceSlotView::Present(_)
    ));
    let SourceSlotView::Present(positioned_end) = body_slots.field_ending().slot().view() else {
        panic!("omitted terminators do not establish absent ending carriers");
    };
    assert_eq!(
        positioned_end.read().expect("ending owner").raw_node(),
        ending.raw_node()
    );
    println!(
        "{stem}_{index}: main={:?}, body={:?}, ending={:?}",
        (main.raw_node().byte_range(), main.raw_node().has_error()),
        (body.raw_node().byte_range(), body.raw_node().has_error()),
        (
            ending.raw_node().byte_range(),
            ending.raw_node().has_error()
        )
    );

    let expected = &candidate
        .control
        .file()
        .utterances()
        .find(|utterance| {
            utterance.main.location.span().expect("located").start()
                == candidate.original_tier.start
        })
        .expect("control owns the edited turn")
        .main;
    let actual = file
        .utterances()
        .find(|utterance| {
            utterance.main.location.span().expect("located").start() == candidate.tier.start
        })
        .map(|utterance| &utterance.main);
    let healthy_ending = !ending.raw_node().has_error();
    match policy {
        TailPolicy::NativeCarrier => {
            assert!(
                healthy_ending,
                "body cases have healthy actual ending carriers"
            );
            let associated = ending.extract();
            let slots = associated.children();
            let terminator = match slots.child_0.slot() {
                Some(NodeSlot::Present(choice)) => Some(
                    candidate
                        .source
                        .get(choice.raw_node().byte_range())
                        .expect("native terminator")
                        .to_owned(),
                ),
                None => None,
                other => panic!("healthy ending's terminator: {other:?}"),
            };
            let range = ending.raw_node().byte_range();
            let mut codes = Vec::new();
            let mut bullet = None;
            for node in root.descendants() {
                let node = node.expect("source-owned ending descendant");
                let raw = node.raw_node();
                if raw.start_byte() < range.start || range.end < raw.end_byte() {
                    continue;
                }
                if let Some(code) = node.typed::<PostcodeNode>() {
                    codes.push(code.text().to_owned());
                }
                if let Some(timing) = node.typed::<BulletNode>() {
                    assert!(bullet.replace(timing).is_none(), "one terminal slot");
                }
            }
            let actual = actual.expect("positioned body's retained model");
            assert_eq!(
                actual
                    .content
                    .terminator
                    .as_ref()
                    .map(WriteChat::to_chat_string),
                terminator,
                "{stem}_{index}: actual native terminator"
            );
            assert_eq!(
                actual
                    .content
                    .postcodes
                    .iter()
                    .map(WriteChat::to_chat_string)
                    .collect::<Vec<_>>(),
                codes,
                "{stem}_{index}: actual native final codes"
            );
            if let Some(bullet) = bullet {
                let expected = expected
                    .content
                    .bullet
                    .as_ref()
                    .expect("unchanged timing control");
                assert_eq!(
                    bullet.raw_node().byte_range(),
                    candidate.translated(expected.location.span().expect("located"))
                );
                assert!(
                    actual
                        .content
                        .bullet
                        .as_ref()
                        .is_some_and(|value| expected.semantic_eq(value)),
                    "{stem}_{index}: healthy grammar-owned terminal timing lost"
                );
            }
        }
        TailPolicy::MissingTerminator => {
            if !healthy_ending
                || !actual.is_some_and(|actual| {
                    actual.content.terminator.is_none()
                        && expected
                            .content
                            .postcodes
                            .semantic_eq(&actual.content.postcodes)
                        && expected.content.bullet.semantic_eq(&actual.content.bullet)
                })
            {
                failures.push(format!(
                    "{stem}_{index}: terminator omission lost its ending payload"
                ));
            }
        }
        TailPolicy::RejectedBullet(boundary) => {
            assert_eq!(
                healthy_ending,
                matches!(boundary, BulletBoundary::DecoderRefusal),
                "legacy syntax recovery is not a clean numeric decoder rejection"
            );
            if !actual.is_some_and(|actual| {
                expected
                    .content
                    .terminator
                    .semantic_eq(&actual.content.terminator)
                    && expected
                        .content
                        .postcodes
                        .semantic_eq(&actual.content.postcodes)
                    && actual.content.bullet.is_none()
            }) {
                failures.push(format!(
                    "{stem}_{index}: bullet refusal invented timing or lost codes"
                ));
            }
        }
        TailPolicy::Recovery => {}
    }
    let input = candidate
        .source
        .get(candidate.tier.clone())
        .expect("owned caller bytes");
    for input in [input, input.trim_end_matches('\n')] {
        assert_fragment_rebases(&path, input, |offset, errors| {
            ChatParser::parse_main_tier(&parser, input, offset, errors)
        });
        let errors = ErrorCollector::new();
        let outcome = ChatParser::parse_main_tier(&parser, input, 0, &errors);
        match policy {
            TailPolicy::NativeCarrier
            | TailPolicy::Recovery
            | TailPolicy::RejectedBullet(BulletBoundary::SyntaxRecovery) => {
                if !matches!(outcome, ParseOutcome::Rejected) || errors.is_empty() {
                    failures.push(format!(
                        "{stem}_{index}: syntax recovery admitted as a fragment"
                    ));
                }
            }
            TailPolicy::RejectedBullet(BulletBoundary::DecoderRefusal) => {
                if !errors
                    .to_vec()
                    .iter()
                    .any(|error| error.code.as_str() == "E360")
                {
                    failures.push(format!("{stem}_{index}: fragment bullet refusal lost"));
                }
                if let ParseOutcome::Parsed(main) = outcome {
                    assert!(
                        main.content.bullet.is_none(),
                        "decoder refusal invents no timing"
                    );
                }
            }
            TailPolicy::MissingTerminator => {}
        }
    }
}

#[test]
fn admitted_body_ending_control_owns_complete_token_edits() {
    let control = AdmittedControl::parse(CONTROL);
    for (field, spelling) in [
        (EndingField::Terminator, "."),
        (EndingField::FirstPostcode, "[+ fifth]"),
        (EndingField::Bullet, "\u{15}500_600\u{15}"),
    ] {
        let edit = control.ending_edit("hazel", field);
        let (owner, range, tier) = edit.into_parts();
        assert!(std::ptr::eq(owner, &control));
        assert_eq!(owner.source().get(range.clone()), Some(spelling));
        assert!(tier.start <= range.start && range.end <= tier.end);
    }
    let parser = TreeSitterParser::new().expect("parser");
    let native = parser
        .parse_source_incremental(CONTROL, None)
        .expect("native producer");
    let root = native.root().expect("source owner");
    for word in WORDS.into_iter().chain(["hazel"]) {
        let word = control.word(word);
        let expected = control
            .file()
            .utterances()
            .find(|utterance| {
                utterance.main.location.span().expect("located").start()
                    <= word.location.span().expect("located").start()
                    && word.location.span().expect("located").end()
                        <= utterance.main.location.span().expect("located").end()
            })
            .expect("word owner");
        let tier = root
            .descendants()
            .filter_map(|node| node.expect("range").typed::<MainTierNode>())
            .find(|node| {
                node.raw_node().start_byte()
                    == expected.main.location.span().expect("located").start()
            })
            .expect("admitted native main");
        let main = tier.extract();
        let SourceSlotView::Present(body) = main.field_child_5().slot().view() else {
            panic!("control has a positioned body");
        };
        let body = body.read().expect("same owner");
        let body = body.extract();
        assert!(matches!(
            body.field_content_2().slot().view(),
            SourceSlotView::Present(_)
        ));
        assert!(matches!(
            body.field_ending().slot().view(),
            SourceSlotView::Present(_)
        ));
        assert!(expected.main.content.terminator.is_some());
        assert!(!expected.main.content.postcodes.is_empty());
        assert!(expected.main.content.bullet.is_some());
    }
}

#[test]
fn body_marker_and_unclosed_replacement_specs_preserve_healthy_endings() {
    let control = AdmittedControl::parse(CONTROL);
    let mut failures = Vec::new();
    for (position, word) in WORDS.into_iter().enumerate() {
        for (index, replacement) in [
            (position + 2, "&".to_owned()),
            (position + 6, format!("{word} [: branch")),
        ] {
            let candidate = Candidate::word(&control, word, &replacement);
            let verdict = match index {
                6..=8 => Verdict::ParseInTier("E316"),
                9 => Verdict::ParseInTier("E342"),
                _ => Verdict::Parse("E316"),
            };
            check_candidate(
                &candidate,
                "E316_body_ending",
                index,
                verdict,
                TailPolicy::NativeCarrier,
                &mut failures,
            );
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn empty_replacements_and_unmatched_delimiters_keep_specific_routing() {
    let control = AdmittedControl::parse(CONTROL);
    let mut failures = Vec::new();
    for (position, word) in WORDS.into_iter().enumerate() {
        for (stem, code, replacement) in [
            ("E376_body_ending", "E376", format!("{word} [:]")),
            ("E375_body_ending", "E316", format!("{word} ]]]]")),
        ] {
            let candidate = Candidate::word(&control, word, &replacement);
            check_candidate(
                &candidate,
                stem,
                position + 2,
                Verdict::Parse(code),
                TailPolicy::NativeCarrier,
                &mut failures,
            );
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn malformed_ending_tokens_remain_source_owned_recovery() {
    let control = AdmittedControl::parse(CONTROL);
    let mut failures = Vec::new();
    for (index, field, replacement) in [
        (10, EndingField::Terminator, ". stray"),
        (11, EndingField::FirstPostcode, "[+ ]"),
        (12, EndingField::Bullet, "\u{15}500x600\u{15}"),
    ] {
        let candidate = Candidate::replace(control.ending_edit("hazel", field), replacement);
        check_candidate(
            &candidate,
            "E316_body_ending",
            index,
            Verdict::Parse("E316"),
            TailPolicy::Recovery,
            &mut failures,
        );
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn omitted_terminator_retains_ordered_codes_and_terminal_timing() {
    let control = AdmittedControl::parse(CONTROL);
    let candidate = Candidate::replace(control.ending_edit("hazel", EndingField::Terminator), "");
    let mut failures = Vec::new();
    check_candidate(
        &candidate,
        "E305_body_ending",
        2,
        Verdict::Validation("E305"),
        TailPolicy::MissingTerminator,
        &mut failures,
    );
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn rejected_terminal_bullets_preserve_other_ending_fields() {
    let control = AdmittedControl::parse(CONTROL);
    let mut failures = Vec::new();
    for (index, replacement) in [
        (2, "\u{15}500_600-\u{15}"),
        (3, "\u{15}500_18446744073709551616\u{15}"),
    ] {
        let candidate = Candidate::replace(
            control.ending_edit("hazel", EndingField::Bullet),
            replacement,
        );
        check_candidate(
            &candidate,
            "E360_body_ending",
            index,
            Verdict::Parse("E360"),
            TailPolicy::RejectedBullet(match index {
                2 => BulletBoundary::SyntaxRecovery,
                3 => BulletBoundary::DecoderRefusal,
                _ => unreachable!("authored bullet rows"),
            }),
            &mut failures,
        );
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
