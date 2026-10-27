//! Numeric phonology pauses from the authored E737/E738 controls.

use super::{ComparisonBatch, both_parsers, compare_parses};
use crate::fixture_utils::{LineEnding, workspace_root};
use talkbank_model::model::{DependentTier, TranscriptName};
use talkbank_model::{ChatFile, ChatParser, ErrorCollector, ParseOutcome, RuleSelection};

fn validated(
    parser: &impl ChatParser,
    input: &str,
    origin: usize,
    rules: RuleSelection,
    name: TranscriptName<'_>,
    errors: &ErrorCollector,
) -> ParseOutcome<ChatFile> {
    match parser.parse_chat_file(input, origin, errors) {
        ParseOutcome::Parsed(mut file) => {
            file.validate_with_alignment_and_rules(rules, errors, name);
            ParseOutcome::Parsed(file)
        }
        ParseOutcome::Rejected => ParseOutcome::Rejected,
    }
}

#[test]
fn authored_numeric_phonology_pauses_agree() -> Result<(), String> {
    let specs = talkbank_parser_tests::error_specs::load(workspace_root())
        .map_err(|error| error.to_string())?;
    let (ts, re2c) = both_parsers();
    let mut batch = ComparisonBatch::default();
    let mut tier_witnesses = 0;
    for filename in ["E737.md", "E738.md"] {
        let spec = specs
            .iter()
            .find(|spec| spec.filename == filename)
            .expect("authored phonology spec");
        let example = &spec.examples()[4];
        let name = || {
            example
                .source
                .as_deref()
                .map_or(TranscriptName::Anonymous, |path| {
                    TranscriptName::for_path(std::path::Path::new(path))
                })
        };
        for ending in LineEnding::ALL {
            let input = ending.apply(example.chat.as_str());
            for origin in [0, 97] {
                batch.record(
                    format!("{filename}#5 {ending:?} origin={origin}"),
                    compare_parses(
                        |errors| {
                            validated(
                                &ts,
                                &input,
                                origin,
                                spec.rules().selection(),
                                name(),
                                errors,
                            )
                        },
                        |errors| {
                            validated(
                                &re2c,
                                &input,
                                origin,
                                spec.rules().selection(),
                                name(),
                                errors,
                            )
                        },
                    ),
                );
            }
        }

        // Borrow the original tier slice through the primary model's geometry.
        // No CHAT reconstruction or serialization is used as fragment input.
        let errors = ErrorCollector::new();
        let ParseOutcome::Parsed(file) =
            ChatParser::parse_chat_file(&ts, example.chat.as_str(), 0, &errors)
        else {
            return Err(format!("{filename}#5 primary control rejected"));
        };
        for utterance in file.utterances() {
            for entry in &utterance.dependent_tiers {
                match &entry.tier {
                    DependentTier::Pho(_) | DependentTier::Mod(_) => {}
                    _ => continue,
                }
                let span = entry.content_span().expect("authored tier geometry");
                let input = example
                    .chat
                    .as_str()
                    .get(span.to_range())
                    .expect("source-bound tier");
                tier_witnesses += 1;
                for origin in [0, span.start()] {
                    batch.record(
                        format!("{filename}#5 phonology tier origin={origin}"),
                        compare_parses(
                            |errors| ChatParser::parse_pho_tier(&ts, input, origin, errors),
                            |errors| re2c.parse_pho_tier(input, origin, errors),
                        ),
                    );
                }
            }
        }
    }

    // The same original numeric spelling appears in both selected examples.
    for origin in [0, 97] {
        batch.record(
            format!("authored phonology word origin={origin}"),
            compare_parses(
                |errors| ChatParser::parse_pho_word(&ts, "(1:02.5)", origin, errors),
                |errors| re2c.parse_pho_word("(1:02.5)", origin, errors),
            ),
        );
    }
    assert_eq!(
        tier_witnesses, 2,
        "both authored phonology tiers were exercised"
    );
    batch.finish()
}
