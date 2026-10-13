use super::{Api, INVOCATIONS, Options, clean, parse_pair, source_findings};
use talkbank_model::{ErrorCollector, ParseOutcome};

/// The declared concrete word API already interprets CA context. Ordinary
/// words cannot detect a trait adapter that drops that information: retain
/// contextual shapes and independent category/content witnesses too.
#[test]
fn word_context_preserves_declared_interpretation_and_coordinates() -> Result<(), String> {
    let left = talkbank_parser::TreeSitterParser::new().map_err(|error| error.to_string())?;
    let right = talkbank_parser_re2c::Re2cParser::new();
    let mut failures = Vec::new();
    let inputs = ["(café)", "⌊(café)", "(be)cause", "0(café)"];
    let mut controls = 0;
    for invocation in INVOCATIONS {
        for (index, input) in inputs.into_iter().enumerate() {
            for sample in parse_pair((&left, &right), Api::Word, invocation, input) {
                let label = format!("word interpretation {invocation:?} {input:?}");
                controls += 1;
                clean(&label, &sample, &mut failures);
                source_findings(&label, input, &sample, &mut failures);
                if invocation.ca() && index < 2 {
                    let errors = ErrorCollector::new();
                    let context = Options::Ca.context();
                    let expected = left.parse_word_fragment_with_context(
                        input,
                        sample.origin,
                        &context,
                        &errors,
                    );
                    match expected {
                        ParseOutcome::Parsed(word) => {
                            if word.category != Some(talkbank_model::WordCategory::CAOmission)
                                || word.content().iter().any(|content| {
                                    matches!(content, talkbank_model::WordContent::Shortening(_))
                                })
                            {
                                failures.push(format!("{label}: declared CA interpretation was not an omission with spoken content"));
                            }
                        }
                        ParseOutcome::Rejected => failures.push(format!(
                            "{label}: declared concrete word API rejected its control"
                        )),
                    }
                }
            }
        }
    }
    println!(
        "measured {controls} contextual word controls; {} failed observations",
        failures.len()
    );
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}
