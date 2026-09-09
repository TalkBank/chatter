//! Every word in the golden corpus still parses as a standalone word.
//!
//! # The gap this closes
//!
//! `golden_words_validation` already gates this, and correctly: it returns an
//! error and says how to regenerate. But it runs over
//! `golden_words_minimal()`, one representative per feature signature, which
//! is 47 words. The full `golden_words.txt` is 769, and the only thing
//! checking it was `validate_golden_words --check-only`, which printed
//! "Found N invalid words" and then returned `Ok(())`, so it exited 0 on
//! precisely the state it exists to detect, and CI never ran it either way.
//!
//! The minimal list is deliberately the inner loop and stays as it is. This
//! covers the other 722.

use crate::gate::tree::RelPath;
use crate::gate::{Gate, Outcome, ProbeSuite, ReadTree, UnprovenRule, listing};
use crate::golden::entries_in;
use talkbank_parser::TreeSitterParser;

/// The golden word corpus, in one spelling.
const GOLDEN_WORDS: &str = "crates/talkbank-parser-tests/golden_words.txt";

/// The full golden-word corpus parses.
pub struct GoldenWordsGate;

/// Rules of this gate no plant can reach.
///
/// See [`crate::gate::UnprovenRule`]. The second entry is not a rule the
/// probes miss; it is a rule the gate does not have, which is worse and is
/// why it is written down here rather than nowhere.
const UNPROVEN: &[UnprovenRule] = &[
    UnprovenRule::new(
        "parser construction failing",
        "The grammar and the tree-sitter runtime are compiled together and \
         version-locked by Cargo, so the ABI check the C runtime performs \
         cannot trip at run time. Corrupting `grammar/src/parser.c` to force it \
         produces a link failure, which is a build break rather than a gate \
         failure and proves nothing about the gate.",
    ),
    UnprovenRule::new(
        "CORPUS SHRINKAGE, which is not a rule this gate has",
        "Deleting 700 of the words leaves the rest parsing and the gate reports \
         clean. The only size rule is `not zero`. This is the gate's largest \
         hole and it is invisible in its own output: `N golden word(s) parse` \
         reads identically whether N is 769 or 4. A floor belongs here, and \
         adding one is a code change rather than a probe.",
    ),
    UnprovenRule::new(
        "ROUNDTRIP fidelity, MODEL correctness and BACKEND parity",
        "The gate checks parse success only, and discards the parsed value. A \
         word that parses and serializes back to different bytes passes here; \
         that property belongs to `parser_suite/word_tests.rs`. Named so a \
         green run here is not read as more than it is.",
    ),
];

impl Gate for GoldenWordsGate {
    fn name(&self) -> &'static str {
        "golden words parse (full corpus)"
    }

    /// # Reading the list off the TREE, not out of the binary
    ///
    /// This used `include_str!`, so the corpus arrived as a `&'static str`
    /// baked in when the crate was compiled. That made the gate unprobeable
    /// (an overlay has nothing to intercept) and, worse, meant it judged the
    /// bytes that were on disk at BUILD time rather than the ones there now.
    /// The compiled copy is still what every other golden test uses; this one
    /// asks the question of the tree, which is what a gate is for.
    fn check(&self, tree: ReadTree) -> Outcome {
        let parser = match TreeSitterParser::new() {
            Ok(parser) => parser,
            Err(err) => return Outcome::failed(format!("cannot build the parser: {err}")),
        };

        let text = match tree.read_to_string(&RelPath::new(GOLDEN_WORDS)) {
            Ok(text) => text,
            Err(err) => {
                return Outcome::failed(format!("cannot read the golden word list: {err}"));
            }
        };
        // The loaders' own reader, not a copy of its filter. Not the same
        // LINES: this gate reads the list off the tree and `golden_words()`
        // reads the compiled-in copy, which is the difference every must-fail
        // probe below depends on. The same RULE, so a line the loaders would
        // hand a test as a word is a line this gate judges, and two spellings
        // of "which lines are entries" would be two answers to that.
        let words = entries_in(&text);
        if words.is_empty() {
            return Outcome::failed(
                "the golden word list is empty; a corpus gate over nothing \
                 reports a perfect score and means nothing",
            );
        }

        let invalid: Vec<&str> = words
            .iter()
            .copied()
            .filter(|word| parser.parse_word(word).is_err())
            .collect();

        if invalid.is_empty() {
            return tree.clean(format!("{} golden word(s) parse", words.len()));
        }

        // Truncation is REPORTED, never silent: a list that stops at forty and
        // says so is evidence, one that just stops is a wrong count.
        let shown = invalid.len().min(40);
        let elided = invalid.len() - shown;
        Outcome::failed(listing(
            &format!(
                "FAIL: {} of {} golden words no longer parse.\n\
                 The grammar or parser changed and golden_words.txt is out of sync.\n\
                 Confirm the change was intended, then regenerate:\n\
                 \x20 cargo run --release -p talkbank-parser-tests --bin audit_golden_words",
                invalid.len(),
                words.len()
            ),
            invalid
                .iter()
                .take(shown)
                .map(|word| (*word).to_owned())
                .chain((elided > 0).then(|| format!("... and {elided} more"))),
        ))
    }

    /// Each plant APPENDS, so the insta snapshots over the first three words
    /// are untouched and the probe cannot fail for a neighbouring reason.
    fn probes(&self) -> ProbeSuite {
        ProbeSuite::must_fail(
            "a line that cannot be one CHAT word",
            "golden words no longer parse",
            // The grammar declares `extras: []`, so whitespace is never
            // skipped and no `source_file` production spans a
            // space-separated triple. Every possible tree-sitter outcome
            // lands on an error path in `parse_word`.
            |edit| edit.append(GOLDEN_WORDS, "not a word\n"),
        )
        .refusing(
            "a corpus of no words at all",
            "the golden word list is empty",
            |edit| {
                edit.write(GOLDEN_WORDS, "# every word removed\n");
                Ok(())
            },
        )
        .refusing(
            "more failures than the listing shows, so the elision is reported",
            "... and 1 more",
            |edit| {
                let bad: String = (1..=41).map(|n| format!("bad word {n:02}\n")).collect();
                edit.append(GOLDEN_WORDS, &bad)
            },
        )
        .refusing(
            "the corpus file is not in the tree",
            "cannot read the golden word list",
            |edit| {
                edit.remove_file(GOLDEN_WORDS);
                Ok(())
            },
        )
    }

    fn unproven_rules(&self) -> &'static [UnprovenRule] {
        UNPROVEN
    }
}
