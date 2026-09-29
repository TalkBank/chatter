//! Typed bullet-content construction, wire and output contracts.

use super::{assert_output_refusals, assert_writer_refusals};
use talkbank_model::model::WriteChat;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::repo_paths::workspace_root;
use talkbank_parser_tests::test_error::strict_parse;

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
                        let rebuilt = <$kind>::new(tier.content.clone()).with_span(tier.span);
                        assert_eq!(&rebuilt, tier, "{}", fixture.path().display());
                        assert_eq!(rebuilt.is_empty(), tier.content.is_empty());
                        assert_eq!(rebuilt.to_chat(), output);
                        // Pattern admission supplies actual text, never a flattened
                        // serialization of timing, picture or continuation nodes.
                        if let [BulletContentSegment::Text(text)] = tier.content.segments.as_slice()
                        {
                            let plain = <$kind>::from_text(text.text.clone());
                            assert!(plain.semantic_eq(tier));
                            assert_eq!(plain.to_chat(), output);
                            assert_eq!(plain.is_empty(), tier.is_empty());
                            assert!(plain.span.is_dummy());
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
            assert_eq!(segments.is_empty(), segments.is_empty());
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
