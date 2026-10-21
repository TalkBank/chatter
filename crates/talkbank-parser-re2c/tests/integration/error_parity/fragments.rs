//! Isolated native API boundaries, measured independently of whole-file recovery.
//! The paired carriers derive from authored error specs. No document scaffolding
//! is injected into a fragment and no unrelated fault supplies its rejection.

use super::model::{
    BackendReport, CaseReport, Detection, ExampleOrdinal, ParsingFindings, SpecLabel,
    ValidationFindings,
};
use talkbank_model::{ChatParser, ErrorCollector, ParseOutcome, Severity};

#[derive(Clone, Copy, Debug)]
enum Route {
    Header,
    Main,
    Utterance,
    Word,
    MorTier,
    MorWord,
    GraTier,
    GraRelation,
    PhoTier,
    PhoWord,
    Sin,
    Com,
    Act,
    Wor,
    Dependent,
}

fn parse(
    parser: &impl ChatParser,
    route: Route,
    input: &str,
    offset: usize,
    sink: &ErrorCollector,
) -> bool {
    match route {
        Route::Header => matches!(
            parser.parse_header(input, offset, sink),
            ParseOutcome::Parsed(_)
        ),
        Route::Main => matches!(
            parser.parse_main_tier(input, offset, sink),
            ParseOutcome::Parsed(_)
        ),
        Route::Utterance => matches!(
            parser.parse_utterance(input, offset, sink),
            ParseOutcome::Parsed(_)
        ),
        Route::Word => matches!(
            parser.parse_word(input, offset, sink),
            ParseOutcome::Parsed(_)
        ),
        Route::MorTier => matches!(
            parser.parse_mor_tier(input, offset, sink),
            ParseOutcome::Parsed(_)
        ),
        Route::MorWord => matches!(
            parser.parse_mor_word(input, offset, sink),
            ParseOutcome::Parsed(_)
        ),
        Route::GraTier => matches!(
            parser.parse_gra_tier(input, offset, sink),
            ParseOutcome::Parsed(_)
        ),
        Route::GraRelation => matches!(
            parser.parse_gra_relation(input, offset, sink),
            ParseOutcome::Parsed(_)
        ),
        Route::PhoTier => matches!(
            parser.parse_pho_tier(input, offset, sink),
            ParseOutcome::Parsed(_)
        ),
        Route::PhoWord => matches!(
            parser.parse_pho_word(input, offset, sink),
            ParseOutcome::Parsed(_)
        ),
        Route::Sin => matches!(
            parser.parse_sin_tier(input, offset, sink),
            ParseOutcome::Parsed(_)
        ),
        Route::Com => matches!(
            parser.parse_com_tier(input, offset, sink),
            ParseOutcome::Parsed(_)
        ),
        Route::Act => matches!(
            parser.parse_act_tier(input, offset, sink),
            ParseOutcome::Parsed(_)
        ),
        Route::Wor => matches!(
            parser.parse_wor_tier(input, offset, sink),
            ParseOutcome::Parsed(_)
        ),
        Route::Dependent => matches!(
            parser.parse_dependent_tier(input, offset, sink),
            ParseOutcome::Parsed(_)
        ),
    }
}

fn observe(sink: ErrorCollector) -> BackendReport {
    BackendReport::new(
        ParsingFindings::from_sink(sink),
        ValidationFindings::from_sink(ErrorCollector::new()),
    )
}

/// Each mutation has an independently admitted, error-free control. This deck
/// measures parsing only, not contextual validation or all supported APIs.
#[test]
fn isolated_faults_require_errors_at_supported_fragment_boundaries() -> Result<(), String> {
    let ts = talkbank_parser::TreeSitterParser::new().map_err(|error| error.to_string())?;
    let re2c = talkbank_parser_re2c::Re2cParser::new();
    let pairs = [
        (
            Route::Header,
            "E316",
            "@Languages:\teng, spa\n",
            "@Languages:\teng,, spa\n",
        ),
        (
            Route::Header,
            "W109_recovery",
            "@Media:\tSchlüssel, audio, unlinked\n",
            "@Media:\tSchlüssel,, audio, unlinked\n",
        ),
        (Route::Header, "E530", "@G:\tscene\n", "@G:\t\n"),
        (
            Route::Main,
            "E207",
            "*CHI:\thello &-uh world .\n",
            "*CHI:\thello & world .\n",
        ),
        (
            Route::Main,
            "E375",
            "*CHI:\they [/] hey .\n",
            "*CHI:\they[/] hey .\n",
        ),
        (
            Route::Main,
            "E375",
            "*CHI:\tdog [/] (.) dog .\n",
            "*CHI:\tdog (.) [/] dog .\n",
        ),
        (
            Route::Utterance,
            "E207",
            "*CHI:\thello &-uh world .\n",
            "*CHI:\thello & world .\n",
        ),
        (Route::Word, "E202_missing_form_type", "hello@f", "hello@"),
        (Route::Word, "E203", "word@z:grm", "word@z"),
        (Route::Word, "E203", "hello@f", "hello@f@s:eng"),
        (Route::Word, "E207", "&-uh", "&"),
        (Route::MorTier, "E305_mor_terminator", "n|dog .", "n|dog"),
        (Route::MorWord, "E760", "n|dog", "|dog"),
        (
            Route::GraTier,
            "E316_dependent_composition",
            "1|0|ROOT",
            "1|0|ROOT \u{15}",
        ),
        (Route::GraRelation, "E709", "1|0|ROOT", "1|0|ROOT 2|1|OBJ"),
        (Route::PhoTier, "E316", "kːæt", "k:æt"),
        (Route::PhoWord, "E316", "kːæt", "k:æt"),
        (
            Route::Sin,
            "E316_dependent_composition",
            "g:point",
            "g:point \u{15}",
        ),
        (
            Route::Com,
            "E316_inline_picture",
            "picture \u{15}%pic:\"image.jpg\"\u{15}",
            "picture \u{15}%pic:\"\"\u{15}",
        ),
        (
            Route::Act,
            "E330_dependent_composition",
            "timing \u{15}1_2\u{15}",
            "timing \u{15}",
        ),
        (
            Route::Act,
            "E360",
            "timing \u{15}0_1\u{15}",
            "timing \u{15}18446744073709551616_1\u{15}",
        ),
        (
            Route::Com,
            "E360",
            "timing \u{15}0_1\u{15}",
            "timing \u{15}0_18446744073709551616\u{15}",
        ),
        (Route::Wor, "E207", "hello &-uh .", "hello & ."),
        (
            Route::Dependent,
            "E330_dependent_composition",
            "%com:\thello\n",
            "%com:\thello \u{15}\n",
        ),
    ];
    let mut failures = Vec::new();
    let mut required = 0;
    let mut not_required = 0;
    for offset in [0, "@Comment:\tPréface café\n".len()] {
        for (index, (route, spec, control, mutation)) in pairs.iter().enumerate() {
            let ts_control = ErrorCollector::new();
            let re_control = ErrorCollector::new();
            let ts_ok = parse(&ts, *route, control, offset, &ts_control);
            let re_ok = parse(&re2c, *route, control, offset, &re_control);
            let ts_control = observe(ts_control);
            let re_control = observe(re_control);
            if !ts_ok
                || !re_ok
                || ts_control.diagnostics().next().is_some()
                || re_control.diagnostics().next().is_some()
            {
                failures.push(format!("{spec} {route:?} control at {offset}: parsed TS={ts_ok} re2c={re_ok}; TS {ts_control}; re2c {re_control}"));
            }
            let ts_sink = ErrorCollector::new();
            let re_sink = ErrorCollector::new();
            parse(&ts, *route, mutation, offset, &ts_sink);
            parse(&re2c, *route, mutation, offset, &re_sink);
            let case = CaseReport {
                label: SpecLabel::new(
                    "fragment-control-deck",
                    ExampleOrdinal::from_zero_based(index),
                ),
                tree_sitter: observe(ts_sink),
                re2c: observe(re_sink),
            };
            match case.detection() {
                Detection::Reported => required += 1,
                // File and fragment APIs need not emit the same diagnostics.
                Detection::NotRequired => not_required += 1,
                Detection::Missing(_) | Detection::ToolFailure { .. } => {
                    failures.push(format!("{spec} {route:?}: {} at {offset}", case.detail()))
                }
            }
            for error in case
                .re2c
                .diagnostics()
                .filter(|error| error.severity == Severity::Error)
            {
                let span = error.location.span().expect("located diagnostic");
                if (span.start()) < offset || (span.end()) > offset + mutation.len() {
                    failures.push(format!("{spec} {route:?}: error outside original caller bytes at {offset}: {span:?}"));
                }
            }
        }
    }
    if failures.is_empty() {
        println!(
            "ok {required} isolated fragment error detections; {not_required} without a tree-sitter parse-error obligation; {} independently admitted controls",
            pairs.len() * 2
        );
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}
