//! One-way error detection over original-source retained spec headers.

use super::{
    model::{
        BackendReport, CaseReport, Detection, ExampleOrdinal, ParsingFindings, SpecLabel,
        ValidationFindings,
    },
    spec_corpus::load_spec_corpus,
};
use crate::fixture_utils::LineEnding;
use std::collections::{BTreeMap, btree_map::Entry};
use talkbank_model::SpanShift;
use talkbank_model::validation::{Validate, ValidationContext};
use talkbank_model::{
    ChatParser, CompletedDiagnostics, ErrorCollector, FragmentSemanticContext, Line, ParseOutcome,
};

/// A retained header's original bytes and document origin, admitted together.
struct HeaderFragment<'source> {
    input: &'source str,
    origin: usize,
}

impl<'source> HeaderFragment<'source> {
    fn admit(source: &'source str, location: talkbank_model::Location) -> Result<Self, String> {
        let span = location
            .span()
            .ok_or("retained header has no source extent")?;
        let input = source
            .get(span.to_range())
            .filter(|text| !text.is_empty())
            .ok_or_else(|| format!("invalid retained header extent {span:?}"))?;
        Ok(Self {
            input,
            origin: span.start(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Invocation {
    Implicit,
    Explicit,
}

/// The complete input to an observation under the fixed header-validation
/// policy. Repeated source occurrences reuse it without dropping any distinct
/// bytes, origin or public API invocation.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct HeaderProbe {
    input: String,
    origin: usize,
    invocation: Invocation,
}

fn measure(parser: &impl ChatParser, probe: &HeaderProbe) -> Result<BackendReport, String> {
    let HeaderProbe {
        input,
        origin,
        invocation,
    } = probe;
    let origin = *origin;
    let errors = ErrorCollector::new();
    let outcome = match invocation {
        Invocation::Implicit => parser.parse_header(input, origin, &errors),
        Invocation::Explicit => parser.parse_header_with_context(
            input,
            origin,
            &FragmentSemanticContext::new(),
            &errors,
        ),
    };
    if matches!(outcome, ParseOutcome::Rejected) && !errors.has_errors() {
        return Err(format!(
            "{invocation:?} header rejection has no error: {:?}",
            input
        ));
    }
    let validation = ErrorCollector::new();
    if let ParseOutcome::Parsed(header) = outcome {
        let end = origin
            .checked_add(input.len())
            .ok_or("header origin overflow")?;
        let header = header.try_map_spans(&mut |span: talkbank_model::Span| {
            if span.start() >= origin && span.end() <= end {
                Ok(span)
            } else {
                Err(format!(
                    "header payload span {span:?} escapes {origin}..{end}"
                ))
            }
        })?;
        header.validate(&ValidationContext::default(), &validation);
    }
    Ok(BackendReport::new(
        ParsingFindings::from_sink(errors),
        ValidationFindings::from_sink(validation),
    ))
}

#[test]
fn retained_spec_headers_preserve_error_detection_through_fragment_apis() -> Result<(), String> {
    let corpus = load_spec_corpus()?;
    let primary = talkbank_parser::TreeSitterParser::new().map_err(|error| error.to_string())?;
    let native = talkbank_parser_re2c::Re2cParser::new();
    let mut failures = Vec::new();
    let mut probes = BTreeMap::new();
    let mut headers = 0;
    let mut required = 0;
    let mut comparisons = 0;
    let mut requested = 0;
    for case in &corpus.cases {
        for ending in LineEnding::ALL {
            let input = case.input.with_line_ending(ending);
            let errors = ErrorCollector::new();
            let document = primary.parse_chat_file_streaming(input.text(), &errors);
            CompletedDiagnostics::admit(errors.into_vec()).map_err(|error| error.to_string())?;
            for (index, line) in document.lines.iter().enumerate() {
                let Line::Header { location, .. } = line else {
                    continue;
                };
                let fragment = HeaderFragment::admit(input.text(), *location)?;
                headers += 1;
                for origin in [0, fragment.origin] {
                    for invocation in [Invocation::Implicit, Invocation::Explicit] {
                        requested += 1;
                        let Entry::Vacant(slot) = probes.entry(HeaderProbe {
                            input: fragment.input.to_owned(),
                            origin,
                            invocation,
                        }) else {
                            continue;
                        };
                        let label = format!(
                            "{} header {} {ending:?} {invocation:?} origin {origin}",
                            case.label,
                            index + 1
                        );
                        let report = CaseReport {
                            label: SpecLabel::new(&label, ExampleOrdinal::from_zero_based(0)),
                            tree_sitter: measure(&primary, slot.key())?,
                            re2c: measure(&native, slot.key())?,
                        };
                        slot.insert(());
                        comparisons += 1;
                        match report.detection() {
                            Detection::Reported => required += 1,
                            Detection::NotRequired => {}
                            Detection::Missing(_) => failures.push(report.detail()),
                            Detection::ToolFailure { finding, .. } => {
                                failures.push(format!("{}: {}", report.detail(), finding.message))
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        headers > 0 && required > 0,
        "header error population must not be empty"
    );
    println!(
        "{headers} retained source headers, {comparisons} distinct comparisons, {} identical observations reused, {required} required detections",
        requested - comparisons,
    );
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}
