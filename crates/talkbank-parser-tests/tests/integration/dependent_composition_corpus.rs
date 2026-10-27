//! Finite native/document/fragment recovery policies composed with intact tiers.
//! The spec owns every specimen; private edit capabilities bind its admitted
//! control, tier identity and checked body range. These are observable policies,
//! not tests of construction invariants or fabricated source/CST pairs.
#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::model::{DependentTier, DependentTierEntry, ParseHealthTier};
use talkbank_model::{
    ChatFile, ChatParser, ErrorCode, ErrorCollector, ParseOutcome, SemanticEq, Span,
    TranscriptName, Utterance,
};
use talkbank_parser::TreeSitterParser;
use talkbank_parser::generated_traversal::{
    AsRawNode, ComDependentTierNode, EngDependentTierNode, GraDependentTierNode, MainTierNode,
    MorDependentTierNode, XDependentTierNode,
};

const CONTROL: &str = include_str!("../../../../corpus/reference/tiers/dependent-composition.cha");
const MIDDLE: usize = 1;
const TURN_COUNT: usize = 3;
const TIERS_PER_TURN: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tier {
    Mor,
    Gra,
    Com,
    Eng,
    Xnote,
}

impl Tier {
    fn matches(self, entry: &DependentTierEntry) -> bool {
        match self {
            Self::Mor => matches!(
                entry.tier,
                DependentTier::Mor(_) | DependentTier::UnparsedMor(_)
            ),
            Self::Gra => matches!(
                entry.tier,
                DependentTier::Gra(_) | DependentTier::UnparsedGra(_)
            ),
            Self::Com => matches!(entry.tier, DependentTier::Com(_)),
            Self::Eng => matches!(entry.tier, DependentTier::Eng(_)),
            Self::Xnote => {
                matches!(&entry.tier, DependentTier::UserDefined(t) if t.label.as_str() == "xnote")
            }
        }
    }
}

/// The child module alone issues edits; parents cannot construct a body range
/// or candidate independently of the successfully admitted source and model.
#[path = "dependent_composition_admission.rs"]
mod admitted;

#[derive(Clone, Copy, Debug)]
enum Policy {
    Mor,
    GraIndexDeletion,
    GraError,
    GraLexical(ErrorCode),
    TextRecovery,
    EmptyText,
}

struct Case {
    tier: Tier,
    replacement: &'static str,
    fixture: &'static str,
    policy: Policy,
}

macro_rules! specimen {
    ($index:literal) => {
        include_str!(concat!(
            "../error_corpus/validation_errors/E330_dependent_composition_",
            $index,
            ".cha"
        ))
    };
}

fn entry(utterance: &Utterance, tier: Tier) -> Option<&DependentTierEntry> {
    utterance
        .dependent_tiers
        .iter()
        .find(|entry| tier.matches(entry))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Carrier {
    Healthy,
    Recovery,
}

struct NativeObservation {
    carrier: Carrier,
    healthy_tiers: Vec<Span>,
}

/// Inspect the actual source-bound native carrier before making retention claims.
/// No tier/source pair, reconstructed CST or independently chosen range enters.
fn native_contract(
    candidate: &admitted::Candidate<'_>,
    case: &Case,
    failures: &mut Vec<String>,
) -> NativeObservation {
    let parser = TreeSitterParser::new().expect("parser");
    let parsed = parser
        .parse_source_incremental(candidate.source(), None)
        .expect("native owner");
    let root = parsed.root().expect("native root");
    let span = candidate.span();
    let selected = root
        .descendants()
        .map(|node| node.expect("bound descendant"))
        .find(|node| {
            node.raw_node().start_byte() == span.start() && node.raw_node().end_byte() == span.end()
        })
        .expect("selected native tier has its own range");
    let raw = selected.raw_node();
    let carrier = if raw.has_error() {
        Carrier::Recovery
    } else {
        Carrier::Healthy
    };
    let errors = ErrorCollector::new();
    match case.tier {
        Tier::Mor => {
            let node = selected
                .typed::<MorDependentTierNode>()
                .expect("native MOR identity");
            // A recovery carrier never enters the typed MOR parser: admission
            // refuses it by type, so ERROR-owned lexical material cannot become
            // an item. Admission is the one observation here (it reads the
            // same flag `carrier` did); the document path reports the tier
            // and substitutes the placeholder, which the whole-file checks
            // below observe.
            if !matches!(
                talkbank_parser::api::tiers::TierAdmission::admit(node),
                talkbank_parser::api::tiers::TierAdmission::Recovery(_)
            ) {
                failures.push("MOR mutation must be refused by tier admission".into());
            }
        }
        Tier::Gra => {
            let node = selected
                .typed::<GraDependentTierNode>()
                .expect("native GRA identity");
            let actual = talkbank_parser::api::tiers::parse_gra_tier(node, &errors)
                .expect("native root range admission");
            if !matches!(actual, talkbank_model::ParseOutcome::Rejected) {
                failures.push("rejected native GRA must not expose a partial graph".into());
            }
            let expected_code = match case.policy {
                Policy::GraLexical(code) => {
                    assert_eq!(carrier, Carrier::Healthy);
                    code
                }
                Policy::GraError => {
                    assert_eq!(carrier, Carrier::Recovery);
                    ErrorCode::UnparsableContent
                }
                Policy::GraIndexDeletion => {
                    assert_eq!(carrier, Carrier::Recovery);
                    ErrorCode::MalformedGrammarRelation
                }
                Policy::Mor | Policy::TextRecovery | Policy::EmptyText => panic!("non-GRA policy"),
            };
            if !errors
                .to_vec()
                .iter()
                .any(|error| error.code == expected_code)
            {
                failures.push(format!(
                    "missing native GRA rejection diagnostic {expected_code:?}: {:?}",
                    errors.to_vec()
                ));
            }
        }
        Tier::Com => {
            selected
                .typed::<ComDependentTierNode>()
                .expect("native comment identity");
        }
        Tier::Eng => {
            selected
                .typed::<EngDependentTierNode>()
                .expect("native translation identity");
        }
        Tier::Xnote => {
            selected
                .typed::<XDependentTierNode>()
                .expect("native x identity");
        }
    }
    if matches!(case.policy, Policy::TextRecovery) && carrier != Carrier::Recovery {
        failures.push("bare bullet must retain native body recovery".into());
    }
    if matches!(case.policy, Policy::EmptyText) && carrier != Carrier::Healthy {
        failures.push("optional empty content is not native recovery".into());
    }
    // Every unchanged main tier has an independent healthy native owner.
    for expected in candidate.control().file().utterances() {
        let expected_span = candidate.translated(expected.main.location);
        if !root.descendants().any(|node| {
            let node = node.expect("main descendant");
            node.typed::<MainTierNode>().is_some_and(|main| {
                !main.raw_node().has_error()
                    && main.raw_node().byte_range() == (expected_span.start()..expected_span.end())
            })
        }) {
            failures.push(format!(
                "unchanged main carrier is not healthy: {expected_span:?}"
            ));
        }
    }
    let healthy_tiers = root
        .descendants()
        .filter_map(|node| {
            let node = node.expect("healthy tier descendant");
            let raw = node.raw_node();
            (!raw.has_error()
                && (node.typed::<MorDependentTierNode>().is_some()
                    || node.typed::<GraDependentTierNode>().is_some()
                    || node.typed::<ComDependentTierNode>().is_some()
                    || node.typed::<EngDependentTierNode>().is_some()
                    || node.typed::<XDependentTierNode>().is_some()))
            .then(|| Span::new(raw.start_byte(), raw.end_byte()))
        })
        .collect();
    NativeObservation {
        carrier,
        healthy_tiers,
    }
}

fn observe(control: &admitted::Control, case: Case, failures: &mut Vec<String>) {
    let candidate = control
        .edit(case.tier)
        .replace(case.replacement, case.fixture);
    let native = native_contract(&candidate, &case, failures);
    let carrier = native.carrier;
    let parser = TreeSitterParser::new().expect("parser");
    let errors = ErrorCollector::new();
    let mut file = parser.parse_chat_file_streaming(candidate.source(), &errors);
    let diagnostics = errors.to_vec();
    let validation = ErrorCollector::new();
    file.validate_with_alignment(&validation, TranscriptName::Anonymous);
    let findings = validation.to_vec();
    let expected_code = match case.policy {
        Policy::Mor => Some(ErrorCode::InvalidMorphologyFormat),
        Policy::GraLexical(code) => Some(code),
        Policy::GraError | Policy::GraIndexDeletion => Some(ErrorCode::UnparsableContent),
        Policy::TextRecovery => Some(ErrorCode::TreeParsingError),
        Policy::EmptyText => None,
    };
    if let Some(code) = expected_code {
        if !diagnostics.iter().any(|error| {
            error.code == code
                && error.location.span().expect("located diagnostic").start()
                    >= candidate.span().start()
                && error.location.span().expect("located diagnostic").end()
                    <= candidate.span().end()
        }) {
            failures.push(format!(
                "missing fault-local parse {code:?}: {diagnostics:?}"
            ));
        }
    } else if !diagnostics.is_empty() {
        failures.push(format!("unexpected parse diagnostic: {diagnostics:?}"));
    }
    for error in diagnostics.iter().chain(&findings) {
        let span = error.location.span().expect("located diagnostic");
        if candidate.source().get(span.start()..span.end()).is_none() {
            failures.push(format!("diagnostic escapes caller UTF-8 source: {error:?}"));
        }
        for label in &error.labels {
            let span = label.span;
            if candidate.source().get(span.start()..span.end()).is_none() {
                failures.push(format!("label escapes caller UTF-8 source: {label:?}"));
            }
        }
    }
    if file.utterances().count() != TURN_COUNT {
        failures.push("changed utterance count".into());
    }
    for (index, expected) in control.file().utterances().enumerate() {
        let main_span = candidate.translated(expected.main.location);
        let Some(actual) = file.utterances().find(|utterance| {
            utterance.main.location == talkbank_model::Location::Located(main_span)
        }) else {
            failures.push(format!(
                "lost healthy source-owned main tier {index}: {main_span:?}"
            ));
            continue;
        };
        if !actual.main.semantic_eq(&expected.main) {
            failures.push(format!("changed main tier {index}"));
        }
        if actual.dependent_tiers.windows(2).any(|pair| {
            pair[0].location().span().expect("located").end()
                > pair[1].location().span().expect("located").start()
        }) {
            failures.push(format!(
                "dependent attachment lost native source order in utterance {index}"
            ));
        }
        if index != MIDDLE && (!actual.semantic_eq(expected) || !actual.parse_health().is_clean()) {
            failures.push(format!(
                "changed healthy neighboring utterance {index}: {:?}",
                actual.parse_health()
            ));
        }
        for expected_entry in &expected.dependent_tiers {
            if index == MIDDLE && case.tier.matches(expected_entry) {
                continue;
            }
            let span = candidate.translated(expected_entry.location());
            if !native.healthy_tiers.contains(&span) {
                failures.push(format!("independent native tier is not healthy: {span:?}"));
            }
            let Some(retained) = actual
                .dependent_tiers
                .iter()
                .find(|tier| tier.location() == talkbank_model::Location::Located(span))
            else {
                failures.push(format!(
                    "lost independent tier {}: {span:?}",
                    expected_entry.kind()
                ));
                continue;
            };
            if !retained.semantic_eq(expected_entry)
                || retained.separator != expected_entry.separator
            {
                failures.push(format!(
                    "changed independent tier {}: {span:?}",
                    expected_entry.kind()
                ));
            }
        }
    }
    let middle = file
        .utterances()
        .nth(MIDDLE)
        .expect("retained middle utterance");
    let health = middle.parse_health();
    if !health.is_tier_clean(ParseHealthTier::Main) {
        failures.push("dependent fault tainted intact main tier".into());
    }
    let damaged = match case.policy {
        Policy::Mor => Some(ParseHealthTier::Mor),
        Policy::GraLexical(_) | Policy::GraError | Policy::GraIndexDeletion => {
            Some(ParseHealthTier::Gra)
        }
        Policy::EmptyText | Policy::TextRecovery => None,
    };
    for tier in [
        ParseHealthTier::Mor,
        ParseHealthTier::Gra,
        ParseHealthTier::Pho,
        ParseHealthTier::Mod,
        ParseHealthTier::Wor,
        ParseHealthTier::Sin,
    ] {
        let should_taint = damaged == Some(tier) || matches!(case.policy, Policy::TextRecovery);
        if health.is_tier_tainted(tier) != should_taint
            || health.is_tier_clean(tier) == should_taint
        {
            failures.push(format!("incorrect tier-specific health {tier:?}: {health:?}, policy={:?}, carrier={carrier:?}", case.policy));
        }
    }
    match case.policy {
        Policy::Mor => {
            if middle.mor_tier().is_some()
                || !entry(middle, Tier::Mor)
                    .is_some_and(|entry| matches!(entry.tier, DependentTier::UnparsedMor(_)))
            {
                failures.push("document MOR recovery must preserve an unparsed tier slot".into());
            }
            assert!(
                middle.gra_tier().is_some(),
                "MOR recovery retains healthy GRA"
            );
        }
        Policy::GraLexical(_) | Policy::GraError | Policy::GraIndexDeletion => {
            assert!(
                middle.gra_tier().is_none(),
                "recovery cannot expose a graph"
            );
            let retained = entry(middle, Tier::Gra).expect("retained raw GRA slot");
            let DependentTier::UnparsedGra(raw) = &retained.tier else {
                panic!("expected explicit GRA recovery");
            };
            assert_eq!(raw.source(), candidate.tier_input());
        }
        Policy::TextRecovery => {
            let retained = entry(middle, case.tier);
            match case.tier {
                Tier::Com | Tier::Eng | Tier::Xnote => {
                    if retained.is_some() {
                        failures
                            .push("unreadable text body was fabricated as a retained tier".into());
                    }
                    if findings
                        .iter()
                        .any(|error| error.code == ErrorCode::EmptyDependentTier)
                    {
                        failures.push("body rejection manufactured empty-tier validation".into());
                    }
                }
                Tier::Mor | Tier::Gra => panic!("structured text policy"),
            }
            assert!(
                middle.gra_tier().is_some(),
                "text recovery retains intact GRA"
            );
        }
        Policy::EmptyText => {
            let retained = entry(middle, candidate.tier()).expect("empty text tier is retained");
            if retained.location() != talkbank_model::Location::Located(candidate.span())
                || !health.is_clean()
            {
                failures.push(
                    "empty optional content lost its slot or manufactured parse recovery".into(),
                );
            }
            if !findings.iter().any(|error| {
                error.code == ErrorCode::EmptyDependentTier
                    && error.location.span().expect("located diagnostic").start()
                        >= candidate.span().start()
                    && error.location.span().expect("located diagnostic").end()
                        <= candidate.span().end()
            }) {
                failures.push(format!(
                    "missing fault-local empty-content validation: {findings:?}"
                ));
            }
        }
    }
    if let Some(retained) = entry(middle, case.tier) {
        let original = entry(
            control
                .file()
                .utterances()
                .nth(MIDDLE)
                .expect("control middle"),
            case.tier,
        )
        .expect("control tier");
        if retained.location() != talkbank_model::Location::Located(candidate.span())
            || retained.separator != original.separator
        {
            failures.push("edited tier lost its slot/separator provenance".into());
        }
    }
    let alignments = file.validate_alignments();
    // Alignment warnings describe withheld comparisons of parsed tiers.
    // Raw recovery has its own explicit invalidity; it exposes no graph to compare.
    let expected_warnings = if middle.mor_tier().is_some() {
        usize::from(!health.can_align_main_to_mor())
            + usize::from(middle.gra_tier().is_some() && !health.can_align_mor_to_gra())
    } else {
        0
    };
    if !alignments.is_empty() {
        failures.push(format!(
            "file alignment validation must skip tainted domains: {alignments:?}"
        ));
    }
    let actual_warnings = middle
        .alignment_diagnostics
        .iter()
        .filter(|error| {
            error.code == ErrorCode::TierValidationError
                && error.severity == talkbank_model::Severity::Warning
        })
        .count();
    if actual_warnings != expected_warnings {
        failures.push(format!(
            "metadata alignment did not honor recovered provenance: {:?}",
            middle.alignment_diagnostics
        ));
    }
    // Reuse the existing coordinate contract, including inverse diagnostic
    // translation. A strict tier fragment never certifies a recovery placeholder.
    let path = std::path::Path::new("E330_dependent_composition.md");
    for input in [
        candidate.tier_input(),
        candidate.tier_input().trim_end_matches('\n'),
    ] {
        super::assert_fragment_rebases(path, input, |offset, errors| {
            ChatParser::parse_dependent_tier(&parser, input, offset, errors)
        });
        let fragment_errors = ErrorCollector::new();
        let fragment = ChatParser::parse_dependent_tier(
            &parser,
            input,
            candidate.span().start(),
            &fragment_errors,
        );
        let reject = expected_code.is_some() || carrier == Carrier::Recovery;
        if matches!(fragment, ParseOutcome::Rejected) != reject {
            failures.push(format!("document/native/strict fragment distinction: {fragment:?}, {:?}, carrier={carrier:?}", fragment_errors.to_vec()));
        }
        if let ParseOutcome::Parsed(actual) = fragment {
            let retained = entry(middle, case.tier).expect("accepted fragment's document tier");
            let supplied = Span::new(
                candidate.span().start(),
                candidate.span().start() + input.len(),
            );
            if !actual.semantic_eq(&retained.tier)
                || actual.location() != talkbank_model::Location::Located(supplied)
            {
                failures.push("accepted fragment changed the validation-only tier".into());
            }
        }
    }
    let decoded: ChatFile =
        serde_json::from_str(&serde_json::to_string(&file).expect("wire encode"))
            .expect("wire decode");
    if !decoded.semantic_eq(&file)
        || decoded
            .utterances()
            .any(|utterance| !utterance.parse_health().is_unknown())
    {
        failures.push("wire import changed semantics or fabricated parser trust".into());
    }
}

fn run(cases: impl IntoIterator<Item = Case>) {
    let control = admitted::Control::new();
    let mut failures = Vec::new();
    for case in cases {
        observe(&control, case, &mut failures);
    }
    assert!(
        failures.is_empty(),
        "complete interaction failure inventory:\n{}",
        failures.join("\n")
    );
}

#[test]
fn admitted_control_retains_every_source_owned_tier() {
    let control = admitted::Control::new();
    assert_eq!(CONTROL, specimen!(1));
    for tier in [Tier::Mor, Tier::Gra, Tier::Com, Tier::Eng, Tier::Xnote] {
        control.edit(tier);
    }
}

#[test]
fn morphology_recovery_keeps_healthy_tiers_and_only_mor_taint() {
    run([
        Case {
            tier: Tier::Mor,
            replacement: "middle .",
            fixture: specimen!(2),
            policy: Policy::Mor,
        },
        Case {
            tier: Tier::Mor,
            replacement: "co| .",
            fixture: specimen!(3),
            policy: Policy::Mor,
        },
    ]);
}

#[test]
fn structural_gra_recovery_distinguishes_native_document_and_fragment() {
    run([
        Case {
            tier: Tier::Gra,
            replacement: "|0|ROOT 2|1|PUNCT",
            fixture: specimen!(4),
            policy: Policy::GraIndexDeletion,
        },
        Case {
            tier: Tier::Gra,
            replacement: "bad|0|ROOT 2|1|PUNCT",
            fixture: specimen!(12),
            policy: Policy::GraError,
        },
    ]);
}

#[test]
fn lexical_gra_rejection_keeps_later_relation_and_only_gra_taint() {
    run([
        Case {
            tier: Tier::Gra,
            replacement: "0|0|ROOT 2|1|PUNCT",
            fixture: specimen!(5),
            policy: Policy::GraLexical(ErrorCode::InvalidGrammarIndex),
        },
        Case {
            tier: Tier::Gra,
            replacement: "99999999999999999999999|0|ROOT 2|1|PUNCT",
            fixture: specimen!(6),
            policy: Policy::GraLexical(ErrorCode::MalformedGrammarRelation),
        },
    ]);
}

#[test]
fn free_text_recovery_preserves_independent_tiers_and_conservative_health() {
    run([
        Case {
            tier: Tier::Com,
            replacement: "\u{15}middle comment",
            fixture: specimen!(7),
            policy: Policy::TextRecovery,
        },
        Case {
            tier: Tier::Eng,
            replacement: "\u{15}middle translation",
            fixture: specimen!(8),
            policy: Policy::TextRecovery,
        },
        Case {
            tier: Tier::Xnote,
            replacement: "\u{15}middle café",
            fixture: specimen!(9),
            policy: Policy::TextRecovery,
        },
    ]);
}

#[test]
fn empty_text_is_retained_for_validation_without_parse_taint() {
    run([
        Case {
            tier: Tier::Com,
            replacement: "",
            fixture: specimen!(10),
            policy: Policy::EmptyText,
        },
        Case {
            tier: Tier::Xnote,
            replacement: "",
            fixture: specimen!(11),
            policy: Policy::EmptyText,
        },
    ]);
}
