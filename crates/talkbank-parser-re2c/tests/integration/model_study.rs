// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Single-input equivalence probes: one hand-picked construct each, checked
//! for `semantic_eq` between the tree-sitter parser and the re2c oracle.
//!
//! These are the NARROW companions to the corpus-driven checks in
//! `equivalence_tests.rs`, which iterate fixture sets and the reference corpus.
//! When a divergence appears there, it names a file; here it names a construct.
//!
//! Corpus-wide equivalence is deliberately NOT here. It belongs to
//! `equivalence_tests.rs`, which walks every
//! reference file and isolate failures per file; a copy here would re-walk the
//! same tree with coarser reporting and, when the corpus is absent, pass while
//! testing nothing.

use talkbank_model::SemanticEq;
use talkbank_parser::TreeSitterParser;

fn ts() -> TreeSitterParser {
    TreeSitterParser::new().expect("grammar loads")
}

// ═══════════════════════════════════════════════════════════════
// Word equivalence tests (not ignored; these run in CI)
// ═══════════════════════════════════════════════════════════════

#[test]
fn word_equivalence_simple() {
    let ts_word = ts().parse_word("hello").unwrap();
    let re2c_word = re2c_word("hello");
    assert!(
        ts_word.semantic_eq(&re2c_word),
        "simple word mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_word).unwrap(),
        serde_json::to_string(&re2c_word).unwrap(),
    );
}

#[test]
fn word_equivalence_compound() {
    let ts_word = ts().parse_word("ice+cream").unwrap();
    let re2c_word = re2c_word("ice+cream");
    assert!(
        ts_word.semantic_eq(&re2c_word),
        "compound word mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_word).unwrap(),
        serde_json::to_string(&re2c_word).unwrap(),
    );
}

#[test]
fn word_equivalence_lengthening() {
    let ts_word = ts().parse_word("no::").unwrap();
    let re2c_word = re2c_word("no::");
    assert!(
        ts_word.semantic_eq(&re2c_word),
        "lengthening mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_word).unwrap(),
        serde_json::to_string(&re2c_word).unwrap(),
    );
}

#[test]
fn word_equivalence_form_marker() {
    let ts_word = ts().parse_word("mama@f").unwrap();
    let re2c_word = re2c_word("mama@f");
    assert!(
        ts_word.semantic_eq(&re2c_word),
        "form marker mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_word).unwrap(),
        serde_json::to_string(&re2c_word).unwrap(),
    );
}

#[test]
fn word_equivalence_shortening() {
    let ts_word = ts().parse_word("(be)cause").unwrap();
    let re2c_word = re2c_word("(be)cause");
    assert!(
        ts_word.semantic_eq(&re2c_word),
        "shortening mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_word).unwrap(),
        serde_json::to_string(&re2c_word).unwrap(),
    );
}

#[test]
fn word_equivalence_filler() {
    let ts_word = ts().parse_word("&-um").unwrap();
    let re2c_word = re2c_word("&-um");
    assert!(
        ts_word.semantic_eq(&re2c_word),
        "filler mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_word).unwrap(),
        serde_json::to_string(&re2c_word).unwrap(),
    );
}

#[test]
fn word_equivalence_lang_suffix() {
    let ts_word = ts().parse_word("hao3@s:zho").unwrap();
    let re2c_word = re2c_word("hao3@s:zho");
    assert!(
        ts_word.semantic_eq(&re2c_word),
        "lang suffix mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_word).unwrap(),
        serde_json::to_string(&re2c_word).unwrap(),
    );
}

// ═══════════════════════════════════════════════════════════════
// Main tier structure verification
// ═══════════════════════════════════════════════════════════════

#[test]
fn main_tier_retrace_structure() {
    // Verify our parser produces Retrace for "the the [/] dog ."
    let mt = talkbank_parser_re2c::parser::parse_main_tier("*CHI:\tthe the [/] dog .\n").unwrap();
    let has_retrace = mt
        .tier_body
        .contents
        .iter()
        .any(|c| matches!(c, talkbank_parser_re2c::ast::ContentItem::Retrace(_)));
    assert!(
        has_retrace,
        "expected Retrace in: {:?}",
        mt.tier_body.contents
    );
}

#[test]
fn main_tier_equivalence_simple() {
    let input = "*CHI:\thello world .\n";
    let ts_mt = ts().parse_main_tier(input).unwrap();
    let (re2c_parsed, re2c_src) =
        talkbank_parser_re2c::parser::parse_main_tier_with_source(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(
        &re2c_parsed,
        talkbank_parser_re2c::source_text::SourceText::new(re2c_src),
    );
    assert!(
        ts_mt.semantic_eq(&re2c_mt),
        "simple main tier mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_mt).unwrap(),
        serde_json::to_string(&re2c_mt).unwrap(),
    );
}

#[test]
fn main_tier_equivalence_retrace() {
    let input = "*CHI:\tthe the [/] dog .\n";
    let ts_mt = ts().parse_main_tier(input).unwrap();
    let (re2c_parsed, re2c_src) =
        talkbank_parser_re2c::parser::parse_main_tier_with_source(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(
        &re2c_parsed,
        talkbank_parser_re2c::source_text::SourceText::new(re2c_src),
    );
    assert!(
        ts_mt.semantic_eq(&re2c_mt),
        "retrace main tier mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_mt).unwrap(),
        serde_json::to_string(&re2c_mt).unwrap(),
    );
}

#[test]
fn main_tier_equivalence_compound() {
    let input = "*CHI:\tI want ice+cream .\n";
    let ts_mt = ts().parse_main_tier(input).unwrap();
    let (re2c_parsed, re2c_src) =
        talkbank_parser_re2c::parser::parse_main_tier_with_source(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(
        &re2c_parsed,
        talkbank_parser_re2c::source_text::SourceText::new(re2c_src),
    );
    assert!(
        ts_mt.semantic_eq(&re2c_mt),
        "compound main tier mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_mt).unwrap(),
        serde_json::to_string(&re2c_mt).unwrap(),
    );
}

#[test]
fn main_tier_equivalence_event() {
    let input = "*CHI:\t&=laughs .\n";
    let ts_mt = ts().parse_main_tier(input).unwrap();
    let (re2c_parsed, re2c_src) =
        talkbank_parser_re2c::parser::parse_main_tier_with_source(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(
        &re2c_parsed,
        talkbank_parser_re2c::source_text::SourceText::new(re2c_src),
    );
    assert!(
        ts_mt.semantic_eq(&re2c_mt),
        "event main tier mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_mt).unwrap(),
        serde_json::to_string(&re2c_mt).unwrap(),
    );
}

#[test]
fn main_tier_equivalence_pause() {
    let input = "*CHI:\tI (.) want cookies .\n";
    let ts_mt = ts().parse_main_tier(input).unwrap();
    let (re2c_parsed, re2c_src) =
        talkbank_parser_re2c::parser::parse_main_tier_with_source(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(
        &re2c_parsed,
        talkbank_parser_re2c::source_text::SourceText::new(re2c_src),
    );
    assert!(
        ts_mt.semantic_eq(&re2c_mt),
        "pause main tier mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_mt).unwrap(),
        serde_json::to_string(&re2c_mt).unwrap(),
    );
}

#[test]
fn main_tier_equivalence_trailing_off() {
    let input = "*CHI:\tI was going to the +...\n";
    let ts_mt = ts().parse_main_tier(input).unwrap();
    let (re2c_parsed, re2c_src) =
        talkbank_parser_re2c::parser::parse_main_tier_with_source(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(
        &re2c_parsed,
        talkbank_parser_re2c::source_text::SourceText::new(re2c_src),
    );
    assert!(
        ts_mt.semantic_eq(&re2c_mt),
        "trailing off mismatch:\n  ts:   {}\n  re2c: {}",
        serde_json::to_string(&ts_mt).unwrap(),
        serde_json::to_string(&re2c_mt).unwrap(),
    );
}

#[test]
fn mor_tier_equivalence() {
    let input = "pro|I v|want n|cookie-PL .\n";
    let errors = talkbank_model::errors::ErrorCollector::new();
    let ts_result = ts().parse_mor_tier_fragment(input, 0, &errors);
    let re2c_parsed = talkbank_parser_re2c::parser::parse_mor_tier(input);
    let re2c_tier = match talkbank_model::model::MorTier::try_from(&re2c_parsed) {
        Ok(t) => t,
        Err(e) => panic!("re2c MorTier conversion failed: {:?}", e),
    };
    if let talkbank_model::ParseOutcome::Parsed(ts_tier) = ts_result {
        assert!(
            ts_tier.semantic_eq(&re2c_tier),
            "mor tier mismatch:\n  ts:   {}\n  re2c: {}",
            serde_json::to_string(&ts_tier).unwrap(),
            serde_json::to_string(&re2c_tier).unwrap(),
        );
    } else {
        panic!("ts rejected mor tier");
    }
}

#[test]
fn gra_tier_equivalence() {
    let input = "1|2|SUBJ 2|0|ROOT 3|2|OBJ\n";
    let errors = talkbank_model::errors::ErrorCollector::new();
    let ts_result = ts().parse_gra_tier_fragment(input, 0, &errors);
    let re2c_parsed = talkbank_parser_re2c::parser::parse_gra_tier(input);
    let re2c_tier = talkbank_model::model::GraTier::from(&re2c_parsed);
    if let talkbank_model::ParseOutcome::Parsed(ts_tier) = ts_result {
        assert!(
            ts_tier.semantic_eq(&re2c_tier),
            "gra tier mismatch:\n  ts:   {}\n  re2c: {}",
            serde_json::to_string(&ts_tier).unwrap(),
            serde_json::to_string(&re2c_tier).unwrap(),
        );
    } else {
        panic!("ts rejected gra tier");
    }
}

// ═══════════════════════════════════════════════════════════════
// Full file equivalence
/// Parse a word using our re2c parser and convert to model Word.
fn re2c_word(input: &str) -> talkbank_model::model::Word {
    let parsed = talkbank_parser_re2c::parser::parse_word(input).expect("re2c parse_word");
    // `parse_word` leaks its own copy and does not hand it back, so no source
    // can place these slices; the span stays dummy and these tests compare
    // content, which `semantic_eq` does anyway.
    talkbank_parser_re2c::convert::word_from_parsed(
        &parsed,
        talkbank_parser_re2c::source_text::SourceText::new(input),
    )
}

/// Diff a single file's parse between TreeSitter and Re2c. Standard
/// drill-in tool when `categorize_divergences` flags a file: writes both
/// parsers' JSON to `/tmp/re2c_compare_{ts,re2c}.json` and prints
/// `semantic_eq:`. Used during the P12 parity-push workflow.
///
///     TB_DIFF_FILE=/path/to/file.cha cargo test --offline \
///         -p talkbank-parser-re2c --tests --release \
///         study_diff_one_file -- --ignored --nocapture
#[test]
#[ignore]
fn study_diff_one_file() {
    use talkbank_model::ChatParser;
    use talkbank_model::ErrorCollector;
    use talkbank_model::ParseOutcome;
    use talkbank_parser_re2c::Re2cParser;

    let path = std::env::var("TB_DIFF_FILE").expect("set TB_DIFF_FILE=path");
    let content = std::fs::read_to_string(&path).unwrap();
    let tsp = ts();
    let re2c = Re2cParser::new();
    let errs = ErrorCollector::new();
    // Streaming variant returns the recovered ChatFile even when TS emits
    // diagnostics, matching what categorize_divergences and the
    // equivalence_reference_corpus oracle use.
    let ts_errors = ErrorCollector::new();
    let ts_file = tsp.parse_chat_file_streaming(&content, &ts_errors);
    let re2c_out = re2c.parse_chat_file(&content, 0, &errs);
    let re2c_file = match re2c_out {
        ParseOutcome::Parsed(f) => f,
        _ => panic!("re2c rejected"),
    };
    let eq = ts_file.semantic_eq(&re2c_file);
    eprintln!("semantic_eq: {eq}");
    let ts_json = serde_json::to_string_pretty(&ts_file).unwrap();
    let re2c_json = serde_json::to_string_pretty(&re2c_file).unwrap();
    std::fs::write("/tmp/re2c_compare_ts.json", &ts_json).unwrap();
    std::fs::write("/tmp/re2c_compare_re2c.json", &re2c_json).unwrap();
    eprintln!(
        "wrote /tmp/re2c_compare_ts.json + /tmp/re2c_compare_re2c.json (run: diff /tmp/re2c_compare_ts.json /tmp/re2c_compare_re2c.json)"
    );
}

/// Both backends place every main-tier item at the same source span.
///
/// Pins the half of the picture that is correct, so the comparison below has a
/// fixed reference. Written while establishing why `E258` (consecutive commas)
/// is silent under re2c: that rule reads `comma_span()`, which FILTERS OUT
/// `Span::DUMMY`, so a dummy span switches the check off with no diagnostic.
/// Before the fix, re2c reported `Span::DUMMY` for all four items on this
/// input, through the whole-file path as well as the fragment API, so every
/// span-keyed rule was unreachable under `--parser=re2c`.
///
/// Words were fixed in the same arc: `WordWithAnnotations::raw_text` IS the
/// word's slice of the source, so `span_of` places it directly.
#[test]
fn whole_file_spans_are_reported() {
    let src = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
        @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello ,, world .\n@End\n";
    let ts_errors = talkbank_model::errors::ErrorCollector::new();
    use talkbank_model::ChatParser as _;
    let ts_file = ts().parse_chat_file_streaming(src, &ts_errors);
    let re2c = talkbank_parser_re2c::Re2cParser::new();
    let re_errors = talkbank_model::errors::ErrorCollector::new();
    let re_out = re2c.parse_chat_file(src, 0, &re_errors);

    let talkbank_model::ParseOutcome::Parsed(re2c_file) = re_out else {
        panic!("re2c must parse this input");
    };

    let item_spans = |f: &talkbank_model::ChatFile| -> Vec<talkbank_model::Span> {
        f.utterances()
            .flat_map(|u| u.main.content.content.iter())
            .filter_map(|item| match item {
                talkbank_model::model::UtteranceContent::Separator(sep) => Some(sep.span()),
                talkbank_model::model::UtteranceContent::Word(w) => Some(w.span),
                _ => None,
            })
            .collect()
    };
    let ts_spans = item_spans(&ts_file);
    assert_eq!(
        ts_spans.len(),
        4,
        "two words and two commas, got {ts_spans:?}"
    );
    assert!(
        ts_spans.iter().all(|s| *s != talkbank_model::Span::DUMMY),
        "tree-sitter must place every item, got {ts_spans:?}"
    );
    assert_eq!(
        item_spans(&re2c_file),
        ts_spans,
        "both backends must place every main-tier item identically"
    );
}

/// A MULTI-TOKEN word must be placed too, not just a single-token one.
///
/// `subtoken_word` rebuilds `raw_text` by `Box::leak`ing a fresh concatenation
/// of its tokens' display forms, so that string is a DIFFERENT ALLOCATION from
/// the source and `SourceText::span_of` correctly refuses it. The word then
/// keeps `Span::DUMMY`. `whole_file_spans_are_reported` cannot see this: every
/// word in its input is a single token.
#[test]
fn multi_token_words_are_placed_too() {
    let input =
        std::env::var("PROBE_INPUT").unwrap_or_else(|_| "*CHI:\tthe he(l)lo world .\n".to_string());
    let input = input.as_str();
    let ts_mt = ts().parse_main_tier(input).unwrap();
    let (re2c_parsed, re2c_src) =
        talkbank_parser_re2c::parser::parse_main_tier_with_source(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(
        &re2c_parsed,
        talkbank_parser_re2c::source_text::SourceText::new(re2c_src),
    );
    let spans = |mt: &talkbank_model::model::MainTier| -> Vec<talkbank_model::Span> {
        mt.content
            .content
            .iter()
            .filter_map(|i| match i {
                talkbank_model::model::UtteranceContent::Word(w) => Some(w.span),
                _ => None,
            })
            .collect()
    };
    println!("ts:   {:?}", spans(&ts_mt));
    println!("re2c: {:?}", spans(&re2c_mt));
    assert_eq!(
        spans(&re2c_mt),
        spans(&ts_mt),
        "multi-token word spans differ"
    );
}

/// No diagnostic is reported TWICE by the re2c backend.
///
/// # The gate that cannot see this
///
/// `backends_diverge_only_where_recorded` stores each side's codes as
/// `Codes(BTreeSet<ErrorCode>)`, so multiplicity is structurally
/// unrepresentable there: a rule reported once and a rule reported twice are
/// the same value. Nothing else in the tree compares counts either.
///
/// # Why that mattered
///
/// `parser/file.rs` carries hand-written token scans that MIRROR model rules,
/// each justified by a comment saying the model rule "cannot fire on this
/// parser's output because its X carry dummy spans". When the 2026-08-27 span
/// work made words and separators carry real spans, three of those model rules
/// became reachable and their mirrors were not removed, so re2c reported
/// E749, E764 and E765 twice each. Measured, not supposed.
///
/// A mirror is therefore only correct while its model rule is genuinely
/// unreachable, and this test is what notices when that stops being true.
#[test]
fn re2c_reports_no_diagnostic_twice() {
    use std::collections::BTreeMap;
    use talkbank_model::ChatParser as _;

    // One utterance per rule that has, or had, a mirror in `parser/file.rs`.
    let cases = [
        ("comma glued to next word", "hey ,you ."),
        ("prefixed form glued to preceding word", "dog&-um ."),
        ("separator glued to following content", "dog :and ."),
        ("pause glued to word", "hello(.) world ."),
        ("code glued to following content", "hello [/]x ."),
        ("consecutive commas", "hello ,, world ."),
    ];

    let re2c = talkbank_parser_re2c::Re2cParser::new();
    let mut offenders = Vec::new();
    for (label, utterance) in cases {
        let src = format!(
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
             @ID:\teng|corpus|CHI|3;00.||||Child|||\n*CHI:\t{utterance}\n@End\n"
        );
        // PARSE AND VALIDATE, which is the boundary `chatter validate` uses.
        // Parsing alone does not run the model rules, and the whole point here
        // is that a parser mirror and a model rule can BOTH fire; a test below
        // that boundary sees one of the two and passes.
        let errors = talkbank_model::errors::ErrorCollector::new();
        if let talkbank_model::ParseOutcome::Parsed(file) = re2c.parse_chat_file(&src, 0, &errors) {
            // `Anonymous`: these are test strings, so filename rules do not run.
            file.validate(&errors, talkbank_model::model::TranscriptName::Anonymous);
        }
        let mut counts: BTreeMap<talkbank_model::ErrorCode, usize> = BTreeMap::new();
        for e in errors.into_vec() {
            *counts.entry(e.code).or_insert(0) += 1;
        }
        for (code, n) in counts {
            if n > 1 {
                offenders.push(format!(
                    "{label} ({utterance:?}): {code:?} reported {n} times"
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "re2c reported these diagnostics more than once:\n  {}",
        offenders.join("\n  ")
    );
}
