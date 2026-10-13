//! Contextual fragment semantics, complete-input obligations and original-source
//! diagnostics. Recovered models need not agree across parser architectures.
//! Checked-range arithmetic and whole-file validity retain their own receipts.

mod deck;
mod word_context;

use super::model::{
    BackendReport, CaseReport, Detection, ExampleOrdinal, ParsingFindings, SpecLabel,
    ValidationFindings,
};
use deck::Api;
use talkbank_model::{
    ChatOptionFlag, ChatParser, ErrorCollector, FragmentSemanticContext, FragmentSource,
    ParseOutcome, SemanticEq, Severity, SpanShift,
};

#[derive(Clone, Copy, Debug)]
enum Options {
    Empty,
    NoAlign,
    Ca,
    CaNoAlign,
}

impl Options {
    fn context(self) -> FragmentSemanticContext {
        let flags = match self {
            Self::Empty => vec![],
            Self::NoAlign => vec![ChatOptionFlag::NoAlign],
            Self::Ca => vec![ChatOptionFlag::Ca],
            Self::CaNoAlign => vec![ChatOptionFlag::Ca, ChatOptionFlag::NoAlign],
        };
        FragmentSemanticContext::new().with_option_flags(flags)
    }
}

#[derive(Clone, Copy, Debug)]
enum Invocation {
    Implicit,
    Explicit(Options),
}

impl Invocation {
    fn ca(self) -> bool {
        matches!(self, Self::Explicit(Options::Ca | Options::CaNoAlign))
    }
}

const INVOCATIONS: [Invocation; 5] = [
    Invocation::Implicit,
    Invocation::Explicit(Options::Empty),
    Invocation::Explicit(Options::NoAlign),
    Invocation::Explicit(Options::Ca),
    Invocation::Explicit(Options::CaNoAlign),
];
const ORIGIN: usize = "@Comment:\tPréface café\n".len();

struct Sample {
    origin: usize,
    parsed: [bool; 2],
    same_meaning: bool,
    rebased: [bool; 2],
    diagnostics_rebased: [bool; 2],
    reports: [BackendReport; 2],
}

fn observe(sink: ErrorCollector) -> BackendReport {
    BackendReport::new(
        ParsingFindings::from_sink(sink),
        ValidationFindings::from_sink(ErrorCollector::new()),
    )
}

fn diagnostic_rebasing(base: &BackendReport, placed: &BackendReport) -> bool {
    let move_span = |span: talkbank_model::Span| {
        talkbank_model::Span::new(span.start + ORIGIN as u32, span.end + ORIGIN as u32)
    };
    let base = base.diagnostics().collect::<Vec<_>>();
    let placed = placed.diagnostics().collect::<Vec<_>>();
    base.len() == placed.len()
        && base.iter().zip(placed).all(|(base, placed)| {
            base.code == placed.code
                && base.severity == placed.severity
                && move_span(base.location.span) == placed.location.span
                && base.context == placed.context
                && base.labels.len() == placed.labels.len()
                && base
                    .labels
                    .iter()
                    .zip(&placed.labels)
                    .all(|(base, placed)| move_span(base.span) == placed.span)
        })
}

/// Compare coordinates within one backend, not recovered models between them.
fn rebases<T: PartialEq + SpanShift>(
    base: ParseOutcome<T>,
    placed: &ParseOutcome<T>,
    source: FragmentSource<'_>,
) -> bool {
    match (base, placed) {
        (ParseOutcome::Parsed(base), ParseOutcome::Parsed(placed)) => {
            source.rebase(base) == *placed
        }
        (ParseOutcome::Rejected, ParseOutcome::Rejected) => true,
        (ParseOutcome::Parsed(_), ParseOutcome::Rejected)
        | (ParseOutcome::Rejected, ParseOutcome::Parsed(_)) => false,
    }
}

fn measure<T: SemanticEq + PartialEq + SpanShift>(
    input: &str,
    parse: impl Fn(usize, &ErrorCollector, &ErrorCollector) -> (ParseOutcome<T>, ParseOutcome<T>),
) -> [Sample; 2] {
    let left_sink = ErrorCollector::new();
    let right_sink = ErrorCollector::new();
    let (left, right) = parse(0, &left_sink, &right_sink);
    let first = Sample {
        origin: 0,
        parsed: [left.is_parsed(), right.is_parsed()],
        same_meaning: left.semantic_eq(&right),
        rebased: [true, true],
        diagnostics_rebased: [true, true],
        reports: [observe(left_sink), observe(right_sink)],
    };
    let left_sink = ErrorCollector::new();
    let right_sink = ErrorCollector::new();
    let (placed_left, placed_right) = parse(ORIGIN, &left_sink, &right_sink);
    let source = FragmentSource::new(input, ORIGIN).expect("small fixture range");
    let reports = [observe(left_sink), observe(right_sink)];
    let second = Sample {
        origin: ORIGIN,
        parsed: [placed_left.is_parsed(), placed_right.is_parsed()],
        same_meaning: placed_left.semantic_eq(&placed_right),
        rebased: [
            rebases(left, &placed_left, source),
            rebases(right, &placed_right, source),
        ],
        diagnostics_rebased: [
            diagnostic_rebasing(&first.reports[0], &reports[0]),
            diagnostic_rebasing(&first.reports[1], &reports[1]),
        ],
        reports,
    };
    [first, second]
}

fn parse_pair(
    (left, right): (&impl ChatParser, &impl ChatParser),
    api: Api,
    invocation: Invocation,
    input: &str,
) -> [Sample; 2] {
    macro_rules! invoke {
        ($ordinary:ident, $explicit:ident) => {
            measure(input, |origin, left_sink, right_sink| match invocation {
                Invocation::Implicit => (
                    left.$ordinary(input, origin, left_sink),
                    right.$ordinary(input, origin, right_sink),
                ),
                Invocation::Explicit(options) => {
                    let context = options.context();
                    (
                        left.$explicit(input, origin, &context, left_sink),
                        right.$explicit(input, origin, &context, right_sink),
                    )
                }
            })
        };
    }
    match api {
        Api::Header => invoke!(parse_header, parse_header_with_context),
        Api::Id => invoke!(parse_id_header, parse_id_header_with_context),
        Api::Participant => invoke!(
            parse_participant_entry,
            parse_participant_entry_with_context
        ),
        Api::Utterance => invoke!(parse_utterance, parse_utterance_with_context),
        Api::Main => invoke!(parse_main_tier, parse_main_tier_with_context),
        Api::Word => invoke!(parse_word, parse_word_with_context),
        Api::MorTier => invoke!(parse_mor_tier, parse_mor_tier_with_context),
        Api::MorWord => invoke!(parse_mor_word, parse_mor_word_with_context),
        Api::GraTier => invoke!(parse_gra_tier, parse_gra_tier_with_context),
        Api::GraRelation => invoke!(parse_gra_relation, parse_gra_relation_with_context),
        Api::PhoTier => invoke!(parse_pho_tier, parse_pho_tier_with_context),
        Api::PhoWord => invoke!(parse_pho_word, parse_pho_word_with_context),
        Api::Sin => invoke!(parse_sin_tier, parse_sin_tier_with_context),
        Api::Act => invoke!(parse_act_tier, parse_act_tier_with_context),
        Api::Cod => invoke!(parse_cod_tier, parse_cod_tier_with_context),
        Api::Com => invoke!(parse_com_tier, parse_com_tier_with_context),
        Api::Exp => invoke!(parse_exp_tier, parse_exp_tier_with_context),
        Api::Add => invoke!(parse_add_tier, parse_add_tier_with_context),
        Api::Gpx => invoke!(parse_gpx_tier, parse_gpx_tier_with_context),
        Api::Int => invoke!(parse_int_tier, parse_int_tier_with_context),
        Api::Spa => invoke!(parse_spa_tier, parse_spa_tier_with_context),
        Api::Sit => invoke!(parse_sit_tier, parse_sit_tier_with_context),
        Api::Wor => invoke!(parse_wor_tier, parse_wor_tier_with_context),
        Api::Dependent => invoke!(parse_dependent_tier, parse_dependent_tier_with_context),
    }
}

fn source_findings(label: &str, input: &str, sample: &Sample, failures: &mut Vec<String>) {
    for (backend, report) in ["tree-sitter", "re2c"].into_iter().zip(&sample.reports) {
        for finding in report.diagnostics() {
            let owns = |span: talkbank_model::Span| {
                (span.start as usize) >= sample.origin
                    && (span.end as usize) <= sample.origin + input.len()
                    && span.start <= span.end
            };
            if !owns(finding.location.span) || finding.labels.iter().any(|label| !owns(label.span))
            {
                failures.push(format!(
                    "{label} at {}: {backend} diagnostic/label outside caller: {finding:?}",
                    sample.origin
                ));
            }
            if let Some(context) = &finding.context
                && (!input.contains(context.source_text.as_str())
                    || !context
                        .source_text
                        .is_char_boundary(context.span.start as usize)
                    || !context
                        .source_text
                        .is_char_boundary(context.span.end as usize))
            {
                failures.push(format!(
                    "{label} at {}: {backend} diagnostic context is not caller-owned: {finding:?}",
                    sample.origin
                ));
            }
        }
    }
    if sample.rebased != [true, true] {
        failures.push(format!(
            "{label} at {}: model rebasing TS={} re2c={}",
            sample.origin, sample.rebased[0], sample.rebased[1]
        ));
    }
    if sample.diagnostics_rebased != [true, true] {
        failures.push(format!(
            "{label} at {}: diagnostic rebasing TS={} re2c={}",
            sample.origin, sample.diagnostics_rebased[0], sample.diagnostics_rebased[1]
        ));
    }
}

fn clean(label: &str, sample: &Sample, failures: &mut Vec<String>) {
    if sample.parsed != [true, true]
        || !sample.same_meaning
        || sample
            .reports
            .iter()
            .any(|report| report.diagnostics().next().is_some())
    {
        failures.push(format!(
            "{label} at {}: clean semantic control TS={} re2c={} same meaning={}; TS {}; re2c {}",
            sample.origin,
            sample.parsed[0],
            sample.parsed[1],
            sample.same_meaning,
            sample.reports[0],
            sample.reports[1]
        ));
    }
}

/// Every row is measured, including mutations with no error antecedent. Do not
/// reinterpret their observed parsing behavior as whole-file validity.
#[test]
fn contextual_matrix_preserves_meaning_detection_and_source_ownership() -> Result<(), String> {
    let left = talkbank_parser::TreeSitterParser::new().map_err(|error| error.to_string())?;
    let right = talkbank_parser_re2c::Re2cParser::new();
    let mut failures = Vec::new();
    let mut controls = 0;
    let mut required = 0;
    let mut not_required = 0;
    for invocation in INVOCATIONS {
        for (index, (api, control, structural_fault)) in deck::BASE.iter().enumerate() {
            let label = format!("{api:?} {invocation:?}");
            for sample in parse_pair((&left, &right), *api, invocation, control) {
                controls += 1;
                clean(&format!("{label} control"), &sample, &mut failures);
                source_findings(&label, control, &sample, &mut failures);
            }
            let nul = format!("{control}\0ignored");
            let vertical_tab = format!("{control}\u{b}");
            let trailing = format!("{control}\n@Comment:\textra\n");
            for (kind, input) in [
                ("empty", ""),
                ("structural", *structural_fault),
                ("embedded NUL", nul.as_str()),
                ("forbidden control", vertical_tab.as_str()),
                ("trailing source", trailing.as_str()),
            ] {
                for sample in parse_pair((&left, &right), *api, invocation, input) {
                    let label = format!("{label} {kind}");
                    source_findings(&label, input, &sample, &mut failures);
                    let [tree_sitter, re2c] = sample.reports;
                    let case = CaseReport {
                        label: SpecLabel::new(
                            "contextual-fragment-deck",
                            ExampleOrdinal::from_zero_based(index),
                        ),
                        tree_sitter,
                        re2c,
                    };
                    match case.detection() {
                        Detection::Reported => required += 1,
                        Detection::NotRequired => not_required += 1,
                        Detection::Missing(_) | Detection::ToolFailure { .. } => failures
                            .push(format!("{label} at {}: {}", sample.origin, case.detail())),
                    }
                }
            }
        }
        for (api, input) in deck::OMISSIONS {
            let label = format!("{api:?} {invocation:?} contextual omission");
            for sample in parse_pair((&left, &right), *api, invocation, input) {
                source_findings(&label, input, &sample, &mut failures);
                if invocation.ca() {
                    controls += 1;
                    clean(&label, &sample, &mut failures);
                } else {
                    for (backend, report) in
                        ["tree-sitter", "re2c"].into_iter().zip(&sample.reports)
                    {
                        if !report
                            .diagnostics()
                            .any(|finding| finding.severity == Severity::Error)
                        {
                            failures.push(format!(
                                "{label} at {}: {backend} missing required context refusal",
                                sample.origin
                            ));
                        }
                    }
                }
            }
        }
        for (api, control) in deck::NOTATION {
            let label = format!("{api:?} {invocation:?} independent notation");
            for sample in parse_pair((&left, &right), *api, invocation, control) {
                controls += 1;
                clean(&label, &sample, &mut failures);
                source_findings(&label, control, &sample, &mut failures);
            }
            // CRLF is a distinct caller source, not a normalization before parsing.
            let crlf = control.replace('\n', "\r\n");
            for sample in parse_pair((&left, &right), *api, invocation, &crlf) {
                controls += 1;
                clean(&format!("{label} CRLF"), &sample, &mut failures);
                source_findings(&label, &crlf, &sample, &mut failures);
            }
        }
    }
    println!(
        "measured {controls} clean semantic controls; {required} detected obligations; {not_required} mutations without a parse-error antecedent; {} failed observations",
        failures.len()
    );
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}
