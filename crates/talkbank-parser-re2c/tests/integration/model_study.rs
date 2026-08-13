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
//! `equivalence_tests.rs` and to the `parser_equivalence` gate, which walk every
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
    let re2c_parsed = talkbank_parser_re2c::parser::parse_main_tier(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(&re2c_parsed);
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
    let re2c_parsed = talkbank_parser_re2c::parser::parse_main_tier(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(&re2c_parsed);
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
    let re2c_parsed = talkbank_parser_re2c::parser::parse_main_tier(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(&re2c_parsed);
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
    let re2c_parsed = talkbank_parser_re2c::parser::parse_main_tier(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(&re2c_parsed);
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
    let re2c_parsed = talkbank_parser_re2c::parser::parse_main_tier(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(&re2c_parsed);
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
    let re2c_parsed = talkbank_parser_re2c::parser::parse_main_tier(input).unwrap();
    let re2c_mt = talkbank_parser_re2c::convert::main_tier_to_model(&re2c_parsed);
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
    talkbank_parser_re2c::convert::word_from_parsed(&parsed)
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
