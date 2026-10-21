//! Original-source controls complement the retained standalone fixture decks.

use super::{ComparisonBatch, both_parsers, compare_parses};
use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::model::{CaOptionEffect, DependentTier, Header, Line};
use talkbank_model::{ChatOptionFlag, ChatParser, FragmentSemanticContext};
use talkbank_parser_tests::chat_corpus::ChatCorpus;
use talkbank_parser_tests::test_error::strict_parse;

enum TierApi {
    Mor,
    Gra,
}

#[test]
fn reference_word_and_structured_tier_fragments_agree() -> Result<(), String> {
    let (ts, re2c) = both_parsers();
    let corpus = ChatCorpus::reference().map_err(|error| error.to_string())?;
    let mut batch = ComparisonBatch::default();
    let mut witnesses = [0usize; 3];
    for fixture in corpus.fixtures() {
        let file = strict_parse(ts.parse_chat_file(fixture.source()))
            .map_err(|error| format!("{}: {error}", fixture.path().display()))?;
        let ca = file.lines.iter().any(|line| {
            matches!(line,
            Line::Header { header, .. } if matches!(header.as_ref(),
                Header::Options { options } if options.iter().any(|option|
                    option.has_effect(CaOptionEffect::ParentheticalIsCaOmission))))
        });
        let context = if ca {
            FragmentSemanticContext::new().with_option_flag(ChatOptionFlag::Ca)
        } else {
            FragmentSemanticContext::new()
        };
        for utterance in file.utterances() {
            walk_words(&utterance.main.content.content, None, &mut |item| {
                let word = match item {
                    WordItem::Word(word) => word,
                    WordItem::ReplacedWord(replaced) => &replaced.word,
                    WordItem::Separator(_) => return,
                };
                let word_span = word.location.span().expect("located");
                let origin = word_span.start();
                let input = fixture
                    .source()
                    .get(word_span.to_range())
                    .expect("admitted reference word source");
                batch.record(
                    format!("{} word {origin} {input:?}", fixture.path().display()),
                    compare_parses(
                        |errors| {
                            ChatParser::parse_word_with_context(
                                &ts, input, origin, &context, errors,
                            )
                        },
                        |errors| {
                            ChatParser::parse_word_with_context(
                                &re2c, input, origin, &context, errors,
                            )
                        },
                    ),
                );
                witnesses[0] += 1;
            });
            for entry in &utterance.dependent_tiers {
                let api = match &entry.tier {
                    DependentTier::Mor(_) => TierApi::Mor,
                    DependentTier::Gra(_) => TierApi::Gra,
                    _ => continue,
                };
                let span = entry.content_span().expect("reference tier content span");
                let origin = span.start();
                let input = fixture
                    .source()
                    .get(origin..span.end())
                    .expect("admitted reference tier source");
                let result = match api {
                    TierApi::Mor => {
                        witnesses[1] += 1;
                        compare_parses(
                            |errors| ChatParser::parse_mor_tier(&ts, input, origin, errors),
                            |errors| ChatParser::parse_mor_tier(&re2c, input, origin, errors),
                        )
                    }
                    TierApi::Gra => {
                        witnesses[2] += 1;
                        compare_parses(
                            |errors| ChatParser::parse_gra_tier(&ts, input, origin, errors),
                            |errors| ChatParser::parse_gra_tier(&re2c, input, origin, errors),
                        )
                    }
                };
                batch.record(
                    format!("{} tier {origin} {input:?}", fixture.path().display()),
                    result,
                );
            }
        }
    }
    eprintln!("reference word/MOR/GRA witnesses: {witnesses:?}");
    assert!(witnesses.iter().all(|count| *count > 0));
    batch.finish()
}
