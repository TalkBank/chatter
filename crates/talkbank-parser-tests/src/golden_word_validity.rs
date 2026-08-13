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

use crate::gate::{Gate, GateOutcome, listing};
use crate::golden::golden_words;
use talkbank_parser::TreeSitterParser;

/// The full golden-word corpus parses.
pub struct GoldenWordsGate;

impl Gate for GoldenWordsGate {
    fn name(&self) -> &'static str {
        "golden words parse (full corpus)"
    }

    fn check(&self) -> GateOutcome {
        let parser =
            TreeSitterParser::new().map_err(|err| format!("cannot build the parser: {err}"))?;

        let words = golden_words();
        if words.is_empty() {
            return Err("the golden word list is empty; a corpus gate over nothing \
                 reports a perfect score and means nothing"
                .to_owned());
        }

        let invalid: Vec<&str> = words
            .iter()
            .copied()
            .filter(|word| parser.parse_word(word).is_err())
            .collect();

        if invalid.is_empty() {
            return Ok(format!("{} golden word(s) parse", words.len()));
        }

        // Truncation is REPORTED, never silent: a list that stops at forty and
        // says so is evidence, one that just stops is a wrong count.
        let shown = invalid.len().min(40);
        let elided = invalid.len() - shown;
        Err(listing(
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
}
