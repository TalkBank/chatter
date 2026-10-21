//! Finite repeated-clitic combinations through admitted public CHAT workflows.
//! Source-owned edits distinguish item/chunk/relation spaces without fabricated ASTs.
#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::model::{DependentTier, DependentTierEntry, GraCompleteness, ParseHealthTier};
use talkbank_model::{
    ChatFile, ChatParser, ErrorCode, ErrorCollector, FragmentSemanticContext, ParseOutcome,
    SemanticEq, Span, TranscriptName, Utterance,
};
use talkbank_parser::TreeSitterParser;
use talkbank_parser::generated_traversal::{AsRawNode, MorDependentTierNode};
use talkbank_parser_tests::repo_paths::workspace_root;

const CONTROL: &str = include_str!("../../../../corpus/reference/tiers/mor-gra-combinations.cha");
const MOR: &str = "pron:pers|we-Nom-Person=1~aux|have-Tense=Past~aux|have-Mood=Ind-Polarity=Pos verb|find-Past noun|café-Case=Acc noun|ice_cream-PronType=Int,Rel .";
const GRA: &str = "1|4|NSUBJ 2|4|AUX 3|2|AUX 4|0|ROOT 5|4|OBJ 6|4|OBJ 7|4|PUNCT";

#[derive(Clone, Copy, Debug)]
enum Layout {
    Lf,
    CrLf,
}
impl Layout {
    fn source(self, source: &str) -> String {
        match self {
            Self::Lf => source.to_owned(),
            Self::CrLf => source.replace('\n', "\r\n"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tier {
    Mor,
    Gra,
}
fn entry(turn: &Utterance, tier: Tier) -> Option<&DependentTierEntry> {
    turn.dependent_tiers.iter().find(|entry| match tier {
        Tier::Mor => matches!(entry.tier, DependentTier::Mor(_)),
        Tier::Gra => matches!(entry.tier, DependentTier::Gra(_)),
    })
}

#[path = "mor_gra_combination_admission.rs"]
mod admitted;

fn specimen(code: &str, example: usize, layout: Layout) -> String {
    layout.source(&std::fs::read_to_string(workspace_root().join(format!(
        "crates/talkbank-parser-tests/tests/error_corpus/validation_errors/{code}_mor_gra_combinations_{example}.cha"
    ))).expect("owned generated specimen"))
}

fn require(condition: bool, message: impl Into<String>, failures: &mut Vec<String>) {
    if !condition {
        failures.push(message.into());
    }
}

fn finish(failures: Vec<String>) {
    assert!(
        failures.is_empty(),
        "complete combination failure inventory:\n{}",
        failures.join("\n")
    );
}

fn parse(
    source: &str,
) -> (
    ChatFile,
    Vec<talkbank_model::ParseError>,
    Vec<talkbank_model::ParseError>,
) {
    let parser = TreeSitterParser::new().expect("parser");
    let errors = ErrorCollector::new();
    let mut file = parser.parse_chat_file_streaming(source, &errors);
    let parse = errors.into_vec();
    let errors = ErrorCollector::new();
    file.validate_with_alignment(&errors, TranscriptName::Anonymous);
    (file, parse, errors.into_vec())
}

fn preserve_neighbors(
    candidate: &admitted::Candidate<'_>,
    file: &ChatFile,
    failures: &mut Vec<String>,
) {
    require(file.utterances().count() == 3, "turn count", failures);
    for (index, expected) in candidate.control().file().utterances().enumerate() {
        let span = candidate.translated(expected.main.location);
        let Some(actual) = file
            .utterances()
            .find(|turn| turn.main.location == talkbank_model::Location::Located(span))
        else {
            failures.push(format!("lost healthy main {index}: {span:?}"));
            continue;
        };
        require(
            actual.main.semantic_eq(&expected.main),
            format!("changed main {index}"),
            failures,
        );
        require(
            actual.parse_health().is_tier_clean(ParseHealthTier::Main),
            "main taint",
            failures,
        );
        if index != 1 {
            require(
                actual.semantic_eq(expected) && actual.parse_health().is_clean(),
                format!("changed neighboring turn {index}"),
                failures,
            );
        }
        for expected_entry in &expected.dependent_tiers {
            if index == 1
                && entry(expected, candidate.tier())
                    .is_some_and(|selected| std::ptr::eq(selected, expected_entry))
            {
                continue;
            }
            let span = candidate.translated(expected_entry.location());
            require(
                actual.dependent_tiers.iter().any(|actual| {
                    actual.location() == talkbank_model::Location::Located(span)
                        && actual.semantic_eq(expected_entry)
                        && actual.separator == expected_entry.separator
                }),
                format!("lost independent {}: {span:?}", expected_entry.kind()),
                failures,
            );
        }
    }
}

fn wire_contract(file: &ChatFile, failures: &mut Vec<String>) -> ChatFile {
    let decoded: ChatFile = serde_json::from_slice(&serde_json::to_vec(file).expect("wire output"))
        .expect("wire input");
    require(decoded.semantic_eq(file), "wire semantics", failures);
    require(
        decoded
            .utterances()
            .all(|turn| turn.parse_health().is_unknown()),
        "wire fabricated parse health",
        failures,
    );
    require(
        decoded
            .utterances()
            .filter_map(|turn| turn.gra_tier())
            .all(|gra| gra.completeness() == GraCompleteness::Unknown),
        "wire fabricated GRA completeness",
        failures,
    );
    require(
        decoded
            .utterances()
            .flat_map(|turn| &turn.dependent_tiers)
            .all(|entry| entry.content_span().is_none()),
        "wire fabricated body provenance",
        failures,
    );
    decoded
}

#[test]
fn mor_gra_combinations_preserve_order_counts_and_public_fragments() {
    let mut failures = Vec::new();
    for layout in [Layout::Lf, Layout::CrLf] {
        let control = admitted::Control::new(layout);
        let middle = control.middle();
        let mor = middle.mor_tier().expect("mor");
        require(
            middle.mor_alignable_word_count().get() == 4
                && mor.len() == 4
                && mor.count_chunks() == 7,
            "distinct main/item/chunk spaces",
            &mut failures,
        );
        require(
            mor.items()[0].post_clitics.len() == 2,
            "repeated post-clitics",
            &mut failures,
        );
        require(
            mor.items()[0].main.pos.category() == "pron"
                && mor.items()[0].main.pos.subcategories().eq(["pers"]),
            "POS subcategory",
            &mut failures,
        );
        let chunks: Vec<_> = mor.chunks().collect();
        let lemmas: Vec<_> = chunks.iter().map(|chunk| chunk.lemma()).collect();
        require(
            lemmas
                == [
                    Some("we"),
                    Some("have"),
                    Some("have"),
                    Some("find"),
                    Some("café"),
                    Some("ice_cream"),
                    None,
                ],
            "chunk/Unicode/compound ordering",
            &mut failures,
        );
        let features = &mor.items()[0].post_clitics[1].features;
        require(
            features.len() == 2
                && features[0].key() == Some("Mood")
                && features[0].value() == "Ind"
                && features[1].key() == Some("Polarity")
                && features[1].value() == "Pos",
            "ordered features",
            &mut failures,
        );
        let multivalue = &mor.items()[3].main.features[0];
        require(
            multivalue.key() == Some("PronType") && multivalue.value() == "Int,Rel",
            "one multivalue feature",
            &mut failures,
        );
        let parser = TreeSitterParser::new().expect("parser");
        for tier in [Tier::Mor, Tier::Gra] {
            let expected = entry(middle, tier).expect("tier");
            let span = expected.location();
            let input = &control.source()
                [span.span().expect("located").start()..span.span().expect("located").end()];
            let path = std::path::Path::new("corpus/reference/tiers/mor-gra-combinations.cha");
            super::assert_fragment_rebases(path, input, |offset, errors| {
                ChatParser::parse_dependent_tier(&parser, input, offset, errors)
            });
            super::assert_fragment_rebases(path, input, |offset, errors| {
                ChatParser::parse_dependent_tier_with_context(
                    &parser,
                    input,
                    offset,
                    &FragmentSemanticContext::new(),
                    errors,
                )
            });
            for offset in [
                0,
                span.span().expect("located").start(),
                span.span().expect("located").start() + 25,
            ] {
                let errors = ErrorCollector::new();
                let actual = ChatParser::parse_dependent_tier(&parser, input, offset, &errors);
                require(
                    matches!(actual, ParseOutcome::Parsed(ref actual) if actual.semantic_eq(&expected.tier)),
                    format!("standalone {tier:?}: {actual:?}"),
                    &mut failures,
                );
                require(
                    errors.is_empty(),
                    format!("fragment diagnostics {:?}", errors.to_vec()),
                    &mut failures,
                );
                let errors = ErrorCollector::new();
                let actual = ChatParser::parse_dependent_tier_with_context(
                    &parser,
                    input,
                    offset,
                    &FragmentSemanticContext::new(),
                    &errors,
                );
                require(
                    matches!(actual, ParseOutcome::Parsed(ref actual) if actual.semantic_eq(&expected.tier)),
                    "contextual tier",
                    &mut failures,
                );
                require(errors.is_empty(), "contextual errors", &mut failures);
            }
        }
        wire_contract(control.file(), &mut failures);
    }
    finish(failures);
}

#[test]
fn mor_gra_combinations_distinguish_items_from_word_chunks() {
    let mut failures = Vec::new();
    for layout in [Layout::Lf, Layout::CrLf] {
        let control = admitted::Control::new(layout);
        for (code, index, tier, replacement, expected_items, expected_chunks, expected_code) in [
            (
                "E705",
                2,
                Tier::Mor,
                MOR.replace(" verb|find-Past noun|café", " verb|find-Past~noun|café"),
                3,
                7,
                ErrorCode::MorCountMismatchTooFew,
            ),
            (
                "E706",
                2,
                Tier::Mor,
                MOR.replace("~aux|have-Tense=Past", " aux|have-Tense=Past"),
                5,
                7,
                ErrorCode::MorCountMismatchTooMany,
            ),
            (
                "E720",
                2,
                Tier::Mor,
                MOR.replace("~aux|have-Mood=Ind-Polarity=Pos", ""),
                4,
                6,
                ErrorCode::MorGraCountMismatch,
            ),
            (
                "E720",
                3,
                Tier::Gra,
                GRA.replace("3|2|AUX ", ""),
                4,
                7,
                ErrorCode::MorGraCountMismatch,
            ),
            (
                "E720",
                4,
                Tier::Gra,
                format!("{GRA} 8|4|PUNCT"),
                4,
                7,
                ErrorCode::MorGraCountMismatch,
            ),
        ] {
            let candidate = control
                .edit(tier)
                .replace(&replacement, &specimen(code, index, layout));
            let (file, parse, findings) = parse(candidate.source());
            require(
                parse.is_empty(),
                format!("{code}/{index} syntax: {parse:?}"),
                &mut failures,
            );
            preserve_neighbors(&candidate, &file, &mut failures);
            let middle = file.utterances().nth(1).expect("middle");
            let mor = middle.mor_tier().expect("mor");
            require(
                mor.len() == expected_items && mor.count_chunks() == expected_chunks,
                format!("{code}/{index} count spaces"),
                &mut failures,
            );
            require(
                middle.parse_health().is_clean(),
                "validation is not recovery",
                &mut failures,
            );
            require(
                findings.iter().any(|error| error.code == expected_code),
                format!("{code}/{index} findings: {findings:?}"),
                &mut failures,
            );
            if code != "E720" {
                require(
                    !findings
                        .iter()
                        .any(|error| error.code == ErrorCode::MorGraCountMismatch),
                    "item mutation changed unchanged chunk relation count",
                    &mut failures,
                );
            }
        }
    }
    finish(failures);
}

#[test]
fn mor_gra_combinations_feature_faults_remain_local_through_wire() {
    let mut failures = Vec::new();
    for layout in [Layout::Lf, Layout::CrLf] {
        let control = admitted::Control::new(layout);
        for (index, replacement, count) in [
            (2, MOR.replace("Person=1", "Person="), 1),
            (3, MOR.replace("Mood=Ind", "Mood="), 1),
            (
                4,
                MOR.replace("Mood=Ind-Polarity=Pos", "Mood=-Polarity="),
                2,
            ),
            (5, MOR.replace("PronType=Int,Rel", "PronType="), 1),
        ] {
            let candidate = control
                .edit(Tier::Mor)
                .replace(&replacement, &specimen("E711", index, layout));
            let (file, parse, findings) = parse(candidate.source());
            require(
                parse.is_empty(),
                format!("E711/{index} parse: {parse:?}"),
                &mut failures,
            );
            preserve_neighbors(&candidate, &file, &mut failures);
            let middle = file.utterances().nth(1).expect("middle");
            let mor = middle.mor_tier().expect("mor");
            require(
                mor.len() == 4 && mor.count_chunks() == 7 && mor.items()[0].post_clitics.len() == 2,
                "feature fault erased a word or changed count space",
                &mut failures,
            );
            let errors: Vec<_> = findings
                .iter()
                .filter(|error| error.code == ErrorCode::MorEmptyContent)
                .collect();
            require(
                errors.len() == count
                    && errors.iter().all(|error| {
                        error.location.span().expect("located diagnostic")
                            == mor.location.span().expect("located")
                    }),
                format!("E711/{index} independent diagnostics: {findings:?}"),
                &mut failures,
            );
            require(
                middle.parse_health().is_clean(),
                "keyed empty value is validation not recovery",
                &mut failures,
            );
            let decoded = wire_contract(&file, &mut failures);
            let errors = ErrorCollector::new();
            decoded
                .utterances()
                .nth(1)
                .expect("wire middle")
                .mor_tier()
                .expect("wire mor")
                .validate_content(&errors);
            require(
                errors
                    .to_vec()
                    .iter()
                    .filter(|error| error.code == ErrorCode::MorEmptyContent)
                    .count()
                    == count,
                "wire import repaired feature content",
                &mut failures,
            );
        }
    }
    finish(failures);
}

fn recovery_replacements() -> [String; 5] {
    [
        MOR.replace("pron:pers|we-Nom", "pron:pers|-Nom"),
        MOR.replace("~aux|have-Tense", "~aux|-Tense"),
        MOR.replace("~aux|have-Mood", "~|have-Mood"),
        MOR.replace("~aux|have-Mood", "~aux|have|again-Mood"),
        MOR.replace("Polarity=Pos verb", "Polarity=Pos~ verb"),
    ]
}

#[test]
fn mor_gra_combinations_recovery_retains_independent_turns_and_refuses_fragments() {
    let mut failures = Vec::new();
    for layout in [Layout::Lf, Layout::CrLf] {
        let control = admitted::Control::new(layout);
        for (case, replacement) in recovery_replacements().into_iter().enumerate() {
            let candidate = control
                .edit(Tier::Mor)
                .replace(&replacement, &specimen("E702", case + 2, layout));
            let parser = TreeSitterParser::new().expect("parser");
            let native = parser
                .parse_source_incremental(candidate.source(), None)
                .expect("native owner");
            let selected = native
                .root()
                .expect("native root")
                .descendants()
                .map(|node| node.expect("associated descendant"))
                .find(|node| {
                    node.raw_node().start_byte() == candidate.span().start()
                        && node.raw_node().end_byte() == candidate.span().end()
                });
            require(
                selected.is_some_and(|node| {
                    let bound = node
                        .typed::<MorDependentTierNode>()
                        .expect("source-owned MOR identity");
                    bound.raw_node().has_error()
                }),
                format!(
                    "E702/{} lacks claimed native MOR recovery carrier",
                    case + 2
                ),
                &mut failures,
            );
            let (file, parse, findings) = parse(candidate.source());
            require(
                parse
                    .iter()
                    .any(|error| error.code == ErrorCode::InvalidMorphologyFormat),
                format!("E702/{} parser findings: {parse:?}", case + 2),
                &mut failures,
            );
            require(
                !parse
                    .iter()
                    .chain(&findings)
                    .any(|error| error.code == ErrorCode::InternalError),
                "CHAT fault became internal failure",
                &mut failures,
            );
            preserve_neighbors(&candidate, &file, &mut failures);
            let middle = file.utterances().nth(1).expect("middle");
            require(
                middle.parse_health().is_tier_tainted(ParseHealthTier::Mor)
                    && middle.parse_health().is_tier_clean(ParseHealthTier::Gra),
                "tier health domains",
                &mut failures,
            );
            require(
                middle.gra_tier().expect("intact GRA").completeness() == GraCompleteness::Whole,
                "MOR fault withdrew GRA own completeness",
                &mut failures,
            );
            for offset in [0, candidate.span().start(), candidate.span().start() + 25] {
                let errors = ErrorCollector::new();
                let actual =
                    ChatParser::parse_dependent_tier(&parser, candidate.input(), offset, &errors);
                require(
                    matches!(actual, ParseOutcome::Rejected) && !errors.is_empty(),
                    format!("strict fragment admitted recovery: {actual:?}"),
                    &mut failures,
                );
            }
            for error in parse.iter().chain(&findings) {
                require(
                    candidate
                        .source()
                        .get(
                            error.location.span().expect("located diagnostic").start()
                                ..error.location.span().expect("located diagnostic").end(),
                        )
                        .is_some(),
                    format!("unreadable diagnostic: {error:?}"),
                    &mut failures,
                );
            }
            wire_contract(&file, &mut failures);
        }
    }
    finish(failures);
}

#[test]
fn mor_gra_combinations_incremental_edits_restore_cold_outcomes() {
    let mut failures = Vec::new();
    for layout in [Layout::Lf, Layout::CrLf] {
        let control = admitted::Control::new(layout);
        let parser = TreeSitterParser::new().expect("parser");
        let mut previous = None;
        let candidate = control
            .edit(Tier::Mor)
            .replace(&recovery_replacements()[3], &specimen("E702", 5, layout));
        for source in [control.source(), candidate.source(), control.source()] {
            let warm_errors = ErrorCollector::new();
            let (warm, revision) =
                parser.parse_chat_file_revision(source.into(), previous.as_ref(), &warm_errors);
            let cold_errors = ErrorCollector::new();
            let (cold, _) =
                parser.parse_chat_file_streaming_incremental(source, None, &cold_errors);
            require(
                warm == cold && warm_errors.to_vec() == cold_errors.to_vec(),
                "warm/cold source-owned repair",
                &mut failures,
            );
            require(
                revision.source() == source,
                "revision source",
                &mut failures,
            );
            previous = Some(revision);
        }
    }
    finish(failures);
}
