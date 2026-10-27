//! Typed bullet-content construction, wire and output contracts.

use super::{assert_output_refusals, assert_writer_refusals};
use talkbank_model::model::WriteChat;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_parser_tests::test_error::strict_parse;

/// The authored media examples must exercise real delimited bullets, not bare
/// timestamp-shaped words that merely satisfy a parse-only construct test.
#[test]
fn authored_bullet_constructs_preserve_timing_and_continuation() {
    use talkbank_model::model::{DependentTier, Utterance, UtteranceContent};

    let parser = TreeSitterParser::new().expect("parser");
    for (fixture, delimiters, main_tier) in [
        (
            include_str!(
                "../../../../grammar/test/corpus/generated/main_tier/inline_bullet_continuation.txt"
            ),
            4,
            true,
        ),
        (
            include_str!("../../../../grammar/test/corpus/generated/tiers/x_tier_with_bullets.txt"),
            2,
            false,
        ),
    ] {
        let header = format!("\n{}\n", "=".repeat(80));
        let divider = format!("\n{}\n", "-".repeat(80));
        let (_, body) = fixture
            .split_once(&header)
            .expect("generated fixture header");
        let (source, _) = body.split_once(&divider).expect("generated fixture input");
        assert_eq!(source.matches('\u{15}').count(), delimiters);
        // Grammar corpus inputs are fragments. The public file API needs the
        // same document scaffolding as the generated construct parser tests;
        // this is syntax/model evidence, not participant-header validation.
        let document = talkbank_parser_tests::from_source::SingleSpeaker::english(
            source.trim_end_matches('\n'),
        )
        .source();
        let file =
            strict_parse(parser.parse_chat_file(&document)).expect("authored fixture parses");
        let utterances: Vec<_> = file.utterances().collect();
        let [utterance] = utterances.as_slice() else {
            panic!("fixture must describe one utterance");
        };
        if main_tier {
            let internal: Vec<_> = utterance
                .main
                .content
                .content
                .iter()
                .filter_map(|item| match item {
                    UtteranceContent::InternalBullet(bullet) => bullet.as_parsed(),
                    _ => None,
                })
                .collect();
            let [internal] = internal.as_slice() else {
                panic!("fixture must retain exactly one internal bullet");
            };
            let terminal = utterance
                .main
                .content
                .bullet
                .as_ref()
                .and_then(talkbank_model::BulletState::as_parsed)
                .expect("terminal bullet");
            for (bullet, times, spelling) in [
                (*internal, (0, 2633), "\u{15}0_2633\u{15}"),
                (terminal, (2633, 4026), "\u{15}2633_4026\u{15}"),
            ] {
                assert_eq!((bullet.timing.start_ms, bullet.timing.end_ms), times);
                assert_eq!(
                    document.get(
                        bullet.location.span().expect("located").start()
                            ..bullet.location.span().expect("located").end()
                    ),
                    Some(spelling),
                );
            }
            assert!(source.contains("\n\t(1.0)"));
            assert!(
                utterance
                    .main
                    .content
                    .content
                    .iter()
                    .any(|item| matches!(item, UtteranceContent::Pause(_)))
            );
        } else {
            let [entry] = utterance.dependent_tiers.as_slice() else {
                panic!("fixture must retain its user-defined tier");
            };
            let DependentTier::UserDefined(tier) = &entry.tier else {
                panic!("fixture must remain a user-defined tier");
            };
            assert_eq!(tier.label.as_str(), "xs1");
            assert_eq!(
                tier.content.as_deref(),
                Some("swallow \u{15}3093_4877\u{15}")
            );
        }
        let output = utterance.to_chat_string();
        let timings: &[&str] = if main_tier {
            &["\u{15}0_2633\u{15}", "\u{15}2633_4026\u{15}"]
        } else {
            &["\u{15}3093_4877\u{15}"]
        };
        for timing in timings {
            assert_eq!(output.matches(timing).count(), 1);
        }
        let restored: Utterance =
            serde_json::from_str(&serde_json::to_string(utterance).expect("fixture model JSON"))
                .expect("fixture model import");
        assert_eq!(restored.to_chat_string(), output);
    }
}

/// Body-only consumers preserve typed media and wrapping, independently of
/// which ordinary dependent tier owns the content.
#[test]
fn reference_dependent_bodies_preserve_media_wrapping_and_sink_refusal() {
    use talkbank_model::model::{BulletContent, BulletContentSegment, DependentTier};

    let parser = TreeSitterParser::new().expect("parser");
    let mut shapes = [0usize; 4];
    let mut boundaries = 0;
    for relative in [
        "corpus/reference/content/media-bullets.cha",
        "corpus/reference/core/multiline-continuation.cha",
    ] {
        let path = workspace_root().join(relative);
        let source = std::fs::read_to_string(&path).expect("reference source");
        let file = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
        for utterance in file.utterances() {
            for entry in &utterance.dependent_tiers {
                let body = match &entry.tier {
                    DependentTier::Act(tier) => &tier.content,
                    DependentTier::Cod(tier) => &tier.content,
                    DependentTier::Com(tier) => &tier.content,
                    _ => continue, // These references select bullet-capable tier bodies.
                };
                for segment in &body.segments {
                    shapes[match segment {
                        BulletContentSegment::Text(_) => 0,
                        BulletContentSegment::Bullet(_) => 1,
                        BulletContentSegment::Picture(_) => 2,
                        BulletContentSegment::Continuation => 3,
                        BulletContentSegment::UnparsedBullet(_) => 4,
                    }] += 1;
                }
                let mut streamed = String::new();
                body.write_chat(&mut streamed).expect("body output");
                assert_eq!(body.to_chat_string(), streamed);
                let wire = serde_json::to_value(body).expect("body JSON");
                let restored: BulletContent = serde_json::from_value(wire).expect("body import");
                assert_eq!(&restored, body);
                assert_eq!(restored.to_chat_string(), streamed);
                boundaries +=
                    assert_output_refusals(|mut writer| body.write_chat(&mut writer), &path);
            }
        }
    }
    assert!(
        shapes.iter().all(|count| *count > 0),
        "missing body shape: {shapes:?}"
    );
    assert!(boundaries > 0, "body output must exercise sink refusal");
}

/// Concrete tier constructors distinguish plain text from structured media bodies.
#[test]
fn reference_text_tier_constructors_preserve_structure_and_output() {
    use talkbank_model::model::{
        ActTier, AddTier, BulletContentSegment, CodTier, ComTier, DependentTier, ExpTier, GpxTier,
        IntTier, SemanticEq, SitTier, SpaTier,
    };

    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut plain_witnesses = [0usize; 9];
    let mut structured_witnesses = 0;
    for fixture in corpus.fixtures() {
        let file = strict_parse(parser.parse_chat_file(fixture.source()))
            .expect("canonical reference parses");
        for utterance in file.utterances() {
            for entry in &utterance.dependent_tiers {
                macro_rules! check_tier {
                    ($tier:expr, $kind:ty, $index:expr) => {{
                        let tier = $tier;
                        let output = entry.tier.to_chat_string();
                        assert_eq!(tier.to_chat(), output);
                        let rebuilt = <$kind>::new(tier.location, tier.content.clone());
                        assert_eq!(&rebuilt, tier, "{}", fixture.path().display());
                        assert_eq!(rebuilt.is_empty(), tier.content.is_empty());
                        assert_eq!(rebuilt.to_chat(), output);
                        // Pattern admission supplies actual text, never a flattened
                        // serialization of timing, picture or continuation nodes.
                        if let [BulletContentSegment::Text(text)] = tier.content.segments.as_slice()
                        {
                            let plain = <$kind>::from_text(
                                talkbank_model::Location::Unlocated,
                                text.text.clone(),
                            );
                            assert!(plain.semantic_eq(tier));
                            assert_eq!(plain.to_chat(), output);
                            assert_eq!(plain.is_empty(), tier.is_empty());
                            assert!(plain.location.span().is_none());
                            plain_witnesses[$index] += 1;
                        } else {
                            structured_witnesses += 1;
                        }
                    }};
                }
                match &entry.tier {
                    DependentTier::Act(tier) => check_tier!(tier, ActTier, 0),
                    DependentTier::Cod(tier) => check_tier!(tier, CodTier, 1),
                    DependentTier::Com(tier) => check_tier!(tier, ComTier, 2),
                    DependentTier::Exp(tier) => check_tier!(tier, ExpTier, 3),
                    DependentTier::Add(tier) => check_tier!(tier, AddTier, 4),
                    DependentTier::Spa(tier) => check_tier!(tier, SpaTier, 5),
                    DependentTier::Sit(tier) => check_tier!(tier, SitTier, 6),
                    DependentTier::Gpx(tier) => check_tier!(tier, GpxTier, 7),
                    DependentTier::Int(tier) => check_tier!(tier, IntTier, 8),
                    _ => {} // Structured-language tiers have different constructors.
                }
            }
        }
    }
    assert!(
        plain_witnesses.iter().all(|count| *count > 0),
        "missing tier witness: {plain_witnesses:?}"
    );
    assert!(structured_witnesses > 0, "retain non-plain body controls");
}

/// A caller edits free-text payloads without reparsing or disturbing media.
#[test]
fn reference_comment_edits_preserve_nontext_segments_and_wire_contracts() {
    use talkbank_model::model::{BulletContent, BulletContentSegment, Header, Line};

    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let mut edited_texts = 0;
    let mut retained_payloads = 0;
    for fixture in corpus.fixtures() {
        let parsed = strict_parse(parser.parse_chat_file(fixture.source()))
            .expect("canonical reference parses");
        for line in &parsed.lines {
            let Line::Header { header, .. } = line else {
                continue;
            };
            let Header::Comment { content } = header.as_ref() else {
                continue;
            };
            let mut segments = content.segments.clone();
            for segment in &mut segments {
                if let BulletContentSegment::Text(text) = segment {
                    // Simulate a caller-supplied replacement of an admitted
                    // free-text field, not a CHAT tokenizer or rewrite rule.
                    let replacement = format!("edited {}", text.text);
                    *segment = BulletContentSegment::text(replacement);
                    edited_texts += 1;
                }
            }
            let edited = BulletContent::new(segments.into_iter().collect());
            assert_eq!(edited.segments.len(), content.segments.len());
            for (before, after) in content.segments.iter().zip(&edited.segments) {
                match before {
                    BulletContentSegment::Text(text) => {
                        assert_eq!(
                            *after,
                            BulletContentSegment::text(format!("edited {}", text.text))
                        );
                    }
                    BulletContentSegment::Bullet(_)
                    | BulletContentSegment::UnparsedBullet(_)
                    | BulletContentSegment::Picture(_)
                    | BulletContentSegment::Continuation => {
                        assert_eq!(before, after);
                        retained_payloads += 1;
                    }
                }
            }
            let json = serde_json::to_string(&edited).expect("serialize edited content");
            let restored: BulletContent = serde_json::from_str(&json).expect("restore content");
            assert_eq!(restored, edited);
            assert!(
                assert_writer_refusals(&Header::Comment { content: edited }, fixture.path()) > 0
            );
        }
    }
    assert!(edited_texts > 0, "reference comments must exercise edits");
    assert!(
        retained_payloads > 0,
        "reference comments must retain media or wrapping"
    );
}

/// Rejected main-tier media remains source, never absent or usable timing.
#[test]
fn rejected_main_bullets_survive_wire_and_refuse_timing_consumers() {
    use talkbank_model::alignment::helpers::{ContentItem, walk_content};
    use talkbank_model::{
        BulletState, ChatFile, ChatParser, ErrorCode, ErrorCollector, TranscriptName,
    };
    use talkbank_transform::utterance_split::{SplitRefusal, UtteranceSplitPlan};

    #[derive(Clone, Copy)]
    enum Sanitization {
        PreservesTiming,
        RefusesOrphan,
    }

    fn check(parser: &impl ChatParser, source: &str, sanitization: Sanitization) {
        let errors = ErrorCollector::new();
        let file = parser
            .parse_chat_file(source, 0, &errors)
            .into_option()
            .expect("recovered file");
        assert!(
            errors
                .to_vec()
                .iter()
                .any(|e| e.code == ErrorCode::InvalidMediaBullet),
            "{}",
            parser.parser_name()
        );
        let output = file.to_chat_string();
        let wire = serde_json::to_string(&file).expect("wire");
        assert!(wire.contains("unparsed"));
        let decoded: ChatFile = serde_json::from_str(&wire).expect("recovery JSON");
        assert_eq!(decoded.to_chat_string(), output);
        for document in [&file, &decoded] {
            let utterance = document.utterances().next().expect("utterance");
            assert!(utterance.main.content.has_unparsed_bullets());
            let mut rejected = Vec::new();
            walk_content(&utterance.main.content.content, None, &mut |item| {
                if let ContentItem::InternalBullet(BulletState::Unparsed { unparsed }) = item {
                    rejected.push(unparsed);
                }
            });
            if let Some(BulletState::Unparsed { unparsed }) = &utterance.main.content.bullet {
                rejected.push(unparsed);
                let parents = document
                    .utterances()
                    .enumerate()
                    .map(|(i, _)| {
                        talkbank_transform::transcript_merge::DonorIdx::new(
                            talkbank_model::UtteranceIdx::new(i),
                        )
                    })
                    .collect();
                assert!(
                    talkbank_transform::transcript_merge::SourceBoundDonorSelection::bind(
                        document, document, parents
                    )
                    .is_err(),
                    "rejected terminal cannot establish donor coordinates"
                );
                assert!(
                    utterance.media_bullet().is_none(),
                    "rejected terminal cannot promote internal timing"
                );
                let errors = ErrorCollector::new();
                let (unchanged, outcome) = talkbank_transform::rediarize::rediarize(
                    document,
                    &talkbank_transform::rediarize::DiarizationTimeline::new(Vec::new()),
                    None,
                    &errors,
                )
                .expect("admitted participant declaration");
                assert_eq!(
                    unchanged.utterances().next().expect("retained").main,
                    utterance.main
                );
                assert_eq!(
                    outcome.flagged[0].reason,
                    talkbank_transform::rediarize::FlagReason::UnparsedBullet
                );
            }
            assert_eq!(rejected.len(), 1);
            for raw in rejected {
                assert!(source.contains(raw.source()));
                assert!(output.contains(raw.source()));
                let result = talkbank_transform::redact::sanitize(
                    document.clone(),
                    &talkbank_transform::redact::SanitizationPolicy::strict(),
                );
                match sanitization {
                    Sanitization::PreservesTiming => {
                        let sanitized = result.expect("redaction preserves timing source");
                        assert!(sanitized.to_chat_string().contains(raw.source()));
                    }
                    Sanitization::RefusesOrphan => {
                        assert!(matches!(
                            result,
                            Err(talkbank_transform::redact::RedactError::OrphanAnnotation)
                        ));
                    }
                }
                if let Some(span) = raw.location().span() {
                    assert_eq!(&source[span.to_range()], raw.source());
                }
            }
            assert!(matches!(
                UtteranceSplitPlan::for_morphology(utterance, &[0, 1]),
                Err(SplitRefusal::UnparsedTiming)
            ));
            let errors = ErrorCollector::new();
            document.validate(&errors, TranscriptName::Anonymous);
            assert!(
                errors
                    .to_vec()
                    .iter()
                    .any(|e| e.code == ErrorCode::InvalidMediaBullet)
            );
            assert!(
                !errors.to_vec().iter().any(|e| e.code.as_str() == "E544"),
                "rejected timing is not missing timing"
            );
        }
    }

    let root = workspace_root();
    let specs = talkbank_parser_tests::error_specs::load(root).expect("specs");
    let spec = specs
        .iter()
        .find(|s| s.filename == "E360.md")
        .expect("E360");
    let native = talkbank_parser_re2c::Re2cParser::new();
    let tree = TreeSitterParser::new().expect("parser");
    for (title, native_sanitization) in [
        (
            "Timestamps too large to be timestamps",
            Sanitization::PreservesTiming,
        ),
        (
            "Nested overflow retains surrounding content without invented timing",
            Sanitization::PreservesTiming,
        ),
        (
            "Internal overflow retains words on both sides",
            Sanitization::PreservesTiming,
        ),
        (
            "Rejected final timing does not promote an earlier inline bullet",
            Sanitization::PreservesTiming,
        ),
        // Native recovery retains this unattached postcode as an orphan
        // annotation. Its sanitizer refusal is independent of bullet recovery;
        // the primary parser retains a structured terminal postcode instead.
        (
            "Rejected explicit terminal slot does not absorb an inline bullet",
            Sanitization::RefusesOrphan,
        ),
    ] {
        let source = spec
            .examples()
            .iter()
            .find(|e| e.title.as_deref() == Some(title))
            .expect("example")
            .chat
            .as_str();
        check(&native, source, native_sanitization);
        check(&tree, source, Sanitization::PreservesTiming);
        assert!(
            tree.admit_replacing_tiers(
                source,
                talkbank_parser::ReplacementTiers::WordTiming,
                TranscriptName::Anonymous
            )
            .is_err(),
            "tier replacement cannot admit rejected main timing"
        );
        // A media declaration cannot turn numeric rejection into missing timing.
        if !source.contains("@Media:") {
            let with_media = source.replacen("*CHI:", "@Media:\tsession, audio\n*CHI:", 1);
            check(&native, &with_media, native_sanitization);
            check(&tree, &with_media, Sanitization::PreservesTiming);
        }
    }
}
