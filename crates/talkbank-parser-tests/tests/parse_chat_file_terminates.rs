// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
)]

//! Regression gate: `parse_chat_file` must complete in bounded time on every
//! reference file, and on a long single-utterance synthetic input.
//!
//! History: the tree-sitter-grammar-utils reconstruction engine
//! (`generated_traversal.rs`) matched a rule of shape
//! `seq(item, repeat(choice(item, ...)))` (the CHAT `contents` rule) by an
//! UNMEMOIZED tail-fit that re-solved the whole suffix at every repeat
//! boundary, costing O(2^n) in the number of main-tier content items. Long
//! conversation utterances (por/rus/pol reference files) drove that past
//! practical limits: `parse_chat_file` did not return (a >30-minute effective
//! hang), even though the CLI `validate`/`to-json` paths, which use a
//! different entry point, did not. The engine was total (it terminated for
//! small inputs) but exponential, which is indistinguishable from a hang.
//!
//! This gate parses each input on a worker thread with a hard wall-clock
//! budget and fails FAST if the budget is exceeded, so a reintroduction of the
//! exponential blows up this test in seconds instead of hanging the suite.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use talkbank_parser::TreeSitterParser;

/// Wall-clock budget for a single parse. The fixed (memoized) engine parses
/// each of these in well under a second; the pre-fix exponential engine could
/// not finish a ~12-item utterance in 30 seconds. A generous budget keeps the
/// gate robust on a loaded CI machine while still catching any return of
/// super-linear blowup.
const PARSE_BUDGET: Duration = Duration::from_secs(20);

/// Parse `content` on a worker thread, returning `true` iff it completed
/// within [`PARSE_BUDGET`]. The worker is detached on timeout (the process
/// tears it down at test end); we only care whether the parse RETURNED in
/// time, which is exactly the property under test.
fn parses_within_budget(content: String) -> bool {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let parser = TreeSitterParser::new().expect("TreeSitterParser init");
        let _ = parser.parse_chat_file(&content);
        // Ignore send errors: on timeout the receiver is already gone.
        let _ = tx.send(());
    });
    rx.recv_timeout(PARSE_BUDGET).is_ok()
}

/// Every reference conversation file that historically exercised the blowup,
/// plus the whole reference set is covered by `parser_equivalence`; here we
/// pin the three known-pathological files by name.
#[test]
fn reference_files_parse_in_bounded_time() {
    let files = [
        "../../corpus/reference/languages/por-conversation.cha",
        "../../corpus/reference/languages/rus-conversation.cha",
        "../../corpus/reference/languages/pol-conversation.cha",
    ];
    for file in files {
        let content = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("read {file}: {e}"));
        assert!(
            parses_within_budget(content),
            "parse_chat_file did not return within {PARSE_BUDGET:?} for {file} \
             (exponential-blowup regression in the reconstruction engine)"
        );
    }
}

/// A single main-tier utterance of many plain words is the minimized trigger:
/// the `contents` repeat over content items is where the blowup lived, and it
/// needs no groups, retraces, or dependent tiers. Thirty words would take
/// astronomically long under the pre-fix O(2^n) engine.
#[test]
fn long_single_utterance_parses_in_bounded_time() {
    let words = (0..30)
        .map(|i| format!("w{i}"))
        .collect::<Vec<_>>()
        .join(" ");
    let content = format!("@UTF8\n@Begin\n@Languages:\tpor\n*CHI:\t{words} .\n@End\n");
    assert!(
        parses_within_budget(content),
        "parse_chat_file did not return within {PARSE_BUDGET:?} for a 30-word utterance"
    );
}
