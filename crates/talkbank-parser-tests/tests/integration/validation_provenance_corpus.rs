//! Parser, wire and recovery provenance through canonical alignment workflows.

use super::*;

/// Canonical typed payloads, not a serialization/reparse workaround, exercise
/// construction admission, invalidity, recovery refusal and wire authority loss.
#[test]
fn canonical_checked_construction_is_distinct_from_parser_provenance() {
    use talkbank_model::model::ParseHealthState;
    use talkbank_model::validation::{AlignmentValidation, ValidationPolicy};
    use talkbank_model::{NullErrorSink, RuleSelection};
    use talkbank_transform::splice::admit::{SkipReason, admit_edits};
    use talkbank_transform::splice::engine::{
        EditProvenance, EditTarget, Replacement, SpliceEdit, TransformName,
    };

    let parser = TreeSitterParser::new().expect("parser");
    let mut outcomes = [[0; 3]; 2];
    for root in [
        "corpus/reference",
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors",
    ] {
        let corpus = ChatCorpus::read(&workspace_root().join(root)).expect("canonical corpus");
        for fixture in corpus.fixtures() {
            let file = parser.parse_chat_file_streaming(fixture.source(), &ErrorCollector::new());
            if file.utterances().next().is_none() {
                continue;
            }
            for (policy_index, alignment) in [
                AlignmentValidation::Structure,
                AlignmentValidation::IncludeTierAlignment,
            ]
            .into_iter()
            .enumerate()
            {
                let policy = ValidationPolicy::new(RuleSelection::new(), alignment);
                if file.utterances().any(|u| !u.parse_health().is_clean()) {
                    let rejected = file
                        .clone()
                        .validate_construction_with_policy(
                            policy,
                            &NullErrorSink,
                            TranscriptName::Anonymous,
                        )
                        .expect_err("construction admission cannot erase recorded recovery");
                    assert!(rejected.has_incomplete_parse() || rejected.has_internal_failure());
                    outcomes[policy_index][2] += 1;
                    continue;
                }
                let expected = file.clone().validate_with_policy(
                    policy,
                    &NullErrorSink,
                    TranscriptName::Anonymous,
                );
                let mut assembled = file.clone();
                for line in &mut assembled.lines {
                    if let talkbank_model::model::Line::Utterance(utterance) = line {
                        // Exercise the actual construction API, not just a trust
                        // flag reset. Retain authored tier separators and headers.
                        let mut rebuilt = talkbank_model::Utterance::new(utterance.main.clone());
                        rebuilt.dependent_tiers = utterance.dependent_tiers.clone();
                        rebuilt.preceding_headers = utterance.preceding_headers.clone();
                        **utterance = rebuilt;
                    }
                }
                match assembled.validate_construction_with_policy(
                    policy,
                    &NullErrorSink,
                    TranscriptName::Anonymous,
                ) {
                    Ok(proof) => {
                        let parsed_proof = expected
                            .expect("construction must run the same rules as parsed admission");
                        for utterance in proof.document().utterances() {
                            assert_eq!(utterance.parse_health(), ParseHealthState::Constructed);
                            assert!(!utterance.parse_health().is_clean());
                            assert!(utterance.parse_health().can_align_main_to_mor());
                            let edit = SpliceEdit::new(
                                EditTarget::InsertAt(utterance.main.span.start),
                                Replacement::new("x"),
                                EditProvenance::Transform(TransformName::new(
                                    "construction-boundary",
                                )),
                            );
                            let admission = admit_edits(proof.document(), vec![edit]);
                            assert!(
                                admission.admitted.is_empty(),
                                "construction cannot certify inherited source spans"
                            );
                            assert_eq!(admission.skipped[0].reason, SkipReason::UnknownHealth);
                            let mut changed = utterance.clone();
                            changed.mark_parse_taint(talkbank_model::model::ParseHealthTier::Main);
                            assert!(changed.parse_health().is_unknown());
                            let mut changed = utterance.clone();
                            changed.mark_all_dependent_alignment_taint();
                            assert!(changed.parse_health().is_unknown());
                            assert!(!changed.parse_health().can_align_main_to_mor());
                            if let Some(dependent) = utterance.dependent_tiers.first() {
                                let appended =
                                    utterance.clone().add_dependent_tier(dependent.tier.clone());
                                assert!(appended.parse_health().is_unknown());
                                assert!(appended.alignments.is_none());
                                assert!(appended.alignment_diagnostics.is_empty());
                            }
                        }
                        let wire = serde_json::to_string(&proof).expect("wire");
                        assert_eq!(
                            wire,
                            serde_json::to_string(&parsed_proof).expect("wire"),
                            "same validation must produce identical JSON metadata"
                        );
                        let imported: ChatFile = serde_json::from_str(&wire).expect("wire import");
                        assert!(imported.utterances().all(|u| u.parse_health().is_unknown()));
                        let editable = proof.into_unchecked();
                        assert!(editable.utterances().all(|u| u.parse_health().is_unknown()));
                        assert!(editable.semantic_eq(&file));
                        outcomes[policy_index][0] += 1;
                    }
                    Err(failure) => {
                        assert!(
                            expected.is_err(),
                            "valid canonical structure was refused: {}",
                            fixture.path().display()
                        );
                        assert!(
                            !failure.has_incomplete_parse(),
                            "construction must actually check invalid structure"
                        );
                        assert!(
                            failure
                                .document()
                                .utterances()
                                .all(|u| u.parse_health().is_unknown())
                        );
                        outcomes[policy_index][1] += 1;
                    }
                }
            }
        }
    }
    assert!(
        outcomes.into_iter().flatten().all(|count| count > 0),
        "each policy must exercise success, invalidity and recovery"
    );
    assert!(
        outcomes[0][0] > outcomes[1][0],
        "canonical inputs must witness the additional alignment policy rejecting structure-only successes"
    );
    eprintln!(
        "checked construction outcomes by policy (accepted, invalid, recovered): {outcomes:?}"
    );
}

#[test]
fn recovered_spec_reassembly_cannot_establish_clean_parse_provenance() {
    let source = std::fs::read_to_string(workspace_root().join(
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E258_recovery_2.cha",
    ))
    .expect("authored mixed-recovery spec");
    let parser = TreeSitterParser::new().expect("parser");
    let errors = ErrorCollector::new();
    let file = parser.parse_chat_file_streaming(&source, &errors);
    let original = file.utterances().next().expect("recovered utterance");
    assert!(
        !original.parse_health().is_clean(),
        "fixture must require recovery"
    );
    let assembled = original.dependent_tiers.iter().fold(
        talkbank_model::Utterance::new(original.main.clone()),
        |draft, dependent| draft.add_dependent_tier(dependent.tier.clone()),
    );
    assert!(assembled.main.semantic_eq(&original.main));
    assert!(
        assembled.parse_health().is_unknown(),
        "ordinary assembly cannot establish parser-backed provenance"
    );
}

/// Recovery scope is explicit: dependent recovery must leave main-tier trust
/// alone, while whole-utterance recovery withdraws both alignment sides.
#[derive(Clone, Copy, Debug)]
enum RecoveryScope {
    Tier(talkbank_model::model::ParseHealthTier),
    Dependents,
    WholeUtterance,
}

impl RecoveryScope {
    fn apply(self, utterance: &mut talkbank_model::Utterance) {
        use talkbank_model::model::ParseHealthTier::*;
        match self {
            Self::Tier(tier) => utterance.mark_parse_taint(tier),
            Self::Dependents | Self::WholeUtterance => {
                for tier in [
                    Mor, Gra, Pho, Mod, Wor, Sin, Modsyl, Phosyl, Phoaln, Xphoint,
                ] {
                    utterance.mark_parse_taint(tier);
                }
                if matches!(self, Self::WholeUtterance) {
                    utterance.mark_parse_taint(Main);
                }
            }
        }
    }
}

#[test]
fn canonical_validation_preserves_header_scope_and_wire_provenance() {
    use ParseHealthTier::{Gra, Main, Mod, Modsyl, Mor, Pho, Phoaln, Phosyl, Sin, Wor, Xphoint};
    use talkbank_model::model::{ParseHealthState, ParseHealthTier};
    // Independently stated dependency policy: alignment requires explicit
    // clean provenance for every participating tier, not merely absence of a
    // taint flag. JSON-imported Unknown therefore refuses every gate.
    type HealthGate = (fn(ParseHealthState) -> bool, &'static [ParseHealthTier]);
    let gates: [HealthGate; 10] = [
        (ParseHealthState::can_align_main_to_mor, &[Main, Mor]),
        (ParseHealthState::can_align_mor_to_gra, &[Mor, Gra]),
        (ParseHealthState::can_align_main_to_pho, &[Main, Pho]),
        (
            ParseHealthState::can_resolve_wor_timing_sidecar,
            &[Main, Wor],
        ),
        (ParseHealthState::can_align_main_to_mod, &[Main, Mod]),
        (ParseHealthState::can_align_main_to_sin, &[Main, Sin]),
        (ParseHealthState::can_align_modsyl_to_mod, &[Modsyl, Mod]),
        (ParseHealthState::can_align_phosyl_to_pho, &[Phosyl, Pho]),
        (ParseHealthState::can_align_phoaln, &[Phoaln, Mod, Pho]),
        (ParseHealthState::can_align_xphoint_to_pho, &[Xphoint, Pho]),
    ];
    let mut health_states = [false; 3];
    let mut check_health = |state: ParseHealthState| {
        let index = match state {
            ParseHealthState::Unknown => 0,
            ParseHealthState::Clean => 1,
            ParseHealthState::Tainted(_) => 2,
            ParseHealthState::Constructed => {
                panic!("parser/JSON workflow cannot construct admission")
            }
        };
        health_states[index] = true;
        // Recovery can withdraw trust, never manufacture it for other tiers.
        // In particular, JSON import supplies Unknown provenance; marking a
        // damaged tier cannot certify the rest of that imported utterance.
        for tier in [
            Main, Mor, Gra, Pho, Mod, Wor, Sin, Modsyl, Phosyl, Phoaln, Xphoint,
        ] {
            let mut tainted = state;
            tainted.taint(tier);
            if state.is_unknown() {
                assert!(
                    tainted.is_unknown(),
                    "taint cannot establish parse provenance"
                );
            }
            assert!(!tainted.is_tier_clean(tier));
            for (gate, _) in gates {
                assert!(
                    !gate(tainted) || gate(state),
                    "taint enabled alignment: {state:?}"
                );
            }
        }
        let mut dependent_taint = state;
        dependent_taint.taint_all_alignment_dependents();
        if state.is_unknown() {
            assert!(dependent_taint.is_unknown());
        }
        for (gate, _) in gates {
            assert!(
                !gate(dependent_taint),
                "dependent recovery must block alignment"
            );
        }
        for (gate, required) in gates {
            let expected =
                !state.is_unknown() && required.iter().all(|tier| !state.is_tier_tainted(*tier));
            assert_eq!(gate(state), expected, "{state:?}: {required:?}");
            for tier in required {
                assert_eq!(
                    state.is_tier_clean(*tier),
                    !state.is_unknown() && !state.is_tier_tainted(*tier)
                );
            }
        }
    };
    let parser = TreeSitterParser::new().expect("parser");
    let mut alignment_errors = 0;
    let mut provenance_warnings = 0;
    let mut header_diagnostics = 0;
    let mut metadata_warnings = [0; 8];
    let mut cached_pairs = 0;
    let mut tainted_metadata = [0; 8];
    let mut broad_recovery_metadata = [[0; 8]; 2];
    for root in [
        "corpus/reference",
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors",
    ] {
        let corpus = ChatCorpus::read(&workspace_root().join(root)).expect("canonical corpus");
        for fixture in corpus.fixtures() {
            // Retain the real recovery model too; taint must suppress checks
            // whose input is incomplete, not turn partial output into proof.
            let parse_errors = ErrorCollector::new();
            let file = parser.parse_chat_file_streaming(fixture.source(), &parse_errors);
            let headers = ErrorCollector::new();
            let context = file.validate_headers_only(&headers, TranscriptName::Anonymous);
            let full = ErrorCollector::new();
            file.validate(&full, TranscriptName::Anonymous);
            let header_errors = headers.to_vec();
            assert!(
                full.to_vec().starts_with(&header_errors),
                "header-only checks must be full validation's initial phase: {}",
                fixture.path().display()
            );
            header_diagnostics += header_errors.len();
            let through_trait = ErrorCollector::new();
            Validate::validate(&file, &context, &through_trait);
            assert_eq!(
                full.to_vec(),
                through_trait.to_vec(),
                "anonymous trait validation: {}",
                fixture.path().display()
            );

            assert!(
                file.utterances()
                    .all(|utterance| !utterance.parse_health().is_unknown()),
                "parser must establish provenance: {}",
                fixture.path().display()
            );
            let parsed_alignment = file.validate_alignments();
            for utterance in file.utterances() {
                check_health(utterance.parse_health());
            }
            assert!(
                parsed_alignment
                    .iter()
                    .all(|error| error.severity == Severity::Error),
                "parsed alignment must not report unknown provenance: {}",
                fixture.path().display()
            );
            alignment_errors += parsed_alignment.len();

            let json = serde_json::to_string(&file).expect("serialize parsed corpus model");
            let decoded: ChatFile = serde_json::from_str(&json).expect("decode corpus wire model");
            assert!(
                file.semantic_eq(&decoded),
                "wire semantics: {}",
                fixture.path().display()
            );
            let mut expected_warnings = 0;
            for utterance in decoded.utterances() {
                check_health(utterance.parse_health());
                assert!(
                    utterance.parse_health().is_unknown(),
                    "wire data cannot certify parser provenance"
                );
                expected_warnings += usize::from(utterance.mor_tier().is_some());
                expected_warnings +=
                    usize::from(utterance.mor_tier().is_some() && utterance.gra_tier().is_some());
                expected_warnings += usize::from(utterance.pho_tier().is_some());
                expected_warnings += usize::from(utterance.sin_tier().is_some());
            }
            let warnings = decoded.validate_alignments();
            assert_eq!(
                warnings.len(),
                expected_warnings,
                "wire alignment: {}",
                fixture.path().display()
            );
            assert!(
                warnings
                    .iter()
                    .all(|warning| warning.code == ErrorCode::TierValidationError
                        && warning.severity == Severity::Warning),
                "unknown wire provenance must not become alignment error/proof: {}",
                fixture.path().display()
            );
            provenance_warnings += warnings.len();
            // Exercise the derived-metadata producer, not only the separate
            // validation facade. Recomputed wire alignments must not reuse
            // parser-origin evidence which JSON deliberately cannot preserve.
            let wire_context =
                decoded.validate_headers_only(&ErrorCollector::new(), TranscriptName::Anonymous);
            for utterance in decoded.utterances() {
                let mut computed = utterance.clone();
                computed.compute_alignments(&wire_context);
                assert!(computed.parse_health().is_unknown());
                assert!(
                    computed.semantic_eq(utterance),
                    "metadata computation cannot edit content"
                );
                let metadata = computed
                    .alignments
                    .as_ref()
                    .expect("alignment metadata produced");
                for (total, count) in metadata_warnings
                    .iter_mut()
                    .zip(unknown_metadata(&computed))
                {
                    *total += count;
                }
                assert!(
                    metadata.wor_timings.is_none(),
                    "unknown provenance cannot produce a timing-sidecar binding"
                );
                assert_eq!(
                    metadata
                        .collect_errors()
                        .into_iter()
                        .cloned()
                        .collect::<Vec<_>>(),
                    computed.alignment_diagnostics
                );
                assert_eq!(
                    metadata.is_error_free(),
                    computed.alignment_diagnostics.is_empty()
                );
                let first = metadata.clone();
                let diagnostics = computed.alignment_diagnostics.clone();
                computed.compute_alignments(&wire_context);
                assert_eq!(
                    computed.alignments.as_ref(),
                    Some(&first),
                    "recomputation is stable"
                );
                assert_eq!(
                    computed.alignment_diagnostics, diagnostics,
                    "warnings must not accumulate on recomputation"
                );
            }
            for utterance in file.utterances() {
                let mut computed = utterance.clone();
                computed.compute_alignments(&context);
                assert_eq!(
                    computed.parse_health(),
                    utterance.parse_health(),
                    "metadata cannot promote parser recovery"
                );
                assert!(computed.semantic_eq(utterance));
                let metadata = computed
                    .alignments
                    .as_ref()
                    .expect("parsed metadata produced");
                assert_eq!(
                    metadata
                        .collect_errors()
                        .into_iter()
                        .cloned()
                        .collect::<Vec<_>>(),
                    computed.alignment_diagnostics
                );
                assert_eq!(
                    metadata.is_error_free(),
                    computed.alignment_diagnostics.is_empty()
                );
                let first = metadata.clone();
                let diagnostics = computed.alignment_diagnostics.clone();
                computed.compute_alignments(&context);
                assert_eq!(computed.alignments.as_ref(), Some(&first));
                assert_eq!(computed.alignment_diagnostics, diagnostics);
                if computed.parse_health() == ParseHealthState::Clean {
                    for scope in [
                        Main, Mor, Gra, Pho, Mod, Wor, Sin, Modsyl, Phosyl, Phoaln, Xphoint,
                    ]
                    .into_iter()
                    .map(RecoveryScope::Tier)
                    .chain([RecoveryScope::Dependents, RecoveryScope::WholeUtterance])
                    {
                        let mut tainted = computed.clone();
                        scope.apply(&mut tainted);
                        let health = tainted.parse_health();
                        if matches!(scope, RecoveryScope::Dependents) {
                            assert!(health.is_tier_clean(Main));
                        }
                        if matches!(scope, RecoveryScope::WholeUtterance) {
                            assert!(!health.is_tier_clean(Main));
                        }
                        tainted.compute_alignments(&context);
                        assert_eq!(tainted.parse_health(), health);
                        assert!(tainted.semantic_eq(&computed), "taint cannot edit content");
                        let after = tainted.alignments.as_ref().expect("tainted metadata");
                        // Reuse the independent dependency table above: the
                        // metadata producer must obey the same trust boundary.
                        let counts = [
                            tainted_alignment(
                                first.mor.as_ref(),
                                after.mor.as_ref(),
                                gates[0].0(health),
                            ),
                            tainted_alignment(
                                first.gra.as_ref(),
                                after.gra.as_ref(),
                                gates[1].0(health),
                            ),
                            tainted_alignment(
                                first.pho.as_ref(),
                                after.pho.as_ref(),
                                gates[2].0(health),
                            ),
                            tainted_alignment(
                                first.mod_.as_ref(),
                                after.mod_.as_ref(),
                                gates[4].0(health),
                            ),
                            tainted_alignment(
                                first.sin.as_ref(),
                                after.sin.as_ref(),
                                gates[5].0(health),
                            ),
                            tainted_alignment(
                                first.modsyl.as_ref(),
                                after.modsyl.as_ref(),
                                gates[6].0(health),
                            ),
                            tainted_alignment(
                                first.phosyl.as_ref(),
                                after.phosyl.as_ref(),
                                gates[7].0(health),
                            ),
                            tainted_alignment(
                                first.phoaln.as_ref(),
                                after.phoaln.as_ref(),
                                gates[8].0(health),
                            ),
                        ];
                        for (total, count) in tainted_metadata.iter_mut().zip(counts) {
                            *total += count;
                        }
                        let broad = match scope {
                            RecoveryScope::Tier(_) => None,
                            RecoveryScope::Dependents => Some(0),
                            RecoveryScope::WholeUtterance => Some(1),
                        };
                        if let Some(index) = broad {
                            for (total, count) in
                                broad_recovery_metadata[index].iter_mut().zip(counts)
                            {
                                *total += count;
                            }
                            assert!(gates.iter().all(|(gate, _)| !gate(health)));
                        }
                        if matches!(scope, RecoveryScope::WholeUtterance) {
                            assert!(
                                tainted.alignment_diagnostics.iter().all(|warning| {
                                    warning.message.contains(" and ")
                                        && !warning.message.contains("internal parse-health gate")
                                }),
                                "whole-utterance recovery must identify both damaged sides"
                            );
                        }
                        if gates[3].0(health) {
                            assert_eq!(after.wor_timings, first.wor_timings);
                        } else {
                            assert!(after.wor_timings.is_none());
                        }
                        let metadata = after.clone();
                        let diagnostics = tainted.alignment_diagnostics.clone();
                        tainted.compute_alignments(&context);
                        assert_eq!(tainted.alignments.as_ref(), Some(&metadata));
                        assert_eq!(tainted.alignment_diagnostics, diagnostics);
                    }
                }
                // A wire model may already contain serialized alignment pairs.
                // Their presence cannot restore the parser's provenance proof.
                let mut cached: talkbank_model::Utterance = serde_json::from_str(
                    &serde_json::to_string(&computed).expect("serialize computed metadata"),
                )
                .expect("decode cached metadata");
                assert!(cached.parse_health().is_unknown());
                assert!(cached.semantic_eq(&computed));
                let held = cached
                    .alignments
                    .as_ref()
                    .expect("wire retains legacy alignment payload");
                let pair_count = held.mor.as_ref().map_or(0, |a| a.pairs.len())
                    + held.gra.as_ref().map_or(0, |a| a.pairs.len())
                    + held.pho.as_ref().map_or(0, |a| a.pairs.len())
                    + held.mod_.as_ref().map_or(0, |a| a.pairs.len())
                    + held.sin.as_ref().map_or(0, |a| a.pairs.len())
                    + held.modsyl.as_ref().map_or(0, |a| a.pairs.len())
                    + held.phosyl.as_ref().map_or(0, |a| a.pairs.len())
                    + held.phoaln.as_ref().map_or(0, |a| a.pairs.len());
                cached_pairs += pair_count;
                cached.compute_alignments(&context);
                unknown_metadata(&cached);
                assert!(
                    cached
                        .alignments
                        .as_ref()
                        .expect("recomputed metadata")
                        .wor_timings
                        .is_none()
                );
                assert!(
                    cached.semantic_eq(&computed),
                    "discarding cached trust cannot edit content"
                );
            }
        }
    }
    assert_eq!(
        health_states, [true; 3],
        "canonical parser and wire paths must witness Unknown, Clean and Tainted"
    );
    assert!(
        alignment_errors > 0,
        "specs must witness diagnosed alignment faults"
    );
    assert!(
        provenance_warnings > 0,
        "wire models must witness provenance warnings"
    );
    assert!(
        header_diagnostics > 0,
        "specs must witness header diagnostics"
    );
    assert!(
        metadata_warnings.iter().all(|count| *count > 0),
        "every structural metadata family needs blocked-provenance witnesses: {metadata_warnings:?}"
    );
    assert!(
        cached_pairs > 0,
        "serialized real alignment pairs must witness cache invalidation"
    );
    eprintln!("recovery-withdrawal witnesses by metadata family: {tainted_metadata:?}");
    assert!(
        tainted_metadata.iter().all(|count| *count > 0),
        "all structural metadata families need recovery-withdrawal witnesses: {tainted_metadata:?}"
    );
    assert!(
        broad_recovery_metadata
            .iter()
            .flatten()
            .all(|count| *count > 0),
        "both broad recovery scopes need all eight metadata families: {broad_recovery_metadata:?}"
    );
    eprintln!("broad recovery witnesses: {broad_recovery_metadata:?}");
}

fn tainted_alignment<
    T: talkbank_model::alignment::TierAlignmentResult + PartialEq + std::fmt::Debug,
>(
    before: Option<&T>,
    after: Option<&T>,
    allowed: bool,
) -> usize {
    assert_eq!(
        before.is_some(),
        after.is_some(),
        "taint preserves tier presence"
    );
    if allowed {
        assert_eq!(before, after, "unrelated recovery cannot change alignment");
        return 0;
    }
    let Some(after) = after else { return 0 };
    assert!(
        after.pairs().is_empty(),
        "recovery must withdraw trusted pairs"
    );
    assert_eq!(
        after.errors().len(),
        1,
        "one recovery warning replaces cached errors"
    );
    let warning = &after.errors()[0];
    assert_eq!(warning.code, ErrorCode::TierValidationError);
    assert_eq!(warning.severity, Severity::Warning);
    assert!(warning.message.contains("had parse errors during recovery"));
    1
}

fn unknown_metadata(utterance: &talkbank_model::Utterance) -> [usize; 8] {
    assert!(utterance.parse_health().is_unknown());
    let metadata = utterance.alignments.as_ref().expect("computed metadata");
    [
        unknown_alignment(metadata.mor.as_ref(), utterance.mor_tier().is_some()),
        unknown_alignment(
            metadata.gra.as_ref(),
            utterance.mor_tier().is_some() && utterance.gra_tier().is_some(),
        ),
        unknown_alignment(metadata.pho.as_ref(), utterance.pho_tier().is_some()),
        unknown_alignment(metadata.mod_.as_ref(), utterance.mod_tier().is_some()),
        unknown_alignment(metadata.sin.as_ref(), utterance.sin_tier().is_some()),
        unknown_alignment(
            metadata.modsyl.as_ref(),
            utterance.modsyl_tier().is_some() && utterance.mod_tier().is_some(),
        ),
        unknown_alignment(
            metadata.phosyl.as_ref(),
            utterance.phosyl_tier().is_some() && utterance.pho_tier().is_some(),
        ),
        unknown_alignment(metadata.phoaln.as_ref(), utterance.phoaln_tier().is_some()),
    ]
}

fn unknown_alignment<T: talkbank_model::alignment::TierAlignmentResult>(
    alignment: Option<&T>,
    expected: bool,
) -> usize {
    assert_eq!(
        alignment.is_some(),
        expected,
        "tier presence determines whether alignment is attempted"
    );
    let Some(alignment) = alignment else {
        return 0;
    };
    assert!(
        alignment.pairs().is_empty(),
        "unknown provenance cannot produce trusted index pairs"
    );
    assert_eq!(
        alignment.errors().len(),
        1,
        "one explicit provenance warning per blocked alignment"
    );
    let warning = &alignment.errors()[0];
    assert_eq!(warning.code, ErrorCode::TierValidationError);
    assert_eq!(warning.severity, Severity::Warning);
    assert!(warning.message.contains("provenance is unknown"));
    1
}
