use super::*;
use tower_lsp::lsp_types::NumberOrString;

#[test]
fn reports_file_level_timing_rule_from_shared_validator() {
    let source = include_str!(
        "../../../../talkbank-parser-tests/tests/error_corpus/validation_errors/E752_1.cha"
    );
    let parser = TreeSitterParser::new().unwrap();
    let uri = Url::parse("file:///timing.cha").unwrap();
    let analysis = DocumentAnalysis::parse(&parser, &uri, source.into(), None);
    assert!(
        analysis
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == Some(NumberOrString::String("E752".into())))
    );
}

const REFERENCE: &str = include_str!("../../../../../corpus/reference/core/basic-conversation.cha");

// Differential contract: edited tree-sitter reuse and fresh parsing must agree
// on the complete model (including spans) and the published diagnostics.
#[test]
fn edit_sequence_matches_fresh_analysis() {
    let parser = TreeSitterParser::new().unwrap();
    let uri = Url::parse("file:///basic-conversation.cha").unwrap();
    let mut previous = DocumentAnalysis::parse(&parser, &uri, REFERENCE.into(), None);
    for (name, source) in [
        ("same shape word", REFERENCE.replace("want", "need")),
        ("Unicode length", REFERENCE.replace("cookies", "café😀")),
        (
            "line insertion",
            REFERENCE.replace("*CHI:\tI", "@Comment:\tnew line\n*CHI:\tI"),
        ),
        ("header deletion", REFERENCE.replacen("@UTF8\n", "", 1)),
        ("recovery suffix", format!("{REFERENCE}oops")),
        ("repair", REFERENCE.to_owned()),
    ] {
        let incremental =
            DocumentAnalysis::parse(&parser, &uri, source.clone().into(), Some(&previous));
        let fresh = DocumentAnalysis::parse(&parser, &uri, source.into(), None);
        assert_eq!(incremental.diagnostics(), fresh.diagnostics(), "{name}");
        assert_eq!(incremental.file(), fresh.file(), "{name}: model and spans");
        previous = incremental;
    }
}

// Manual measurement, excluded from CI. Uses the normal analysis path and
// reports computation separately from the editor's fixed debounce delay.
#[test]
#[ignore = "manual analysis latency measurement; no machine-specific CI threshold"]
fn measure_analysis_latency() {
    use std::time::Instant;
    let parser = TreeSitterParser::new().unwrap();
    let uri = Url::parse("file:///basic-conversation.cha").unwrap();
    let (headers, _) = REFERENCE.split_once("*CHI:").unwrap();
    let workloads = match std::env::var("TALKBANK_LSP_BENCH_SOURCE") {
        Ok(path) => vec![std::fs::read_to_string(path).unwrap()],
        Err(std::env::VarError::NotPresent) => [100, 1_000, 5_000]
            .into_iter()
            .map(|utterances| {
                format!(
                    "{headers}{}@End\n",
                    "*CHI:\tI want some cookies .\n".repeat(utterances)
                )
            })
            .collect(),
        Err(error) => panic!("invalid benchmark source path: {error}"),
    };
    for source in workloads {
        let utterances = source.lines().filter(|line| line.starts_with('*')).count();
        let other = source.replacen("@Begin\n", "@Begin\n@Comment:\tlatency probe\n", 1);
        assert_ne!(source, other, "benchmark input needs @Begin");
        let mut previous = DocumentAnalysis::parse(&parser, &uri, source.clone().into(), None);
        let mut durations = Vec::new();
        for index in 0..21 {
            let text = if index % 2 == 0 { &other } else { &source };
            let start = Instant::now();
            let current =
                DocumentAnalysis::parse(&parser, &uri, text.as_str().into(), Some(&previous));
            let micros = start.elapsed().as_micros();
            std::hint::black_box(current.diagnostics());
            if index != 0 {
                durations.push(micros);
            }
            previous = current;
        }
        durations.sort_unstable();
        eprintln!(
            "analysis_latency utterances={utterances} bytes={} samples={} p50_us={} p95_us={} max_us={}",
            source.len(),
            durations.len(),
            durations[9],
            durations[18],
            durations[19]
        );
    }
}

// Attribution probe, not an alternative production pipeline. Lowering includes
// a second, same-source CST parse because the public parser owns that boundary.
// Every measured result is checked against the actual source-bound analysis.
#[test]
#[ignore = "manual phase attribution; no machine-specific CI threshold"]
fn measure_analysis_phases() {
    use std::time::Instant;
    let source = match std::env::var("TALKBANK_LSP_BENCH_SOURCE") {
        Ok(path) => std::fs::read_to_string(path).unwrap(),
        Err(std::env::VarError::NotPresent) => REFERENCE.to_owned(),
        Err(error) => panic!("invalid benchmark source path: {error}"),
    };
    let other = source.replacen("@Begin\n", "@Begin\n@Comment:\tlatency probe\n", 1);
    assert_ne!(source, other, "benchmark input needs @Begin");
    let parser = TreeSitterParser::new().unwrap();
    let uri = Url::parse("file:///basic-conversation.cha").unwrap();
    let mut previous = DocumentAnalysis::parse(&parser, &uri, source.clone().into(), None);
    let mut samples = Vec::new();
    for index in 0..21 {
        let text = if index % 2 == 0 { &other } else { &source };
        let start = Instant::now();
        let mut tree = previous.tree().unwrap();
        tree.edit(&compute_input_edit(&previous.source, text).unwrap());
        let tree = parser.parse_tree_incremental(text, Some(&tree)).unwrap();
        let syntax = start.elapsed();

        let start = Instant::now();
        let sink = ErrorCollector::new();
        let (mut file, _) = parser.parse_chat_file_streaming_incremental(text, Some(&tree), &sink);
        let parse_errors = sink.into_vec();
        assert!(
            !parse_errors
                .iter()
                .any(|error| error.severity == Severity::Error),
            "phase probe requires a syntactically valid transcript"
        );
        let lowering = start.elapsed();

        let start = Instant::now();
        let sink = ErrorCollector::new();
        file.validate_with_alignment(
            &sink,
            TranscriptName::Named(FileStem::from_stem("basic-conversation")),
        );
        let errors = sink.into_vec();
        let validation = start.elapsed();

        let start = Instant::now();
        let diagnostics = to_diagnostics_batch_with_context(
            &errors.iter().collect::<Vec<_>>(),
            text,
            Some(&uri),
            Some(&file),
        );
        let conversion = start.elapsed();
        if index != 0 {
            samples.push([syntax, lowering, validation, conversion]);
        }
        let actual = DocumentAnalysis::parse(&parser, &uri, text.as_str().into(), Some(&previous));
        assert_eq!(diagnostics, actual.diagnostics());
        assert_eq!(&file, actual.file().as_ref());
        previous = actual;
    }
    for (column, name) in [
        "syntax_and_edit",
        "lowering_plus_same_source_parse",
        "validation",
        "diagnostic_conversion",
    ]
    .into_iter()
    .enumerate()
    {
        let mut values: Vec<_> = samples
            .iter()
            .map(|sample| sample[column].as_micros())
            .collect();
        values.sort_unstable();
        eprintln!(
            "analysis_phase phase={name} bytes={} samples={} p50_us={} p95_us={} max_us={}",
            source.len(),
            values.len(),
            values[9],
            values[18],
            values[19]
        );
    }
}
