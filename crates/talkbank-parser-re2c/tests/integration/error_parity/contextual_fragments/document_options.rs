//! A complete single-utterance document retains its own semantic headers.

use super::{INVOCATIONS, clean, deck::Api, parse_pair, source_findings};

#[derive(Clone, Copy, Debug)]
enum DocumentOptions {
    Ca,
    Absent,
}

#[test]
fn utterance_admission_preserves_document_ca_options() -> Result<(), String> {
    let reference = include_str!(
        "../../../../../../corpus/reference/word-features/ca-omission-combinations.cha"
    );
    // Select the first authored utterance while retaining all original headers.
    // The body, including its overlap points and omissions, is not synthesized.
    let (preamble, _) = reference.split_once("*SPK:").unwrap();
    let main = reference
        .lines()
        .find(|line| line.starts_with("*SPK:"))
        .unwrap();
    let end = reference.lines().last().unwrap();
    let document = format!("{preamble}{main}\n{end}\n");
    let primary = talkbank_parser::TreeSitterParser::new().map_err(|e| e.to_string())?;
    let native = talkbank_parser_re2c::Re2cParser::new();
    let mut failures = Vec::new();
    for options in [DocumentOptions::Ca, DocumentOptions::Absent] {
        let input = match options {
            DocumentOptions::Ca => document.clone(),
            DocumentOptions::Absent => document.replace("@Options:\tCA\n", ""),
        };
        for ending in ["\n", "\r\n", "\r"] {
            let input = input.replace('\n', ending);
            for invocation in INVOCATIONS {
                for sample in parse_pair((&primary, &native), Api::Utterance, invocation, &input) {
                    let label = format!("document {options:?}, {ending:?}, {invocation:?}");
                    source_findings(&label, &input, &sample, &mut failures);
                    if matches!(options, DocumentOptions::Ca) || invocation.ca() {
                        clean(&label, &sample, &mut failures);
                    } else if sample.parsed != [false, false]
                        || sample
                            .reports
                            .iter()
                            .any(|report| report.diagnostics().next().is_none())
                    {
                        failures.push(format!("{label}: absent CA must reject shorthand"));
                    }
                }
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}
