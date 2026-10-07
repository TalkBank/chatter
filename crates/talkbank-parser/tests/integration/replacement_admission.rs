//! Replacement admission uses real spec-derived input at the public boundary.
use talkbank_model::ErrorCode;
use talkbank_model::WriteChat;
use talkbank_model::model::TranscriptName;
use talkbank_parser::{
    AdmittedDisposition, RemovalCause, RemovedTier, ReplacementTiers, TreeSitterParser,
    WordTimingPlan,
};

/// One line per removal receipt: domain, source span, recovery, cause and the
/// codes that cost the tier. Stable structured output for a snapshot.
fn receipt_inventory(removed: &[RemovedTier]) -> String {
    removed
        .iter()
        .map(|tier| {
            let (cause, diagnostics) = match tier.cause() {
                RemovalCause::Selected => ("Selected", &[][..]),
                RemovalCause::OwnLowering { diagnostics } => ("OwnLowering", &diagnostics[..]),
                RemovalCause::LocatedValidation { diagnostics } => {
                    ("LocatedValidation", &diagnostics[..])
                }
                RemovalCause::UnattributedValidation { diagnostics } => {
                    ("UnattributedValidation", &diagnostics[..])
                }
            };
            let codes: Vec<_> = diagnostics.iter().map(|d| d.code.as_str()).collect();
            format!(
                "{:?} {}..{} recovery={} {cause} {codes:?}",
                tier.tier(),
                tier.span().start,
                tier.span().end,
                tier.had_syntax_recovery(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The codes of the validation diagnostics bound to one removed tier.
fn located_codes(removed: &RemovedTier) -> Vec<ErrorCode> {
    match removed.cause() {
        RemovalCause::LocatedValidation { diagnostics } => {
            diagnostics.iter().map(|d| d.code).collect()
        }
        other => panic!("expected a located validation cause, found {other:?}"),
    }
}

#[test]
fn word_only_regeneration_carries_linkage_until_complete_validation() {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let reference = include_str!("../../../../corpus/reference/tiers/wor.cha");
    let word_only = reference
        .replace(" . \u{15}100_600\u{15}", " .")
        .replace(" ? \u{15}700_1200\u{15}", " ?")
        .replace(" . \u{15}565499_566219\u{15}", " .");
    let retained = parser
        .admit_word_timing_plan(&word_only, TranscriptName::Anonymous, |_| {
            WordTimingPlan::PreferRetained
        })
        .expect("valid word-only timing is retained");
    assert!(matches!(
        retained.into_disposition(),
        AdmittedDisposition::Preserved(_)
    ));

    // One word bullet the grammar can only recover: only that tier is
    // removed, and the timing the other two retain satisfies the linkage, so
    // the disposition is a complete replacement rather than regeneration.
    let one_corrupt = word_only.replacen("100_300", "invalid", 1);
    let admitted = parser
        .admit_word_timing_plan(&one_corrupt, TranscriptName::Anonymous, |_| {
            WordTimingPlan::PreferRetained
        })
        .expect("retained word timing keeps the document complete");
    assert_eq!(admitted.removed_tiers().len(), 1);
    assert!(matches!(
        admitted.into_disposition(),
        AdmittedDisposition::Replaced(_)
    ));

    // Every word tier faulty: the first recovers, the other two carry a
    // reversed bullet (E362). Removing all three discards the file's only
    // timing, so the result is pending regeneration.
    let corrupt =
        one_corrupt
            .replacen("700_800", "800_700", 1)
            .replacen("565499_566219", "566219_565499", 1);
    let admitted = parser
        .admit_word_timing_plan(&corrupt, TranscriptName::Anonymous, |_| {
            WordTimingPlan::PreferRetained
        })
        .expect("source timing permits pending regeneration");
    let AdmittedDisposition::Regenerating(pending) = admitted.into_disposition() else {
        panic!("removing the only timing cannot issue complete validity");
    };
    assert_eq!(pending.parsed_source().source(), corrupt);
    insta::assert_snapshot!(
        "every_word_tier_faulty_removal_receipts",
        receipt_inventory(pending.removed_tiers())
    );
    let [recovered, reversed @ ..] = pending.removed_tiers() else {
        panic!("every word tier is removed");
    };
    assert!(recovered.had_syntax_recovery());
    assert!(matches!(
        recovered.cause(),
        RemovalCause::OwnLowering { diagnostics } if !diagnostics.is_empty()
    ));
    assert_eq!(reversed.len(), 2);
    for tier in reversed {
        assert_eq!(located_codes(tier), [ErrorCode::TimestampBackwards]);
    }
    assert!(
        pending
            .document()
            .utterances()
            .all(|u| u.wor_tier().is_none())
    );
    assert_eq!(
        pending.document().media,
        parser
            .admit_word_timing_plan(reference, TranscriptName::Anonymous, |_| {
                WordTimingPlan::Preserve
            })
            .unwrap()
            .document()
            .media
    );
    assert!(!pending.pending_file().obligation().header_span().is_dummy());
    // The obligation is discharged only by the check that issued it, and only
    // against the document it travels with: the untimed working document hands
    // the payload back; once timing is written into that document, it
    // discharges and hands the document out.
    let mut pending_file = pending
        .into_pending_file()
        .discharge()
        .expect_err("the untimed working document still owes timing");
    let file = pending_file.document().clone();
    let timed = parser
        .admit_word_timing_plan(reference, TranscriptName::Anonymous, |_| {
            WordTimingPlan::Preserve
        })
        .expect("timed reference is admitted");
    // Regeneration writes timing into the working document; the timed
    // admission of the same reference stands in for it.
    *pending_file.document_mut() = timed.document().clone();
    assert!(
        (*pending_file).discharge().is_ok(),
        "a timed working document discharges its own obligation"
    );
    let failure = file
        .validate_construction_with_policy(
            talkbank_model::validation::ValidationPolicy::new(
                talkbank_model::RuleSelection::new(),
                talkbank_model::validation::AlignmentValidation::IncludeTierAlignment,
            ),
            &talkbank_model::NullErrorSink,
            TranscriptName::Anonymous,
        )
        .expect_err("pending model cannot become writable without timing");
    assert!(
        failure
            .diagnostics()
            .iter()
            .any(|e| e.code == talkbank_model::ErrorCode::MediaLinkageWithoutTiming)
    );
    assert!(
        parser
            .admit_word_timing_plan(&corrupt, TranscriptName::Anonymous, |_| {
                WordTimingPlan::Preserve
            })
            .is_err()
    );

    for fault in [
        corrupt.replace("hello world .", "<hello world ."),
        corrupt.replace("@End", "%mor:\tn|extra n|other .\n@End"),
        corrupt.replace("@End", "@Media:\tother, audio\n@End"),
    ] {
        assert!(
            parser
                .admit_word_timing_plan(&fault, TranscriptName::Anonymous, |_| {
                    WordTimingPlan::PreferRetained
                })
                .is_err(),
            "pending timing must not excuse another fault"
        );
    }
    // Fixed removal does not carry source timing obligations and remains strict.
    assert!(
        parser
            .admit_replacing_tiers(
                &word_only,
                ReplacementTiers::WordTiming,
                TranscriptName::Anonymous
            )
            .is_err()
    );
}

#[test]
fn adaptive_word_plan_retains_complete_partial_and_drifted_valid_timing() {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let reference = include_str!("../../../../corpus/reference/tiers/wor.cha");
    let partial = reference.replacen("hello \u{15}100_300\u{15}", "hello", 1);
    let drifted = include_str!("../../../../corpus/reference/tiers/wor-drift.cha");
    for input in [reference, partial.as_str(), drifted] {
        let calls = std::cell::Cell::new(0);
        let admitted = parser
            .admit_word_timing_plan(input, TranscriptName::Anonymous, |_| {
                calls.set(calls.get() + 1);
                WordTimingPlan::PreferRetained
            })
            .expect("valid timing must be retained, not only complete timing");
        assert_eq!(calls.get(), 1);
        assert_eq!(admitted.selection(), None);
        assert!(admitted.removed_tiers().is_empty());
        assert_eq!(admitted.document().to_chat_string(), input);
        assert!(matches!(
            admitted.into_disposition(),
            AdmittedDisposition::Preserved(_)
        ));
    }
}

#[test]
fn adaptive_word_plan_removes_only_the_recovered_word_tier() {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let reference = include_str!("../../../../corpus/reference/tiers/wor.cha");
    let input = reference.replacen("100_300", "invalid", 1);
    let admitted = parser
        .admit_word_timing_plan(&input, TranscriptName::Anonymous, |_| {
            WordTimingPlan::PreferRetained
        })
        .expect("word recovery is wholly inside one concrete tier");
    assert_eq!(admitted.selection(), Some(ReplacementTiers::WordTiming));
    let [removed] = admitted.removed_tiers() else {
        panic!("only the recovered tier is removed");
    };
    assert!(removed.had_syntax_recovery());
    assert!(input[removed.span().to_range()].starts_with("%wor:\thello \u{15}invalid"));
    assert!(matches!(removed.cause(), RemovalCause::OwnLowering { .. }));
    assert_eq!(
        admitted
            .document()
            .utterances()
            .filter(|u| u.wor_tier().is_some())
            .count(),
        2
    );
    assert!(
        admitted
            .document()
            .utterances()
            .all(|u| u.parse_health().permits_validation())
    );
    assert_eq!(admitted.parsed_source().source(), input);
    assert!(matches!(
        admitted.into_disposition(),
        AdmittedDisposition::Replaced(_)
    ));
    assert!(
        parser
            .admit_word_timing_plan(&input, TranscriptName::Anonymous, |_| {
                WordTimingPlan::Preserve
            })
            .is_err(),
        "preservation has no recovery exemption"
    );
}

#[test]
fn adaptive_word_plan_never_exempts_retained_source_faults() {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let reference = include_str!("../../../../corpus/reference/tiers/wor.cha");
    let word_recovery = reference.replacen("100_300", "invalid", 1);
    for input in [
        word_recovery.replace("hello world .", "<hello [/] world ."),
        word_recovery.replace("@End", "%mor:\tnoun|extra noun|other .\n@End"),
        word_recovery.replace("@End", "@Media:\tother, audio\n@End"),
    ] {
        assert!(
            parser
                .admit_word_timing_plan(&input, TranscriptName::Anonymous, |_| {
                    WordTimingPlan::PreferRetained
                })
                .is_err(),
            "the word plan must retain every unrelated fault"
        );
    }
}

/// A zero-duration `%wor` bullet is legal (leniency policy, Decision 10), so
/// neither plan removes or refuses its tier.
#[test]
fn a_zero_duration_word_interval_is_retained_by_both_plans() {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let reference = include_str!("../../../../corpus/reference/tiers/wor.cha");
    let input = reference.replacen("100_300", "100_100", 1);
    let retained = parser
        .admit_word_timing_plan(&input, TranscriptName::Anonymous, |_| {
            WordTimingPlan::PreferRetained
        })
        .expect("a zero-duration word interval is valid CHAT");
    assert_eq!(retained.selection(), None);
    assert!(retained.removed_tiers().is_empty());
    assert_eq!(retained.document().to_chat_string(), input);
    let preserved = parser
        .admit_word_timing_plan(&input, TranscriptName::Anonymous, |_| {
            WordTimingPlan::Preserve
        })
        .expect("preservation keeps a zero-duration word interval");
    assert_eq!(preserved.document().to_chat_string(), input);
}

/// A reversed `%wor` bullet is E362. The adaptive plan removes only the tier
/// that carries it and retains the other two timed tiers; preservation has no
/// exemption.
#[test]
fn a_reversed_word_interval_removes_only_its_own_tier() {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let reference = include_str!("../../../../corpus/reference/tiers/wor.cha");
    let input = reference.replacen("100_300", "300_100", 1);
    let admitted = parser
        .admit_word_timing_plan(&input, TranscriptName::Anonymous, |_| {
            WordTimingPlan::PreferRetained
        })
        .expect("the faulty word tier is removed, the rest retained");
    assert_eq!(admitted.selection(), Some(ReplacementTiers::WordTiming));
    let [removed] = admitted.removed_tiers() else {
        panic!("only the tier with the reversed bullet is removed");
    };
    assert!(input[removed.span().to_range()].starts_with("%wor:\thello \u{15}300_100"));
    assert_eq!(located_codes(removed), [ErrorCode::TimestampBackwards]);
    assert!(
        removed
            .cause()
            .diagnostics()
            .iter()
            .all(|d| removed.span().contains_span(d.location.span))
    );
    assert_eq!(
        admitted
            .document()
            .utterances()
            .filter(|u| u.wor_tier().is_some())
            .count(),
        2
    );
    let retained = admitted.document().to_chat_string();
    assert!(!retained.contains("%wor:\thello"));
    assert!(retained.contains("%wor:\thow \u{15}700_800\u{15}"));
    assert!(matches!(
        admitted.into_disposition(),
        AdmittedDisposition::Replaced(_)
    ));
    assert!(
        parser
            .admit_word_timing_plan(&input, TranscriptName::Anonymous, |_| {
                WordTimingPlan::Preserve
            })
            .is_err(),
        "preservation cannot exempt a reversed interval"
    );
}

/// A fault located outside every word tier is a retained fault: removing word
/// tiers never exempts it, even when a word tier was also at fault.
#[test]
fn a_fault_outside_any_word_tier_still_refuses() {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let reference = include_str!("../../../../corpus/reference/tiers/wor.cha");
    let reversed = reference.replacen("100_300", "300_100", 1);
    for input in [
        reversed.replace("@End", "%mor:\tnoun|extra noun|other .\n@End"),
        reversed.replace("@End", "@Media:\tother, audio\n@End"),
    ] {
        let failure = parser
            .admit_word_timing_plan(&input, TranscriptName::Anonymous, |_| {
                WordTimingPlan::PreferRetained
            })
            .expect_err("a retained fault refuses admission");
        assert!(!failure.has_internal_failure());
        assert!(
            failure
                .diagnostics()
                .iter()
                .all(|d| d.code != ErrorCode::TimestampBackwards),
            "the word tier at fault was removed before the refusal"
        );
    }
}

/// A duplicated `%wor` tier is E401 at the duplicate's own span: only the
/// duplicate is removed, and the original tier stays in its position.
#[test]
fn a_duplicate_word_tier_removes_only_the_duplicate() {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let reference = include_str!("../../../../corpus/reference/tiers/wor.cha");
    let tier = "%wor:\thello \u{15}100_300\u{15} world \u{15}300_600\u{15} .\n";
    let input = reference.replacen(tier, &format!("{tier}{tier}"), 1);
    let admitted = parser
        .admit_word_timing_plan(&input, TranscriptName::Anonymous, |_| {
            WordTimingPlan::PreferRetained
        })
        .expect("the duplicate is removed, the original retained");
    let [removed] = admitted.removed_tiers() else {
        panic!("only the duplicate is removed");
    };
    let second = input.find(tier).map(|first| first + tier.len());
    assert_eq!(Some(removed.span().start as usize), second);
    assert_eq!(located_codes(removed), [ErrorCode::DuplicateDependentTier]);
    assert_eq!(admitted.document().to_chat_string(), reference);
}

#[test]
fn adaptive_word_plan_preserves_dependent_tier_order() {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let reference = include_str!("../../../../corpus/reference/tiers/wor.cha");
    let input = reference
        .replacen("%wor:\thello", "%com:\tbefore timing\n%wor:\thello", 1)
        .replacen("*MOT:", "%exp:\tafter timing\n*MOT:", 1);
    let admitted = parser
        .admit_word_timing_plan(&input, TranscriptName::Anonymous, |_| {
            WordTimingPlan::PreferRetained
        })
        .expect("typed comment tiers remain in their original order");
    assert_eq!(admitted.document().to_chat_string(), input);
    assert_eq!(admitted.selection(), None);
}

#[test]
fn replacement_admission_removes_incomplete_morphology_before_retained_validation() {
    let input = include_str!(
        "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E305_mor_terminator_2.cha"
    );
    let parser = TreeSitterParser::new().expect("grammar loads");
    let admitted = parser
        .admit_replacing_tiers(
            input,
            ReplacementTiers::Morphosyntax,
            TranscriptName::Anonymous,
        )
        .expect("retained source is valid without the replaced morphology");
    assert_eq!(admitted.removed_tiers().len(), 1);
    assert!(
        admitted
            .document()
            .utterances()
            .all(|u| u.mor_tier().is_none())
    );
}

#[test]
fn replacement_admission_does_not_exempt_a_retained_morphology_tier() {
    let input = include_str!(
        "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E305_mor_terminator_2.cha"
    );
    let parser = TreeSitterParser::new().expect("grammar loads");
    assert!(
        parser
            .admit_replacing_tiers(
                input,
                ReplacementTiers::WordTiming,
                TranscriptName::Anonymous,
            )
            .is_err()
    );
}

#[test]
fn replacement_admission_removes_orphaned_grammar_without_changing_main_tier() {
    let input = include_str!(
        "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E604_gra_without_mor_1.cha"
    );
    let parser = TreeSitterParser::new().expect("grammar loads");
    let admitted = parser
        .admit_replacing_tiers(
            input,
            ReplacementTiers::Morphosyntax,
            TranscriptName::Anonymous,
        )
        .expect("orphaned grammar is actually removed");
    assert_eq!(admitted.removed_tiers().len(), 1);
    assert!(
        admitted
            .document()
            .utterances()
            .all(|u| u.gra_tier().is_none())
    );
    insta::assert_snapshot!(admitted.valid_file().to_chat_string(), @"
    @UTF8
    @Begin
    @Languages:\teng
    @Participants:\tCHI Target_Child
    @ID:\teng|corpus|CHI|||||Target_Child|||
    *CHI:\thello world .
    @End
    ");
}

#[test]
fn replacement_admission_preserves_retained_retrace_refusal() {
    let input = include_str!(
        "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E342_1.cha"
    );
    let parser = TreeSitterParser::new().expect("grammar loads");
    for selection in [ReplacementTiers::Morphosyntax, ReplacementTiers::WordTiming] {
        assert!(
            parser
                .admit_replacing_tiers(input, selection, TranscriptName::Anonymous)
                .is_err()
        );
    }
}

#[test]
fn replacement_admission_discards_only_source_bound_morphology_recovery() {
    let input = include_str!(
        "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E702_2.cha"
    );
    let parser = TreeSitterParser::new().expect("grammar loads");
    let admitted = parser
        .admit_replacing_tiers(
            input,
            ReplacementTiers::Morphosyntax,
            TranscriptName::Anonymous,
        )
        .expect("recovery is wholly inside the removed morphology tier");
    assert_eq!(admitted.removed_tiers().len(), 1);
    assert!(admitted.removed_tiers()[0].had_syntax_recovery());
    assert_eq!(admitted.parsed_source().source(), input);
    assert!(
        admitted
            .document()
            .utterances()
            .all(|u| u.parse_health().permits_validation())
    );
}

#[test]
fn replacement_admission_removes_word_timing_but_preserves_main_timing() {
    let input = include_str!("../../../../corpus/reference/tiers/wor.cha");
    let parser = TreeSitterParser::new().expect("grammar loads");
    let admitted = parser
        .admit_replacing_tiers(
            input,
            ReplacementTiers::WordTiming,
            TranscriptName::Anonymous,
        )
        .expect("retained main timing is valid");
    assert_eq!(admitted.removed_tiers().len(), 3);
    assert!(
        admitted
            .document()
            .utterances()
            .all(|u| u.wor_tier().is_none())
    );
    assert!(
        admitted
            .document()
            .utterances()
            .all(|u| u.main.content.bullet.is_some())
    );
}

#[test]
fn replacement_admission_preserves_header_refusal() {
    let input = include_str!(
        "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E501_2.cha"
    );
    let parser = TreeSitterParser::new().expect("grammar loads");
    for selection in [ReplacementTiers::Morphosyntax, ReplacementTiers::WordTiming] {
        let failure = parser
            .admit_replacing_tiers(input, selection, TranscriptName::Anonymous)
            .expect_err("duplicate retained media headers remain invalid");
        assert!(!failure.has_internal_failure());
        assert!(
            failure
                .diagnostics()
                .iter()
                .any(|d| d.code.as_str() == "E501")
        );
    }
}

#[test]
fn planned_admission_consumes_the_header_decision_once() {
    let input = include_str!(
        "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E305_mor_terminator_2.cha"
    );
    let parser = TreeSitterParser::new().expect("grammar loads");
    let calls = std::cell::Cell::new(0);
    let admitted = parser
        .admit_planned_tiers(input, TranscriptName::Anonymous, |headers| {
            calls.set(calls.get() + 1);
            assert!(
                headers
                    .iter()
                    .any(|h| matches!(h, talkbank_model::Header::Languages { .. }))
            );
            assert!(
                headers
                    .iter()
                    .any(|h| matches!(h, talkbank_model::Header::End))
            );
            Some(ReplacementTiers::Morphosyntax)
        })
        .expect("selected morphology is removed");
    assert_eq!(calls.get(), 1);
    assert_eq!(admitted.selection(), Some(ReplacementTiers::Morphosyntax));
    assert_eq!(admitted.parsed_source().source(), input);
    assert_eq!(admitted.removed_tiers().len(), 1);
    assert!(
        parser
            .admit_planned_tiers(input, TranscriptName::Anonymous, |_| None)
            .is_err()
    );
}

#[test]
fn planned_preservation_retains_the_complete_reference_document() {
    let input = include_str!("../../../../corpus/reference/tiers/wor.cha");
    let parser = TreeSitterParser::new().expect("grammar loads");
    let admitted = parser
        .admit_planned_tiers(input, TranscriptName::Anonymous, |_| None)
        .expect("whole reference is valid");
    assert_eq!(admitted.selection(), None);
    assert!(admitted.removed_tiers().is_empty());
    assert_eq!(
        admitted
            .document()
            .utterances()
            .filter(|u| u.wor_tier().is_some())
            .count(),
        3
    );
    assert_eq!(admitted.valid_file().to_chat_string(), input);
    let AdmittedDisposition::Preserved(preserved) = admitted.into_disposition() else {
        panic!("a preservation plan must retain original-source admission");
    };
    assert_eq!(preserved.source(), input);
    assert_eq!(preserved.document().to_chat_string(), input);
}

#[test]
fn selected_replacement_cannot_acquire_original_source_admission() {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let input = include_str!("../../../../corpus/reference/languages/eng-conversation.cha");
    // Selecting removal is distinct from certifying the original bytes, even
    // when this source has no tier matching the selected operation.
    let admitted = parser
        .admit_replacing_tiers(
            input,
            ReplacementTiers::WordTiming,
            TranscriptName::Anonymous,
        )
        .expect("retained document is valid");
    assert!(admitted.removed_tiers().is_empty());
    assert!(matches!(
        admitted.into_disposition(),
        AdmittedDisposition::Replaced(_)
    ));

    let input = include_str!("../../../../corpus/reference/tiers/wor.cha");
    let admitted = parser
        .admit_replacing_tiers(
            input,
            ReplacementTiers::WordTiming,
            TranscriptName::Anonymous,
        )
        .expect("retained document is valid");
    assert_eq!(admitted.removed_tiers().len(), 3);
    assert!(matches!(
        admitted.into_disposition(),
        AdmittedDisposition::Replaced(_)
    ));
}

#[test]
fn planned_removal_still_refuses_retained_recovery_and_header_faults() {
    let parser = TreeSitterParser::new().expect("grammar loads");
    for input in [
        include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E342_1.cha"
        ),
        include_str!(
            "../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E501_2.cha"
        ),
    ] {
        assert!(
            parser
                .admit_planned_tiers(input, TranscriptName::Anonymous, |_| {
                    Some(ReplacementTiers::Morphosyntax)
                })
                .is_err()
        );
    }
}
