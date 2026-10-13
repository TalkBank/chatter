//! The nine remaining shared fragment APIs, plus singular utterance admission.
//! Controls use the authored ID/participant and dependent-text carriers. Each
//! fault changes only cardinality, one delimiter, or a trailing control byte.
//! Explicit context uses the same empty-option policy as the implicit route;
//! this is not proof of every context-sensitive policy or every possible fault.

use super::model::{
    BackendReport, CaseReport, Detection, ExampleOrdinal, ParsingFindings, SpecLabel,
    ValidationFindings,
};
use talkbank_model::{ChatParser, ErrorCollector, FragmentSemanticContext, SemanticEq, Severity};

#[derive(Clone, Copy, Debug)]
enum Api {
    Id,
    Participant,
    Cod,
    Exp,
    Add,
    Gpx,
    Int,
    Spa,
    Sit,
    Utterance,
}

#[derive(Clone, Copy, Debug)]
enum Invocation {
    Implicit,
    Explicit,
}

#[derive(Clone, Copy, Debug)]
enum Fault {
    Shape,
    TrailingNul,
}

struct ModelObservation {
    tree_sitter_parsed: bool,
    re2c_parsed: bool,
    same_meaning: bool,
}

fn parse_pair(
    (tree_sitter, re2c): (&impl ChatParser, &impl ChatParser),
    api: Api,
    invocation: Invocation,
    input: &str,
    offset: usize,
    (ts_sink, re_sink): (&ErrorCollector, &ErrorCollector),
) -> ModelObservation {
    let context = FragmentSemanticContext::new();
    macro_rules! invoke {
        ($ordinary:ident, $explicit:ident) => {{
            let left = match invocation {
                Invocation::Implicit => tree_sitter.$ordinary(input, offset, ts_sink),
                Invocation::Explicit => tree_sitter.$explicit(input, offset, &context, ts_sink),
            };
            let right = match invocation {
                Invocation::Implicit => re2c.$ordinary(input, offset, re_sink),
                Invocation::Explicit => re2c.$explicit(input, offset, &context, re_sink),
            };
            ModelObservation {
                tree_sitter_parsed: left.is_parsed(),
                re2c_parsed: right.is_parsed(),
                same_meaning: left.semantic_eq(&right),
            }
        }};
    }
    match api {
        Api::Id => invoke!(parse_id_header, parse_id_header_with_context),
        Api::Participant => invoke!(
            parse_participant_entry,
            parse_participant_entry_with_context
        ),
        Api::Cod => invoke!(parse_cod_tier, parse_cod_tier_with_context),
        Api::Exp => invoke!(parse_exp_tier, parse_exp_tier_with_context),
        Api::Add => invoke!(parse_add_tier, parse_add_tier_with_context),
        Api::Gpx => invoke!(parse_gpx_tier, parse_gpx_tier_with_context),
        Api::Int => invoke!(parse_int_tier, parse_int_tier_with_context),
        Api::Spa => invoke!(parse_spa_tier, parse_spa_tier_with_context),
        Api::Sit => invoke!(parse_sit_tier, parse_sit_tier_with_context),
        Api::Utterance => invoke!(parse_utterance, parse_utterance_with_context),
    }
}

fn observe(sink: ErrorCollector) -> BackendReport {
    BackendReport::new(
        ParsingFindings::from_sink(sink),
        ValidationFindings::from_sink(ErrorCollector::new()),
    )
}

/// Keep parsing and contextual validation distinct. Here both overloads use
/// identical empty-option parsing policy on the original caller's bytes.
#[test]
fn remaining_apis_preserve_input_shape_and_error_detection() -> Result<(), String> {
    let ts = talkbank_parser::TreeSitterParser::new().map_err(|error| error.to_string())?;
    let re2c = talkbank_parser_re2c::Re2cParser::new();
    let cases = [
        (
            Api::Id,
            "@ID:\teng|corpus|CHI|||||Child|||\n",
            "@ID:\teng|corpus|CHI|||||Child||\n",
        ),
        (
            Api::Participant,
            "CHI Target_Child",
            "CHI Target_Child, MOT Mother",
        ),
        (Api::Cod, "hello", "hello \u{15}"),
        (Api::Exp, "hello", "hello \u{15}"),
        (Api::Add, "hello", "hello \u{15}"),
        (Api::Gpx, "hello", "hello \u{15}"),
        (Api::Int, "hello", "hello \u{15}"),
        (Api::Spa, "hello", "hello \u{15}"),
        (Api::Sit, "hello", "hello \u{15}"),
        (
            Api::Utterance,
            "*CHI:\thello .\n",
            "*CHI:\thello .\n*CHI:\tworld .\n",
        ),
    ];
    let mut failures = Vec::new();
    let mut controls = 0;
    let mut required = 0;
    let mut not_required = 0;
    for offset in [0, "@Comment:\tPréface café\n".len()] {
        for invocation in [Invocation::Implicit, Invocation::Explicit] {
            for (index, (api, control, shape_fault)) in cases.iter().enumerate() {
                let ts_sink = ErrorCollector::new();
                let re_sink = ErrorCollector::new();
                let models = parse_pair(
                    (&ts, &re2c),
                    *api,
                    invocation,
                    control,
                    offset,
                    (&ts_sink, &re_sink),
                );
                let ts_control = observe(ts_sink);
                let re_control = observe(re_sink);
                controls += 1;
                if !models.tree_sitter_parsed
                    || !models.re2c_parsed
                    || !models.same_meaning
                    || ts_control.diagnostics().next().is_some()
                    || re_control.diagnostics().next().is_some()
                {
                    failures.push(format!("{api:?} {invocation:?} control at {offset}: TS parsed={} {ts_control}; re2c parsed={} {re_control}; same meaning={}",
                        models.tree_sitter_parsed, models.re2c_parsed, models.same_meaning));
                }
                let nul_fault = format!("{control}\0");
                for (fault, input) in [
                    (Fault::Shape, *shape_fault),
                    (Fault::TrailingNul, nul_fault.as_str()),
                ] {
                    let ts_sink = ErrorCollector::new();
                    let re_sink = ErrorCollector::new();
                    // Recovered fault-model identity is intentionally not an
                    // acceptance condition; only the sinks issue obligations.
                    parse_pair(
                        (&ts, &re2c),
                        *api,
                        invocation,
                        input,
                        offset,
                        (&ts_sink, &re_sink),
                    );
                    let case = CaseReport {
                        label: SpecLabel::new(
                            "remaining-fragment-control-deck",
                            ExampleOrdinal::from_zero_based(index),
                        ),
                        tree_sitter: observe(ts_sink),
                        re2c: observe(re_sink),
                    };
                    match case.detection() {
                        Detection::Reported => required += 1,
                        Detection::NotRequired => not_required += 1,
                        Detection::Missing(_) | Detection::ToolFailure { .. } => {
                            failures.push(format!(
                                "{api:?} {invocation:?} {fault:?} at {offset}: {}",
                                case.detail()
                            ))
                        }
                    }
                    for error in case
                        .re2c
                        .diagnostics()
                        .filter(|error| error.severity == Severity::Error)
                    {
                        let span = error.location.span;
                        if (span.start as usize) < offset
                            || (span.end as usize) > offset + input.len()
                        {
                            failures.push(format!("{api:?} {invocation:?} {fault:?}: diagnostic outside caller at {offset}: {span:?}"));
                        }
                    }
                }
            }
        }
    }
    if failures.is_empty() {
        println!(
            "ok {controls} original-input controls, {required} required detections, {not_required} without a tree-sitter parse-error antecedent"
        );
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}
