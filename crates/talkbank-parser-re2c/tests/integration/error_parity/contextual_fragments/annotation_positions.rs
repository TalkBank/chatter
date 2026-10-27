//! Isolated annotation-position faults must survive each public fragment route.

use super::{
    CaseReport, Detection, ExampleOrdinal, Invocation, SpecLabel, clean, deck::Api, parse_pair,
    source_findings,
};
use crate::fixture_utils::LineEnding;
use talkbank_parser_tests::{error_specs, repo_paths};
use talkbank_spec_vocabulary::frontmatter::Claim;

#[derive(Clone, Copy)]
enum Expected {
    Clean,
    Error,
}

fn check(
    label: &str,
    input: &str,
    sample: super::Sample,
    expected: Expected,
    failures: &mut Vec<String>,
) {
    source_findings(label, input, &sample, failures);
    match expected {
        Expected::Clean => clean(label, &sample, failures),
        Expected::Error => {
            let [tree_sitter, re2c] = sample.reports;
            let report = CaseReport {
                label: SpecLabel::new(label, ExampleOrdinal::from_zero_based(0)),
                tree_sitter,
                re2c,
            };
            if !matches!(report.detection(), Detection::Reported) {
                failures.push(format!("{label} at {}: {}", sample.origin, report.detail()));
            }
        }
    }
}

#[test]
fn annotation_positions_are_not_silently_dropped_by_fragment_apis() -> Result<(), String> {
    let specs = error_specs::load(repo_paths::workspace_root())?;
    let spec = specs
        .iter()
        .find(|spec| spec.filename == "E316_annotation_positions.md")
        .ok_or("annotation position specification missing")?;
    let primary = talkbank_parser::TreeSitterParser::new().map_err(|error| error.to_string())?;
    let native = talkbank_parser_re2c::Re2cParser::new();
    let mut failures = Vec::new();
    let mut observed = 0;
    for (index, example) in spec.examples().iter().enumerate() {
        // This specification deliberately gives each isolated fault one main
        // tier. Select those exact source bytes, retaining the authored newline.
        let mut mains = example
            .chat
            .as_str()
            .split_inclusive('\n')
            .filter(|line| line.starts_with("*CHI:\t"));
        let main = mains.next().ok_or("missing authored main tier")?;
        if mains.next().is_some() {
            return Err("annotation position case must have one main tier".to_owned());
        }
        let expected = match example.claim {
            Claim::Legal => Expected::Clean,
            Claim::Violates | Claim::SubsumedBy(_) => Expected::Error,
        };
        for ending in LineEnding::ALL {
            let input = ending.apply(main);
            for api in [Api::Main, Api::Utterance] {
                let label = format!("{} example {} {api:?} {ending:?}", spec.filename, index + 1);
                for sample in parse_pair((&primary, &native), api, Invocation::Implicit, &input) {
                    check(&label, &input, sample, expected, &mut failures);
                    observed += 1;
                }
            }
        }
    }
    // A legal word WITH attached syntax is still not a standalone lexical word.
    // Select both controls from the first authored example, not a new transcript.
    let source = spec
        .examples()
        .first()
        .ok_or("empty annotation specification")?
        .chat
        .as_str();
    let line = source
        .split_inclusive('\n')
        .find(|line| line.starts_with("*CHI:\t"))
        .ok_or("word control missing")?;
    let attached = line
        .strip_prefix("*CHI:\t")
        .and_then(|body| body.strip_suffix(" .\n"))
        .ok_or("word control boundary changed")?;
    let (word, _) = attached
        .split_once(" [: ")
        .ok_or("word replacement control changed")?;
    for (input, expected) in [(word, Expected::Clean), (attached, Expected::Error)] {
        for sample in parse_pair((&primary, &native), Api::Word, Invocation::Implicit, input) {
            check(
                "standalone word excludes attached annotations",
                input,
                sample,
                expected,
                &mut failures,
            );
            observed += 1;
        }
    }
    assert!(
        observed > 4,
        "the annotation position population must be present"
    );
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}
