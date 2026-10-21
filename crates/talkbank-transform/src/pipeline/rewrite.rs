//! The proof a whole-file rewriter needs before it may overwrite a transcript.
//!
//! # The defect this exists to make unrepresentable
//!
//! `normalize` was parse, `to_chat_string()`, write. The return type was
//! [`String`], which cannot carry the one fact the caller needed, so on
//! 2026-08-27 it wrote a ZERO-BYTE file over a transcript at exit 0 under a
//! `✓ Normalized` line. Each case is pinned, with its exact input and exit
//! code, in `chatter/tests/integration/rewrite_never_loses_content_tests.rs`.
//!
//! # Which rewriters use THIS, and which use a different proof
//!
//! Only `normalize`. Its contract is "reshape, lose nothing", so "did a line
//! vanish" is exactly its question.
//!
//! The three EDITING rewriters (`chatter debug fix-s`, `retag-language` and
//! `join-retrace`) need a different one, and trying to share this was a real
//! mistake made and caught on 2026-08-27: they change content ON PURPOSE, so a
//! did-anything-vanish test applied to their output refuses every legitimate
//! edit they make. `fix-s` removes the very `@s:` markers this would look for.
//! Their proof is `parse_faithfully_or_report` in `chatter`'s debug module:
//! the model must reproduce the source BYTE FOR BYTE before the edit, which is
//! the only point at which faithfulness is a clean question for them.
//!
//! # What this type promises, and what it does NOT
//!
//! [`Rewrite`] requires ordered, disjoint witnesses for source-line content.
//! One output occurrence cannot stand in for repeated source lines. That is a claim
//! about LOSS, and it is deliberately not a claim about FAITHFULNESS: it
//! cannot prove a rewrite preserved meaning, and it is not a validity check.
//! An invalid transcript may still be rewritten, because validity and
//! representability are different questions and `normalize` is entitled to
//! canonicalise a file the validator would reject.
//!
//! # Why whitespace is stripped, and why containment rather than equality
//!
//! Canonicalising whitespace is precisely `normalize`'s job, so comparing raw
//! text would refuse every rewrite it exists to make; and a wrapped header
//! joined onto one line is why a source line must be CONTAINED rather than
//! equal. The six reference-corpus rewrites this must not refuse are listed,
//! as measured before/after pairs, on `the_normalizations_this_must_not_refuse`
//! below.

use talkbank_model::WriteChat;
use talkbank_model::model::ChatFile;

/// CHAT text that may be written over the source it was derived from.
///
/// The only constructor is [`Rewrite::of`]. A caller holding one of these has
/// already matched every nonblank source line in order without reusing output
/// bytes (ignoring whitespace); a caller that wants
/// the bytes without that assurance is asking for `ChatFile::to_chat_string`
/// and is not rewriting a file in place.
#[derive(Debug, Clone)]
pub struct Rewrite {
    text: String,
}

/// A source line with no remaining ordered counterpart in the serialization.
///
/// Carries the line so the operator can see WHAT would have been lost. A
/// refusal that only says "content would be dropped" sends someone hunting
/// through a file for a difference the tool already knows.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "rewriting would DROP or reorder content: line {line} of the source has no remaining ordered counterpart in the output: {text:?}"
)]
pub struct DroppedContent {
    /// The 1-based line number in the source.
    pub line: usize,
    /// The source line itself, as written.
    pub text: String,
}

/// Append every non-whitespace character of `line` to `out`, in order.
///
/// Not a general normalization: it exists so that "the same content, spaced
/// differently" compares equal, and nothing more.
///
/// Writes into a caller's buffer rather than returning a `String` because the
/// scan below runs per line of every transcript `normalize` touches, and a
/// `String` per line cost 31,430 allocations on a 15,715-line file. One
/// definition either way: a second spelling of the rule is how the two halves
/// of a comparison drift apart.
fn strip_whitespace_into(line: &str, out: &mut String) {
    out.extend(line.chars().filter(|c| !c.is_whitespace()));
}

/// Consume the earliest whole-line witness from the unmatched output suffix.
/// Returning only the suffix after the match prevents reuse of its bytes.
/// A source continuation may consume another part of the same output line.
fn consume_witness<'a>(mut remaining: &'a str, needle: &str) -> Option<&'a str> {
    while let Some((line, following)) = remaining.split_once('\n') {
        if let Some(start) = line.find(needle) {
            return Some(&remaining[start + needle.len()..]);
        }
        remaining = following;
    }
    None
}

impl Rewrite {
    /// Serialize `model`, requiring ordered, disjoint source-line witnesses.
    ///
    /// # Errors
    ///
    /// [`DroppedContent`] naming the first source line whose content does not
    /// appear after the preceding witness in the serialization.
    pub fn of(model: &ChatFile, source: &str) -> Result<Self, DroppedContent> {
        let text = model.to_chat_string();

        // EQUAL BYTES END IT, and this is not an optimisation of the answer,
        // it IS the answer: if the output is the source, every source line
        // trivially appears in it.
        //
        // The unchanged case needs no comparison buffers. Changed output uses a
        // monotonically consumed suffix, never restarting at an earlier line.
        if text == source {
            return Ok(Self { text });
        }

        // Stripped once into ONE buffer with line boundaries, rather than a
        // `String` per line: the 15,715-line file allocated 31,430 `String`s
        // to answer a question about 6 files in 107.
        let mut stripped = String::with_capacity(text.len());
        for line in text.lines() {
            strip_whitespace_into(line, &mut stripped);
            stripped.push('\n');
        }
        let mut remaining = stripped.as_str();

        // One scratch buffer, reused, so the needle costs no allocation after
        // the first line.
        let mut needle = String::new();
        for (index, source_line) in source.lines().enumerate() {
            needle.clear();
            strip_whitespace_into(source_line, &mut needle);
            // A blank or whitespace-only source line carries no content, so
            // its disappearance is not a loss. `normalize` removes them by
            // design; E747 says a blank line is not legal CHAT anyway.
            if needle.is_empty() {
                continue;
            }
            remaining = consume_witness(remaining, &needle).ok_or_else(|| DroppedContent {
                line: index + 1,
                text: source_line.to_string(),
            })?;
        }

        Ok(Self { text })
    }

    /// The CHAT text to write.
    ///
    /// Borrows rather than consuming, deliberately. An `into_text` beside this
    /// was the escape hatch: it turned the proof back into a bare `String`,
    /// which is the exact shape whose inability to carry this fact caused the
    /// bug. A caller that needs an owned copy can say `.text().to_owned()` and
    /// still be holding the proof when it writes.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

#[cfg(test)]
mod tests {
    // Test code: the panic family is relaxed by policy here, as at every other
    // test site in this workspace.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{consume_witness, strip_whitespace_into};

    /// The one owner's rule, as a value, for readable assertions.
    fn without_whitespace(line: &str) -> String {
        let mut out = String::new();
        strip_whitespace_into(line, &mut out);
        out
    }

    /// The comparison is whitespace-blind, which is the single line every
    /// acceptance below depends on, and one a future edit could quietly
    /// narrow.
    #[test]
    fn comparison_ignores_every_kind_of_whitespace() {
        assert_eq!(without_whitespace("spa , eng"), "spa,eng");
        assert_eq!(without_whitespace("\tMOT Mother,"), "MOTMother,");
        assert_eq!(
            without_whitespace("we need to  do something ."),
            "weneedtodosomething."
        );
        assert_eq!(without_whitespace("   "), "");
    }

    /// The rewrites `normalize` really performs on the reference corpus.
    ///
    /// A MEASUREMENT, not a policy: these are the actual before/after line
    /// pairs from `corpus/reference` on 2026-08-27, the six files `normalize`
    /// changes. Each must pass the predicate, because a content-loss guard
    /// that refuses these has replaced a data-loss bug with a useless tool.
    #[test]
    fn the_normalizations_this_must_not_refuse() {
        for (before, after) in [
            (
                "*CHI:\twe need to do something .",
                "*CHI:\twe need to  do something .",
            ),
            (
                "@ID:\teng|corpus|PAR|43;|male|Broca||Participant||73.9 |",
                "@ID:\teng|corpus|PAR|43;|male|Broca||Participant||73.9|",
            ),
            (
                "@ID:\tspa , eng|corpus|CHI|2;08.20||||Target_Child|||",
                "@ID:\tspa, eng|corpus|CHI|2;08.20||||Target_Child|||",
            ),
            ("*PAR:\tyeah sure &*INV:ah.", "*PAR:\tyeah sure &*INV:ah ."),
        ] {
            assert!(
                consume_witness(
                    &format!("{}\n", without_whitespace(after)),
                    &without_whitespace(before)
                )
                .is_some(),
                "the guard would refuse a legitimate normalization:\n  before: {before:?}\n  after:  {after:?}"
            );
        }
    }

    /// A wrapped header joined onto one line is why this asks for CONTAINMENT
    /// rather than equality.
    #[test]
    fn a_joined_continuation_is_not_a_loss() {
        let joined = format!(
            "{}\n",
            without_whitespace("@Participants:\tCHI Child, MOT Mother, FAT Father")
        );
        let mut remaining = joined.as_str();
        for part in [
            "@Participants:\tCHI Child,",
            "\tMOT Mother,",
            "\tFAT Father",
        ] {
            remaining = consume_witness(remaining, &without_whitespace(part))
                .unwrap_or_else(|| panic!("{part:?} should survive the join"));
        }
    }

    /// And the three real losses, none of which any amount of whitespace
    /// folding can hide.
    #[test]
    fn a_deleted_line_leaves_no_trace_to_match() {
        let two_utterances = without_whitespace("*CHI:\tone .*CHI:\ttwo .");
        assert!(
            !two_utterances.contains(&without_whitespace("*CHI:\tthree .")),
            "a deleted utterance must not appear to survive"
        );
        assert!(
            !without_whitespace("%gra:\t").contains(&without_whitespace("%gra:\t|2|SUBJ 2|0|ROOT")),
            "an emptied dependent tier must not appear to survive"
        );
        assert!(
            !without_whitespace("").contains(&without_whitespace("notes about this session")),
            "a file emptied to zero bytes must not appear to survive"
        );
    }
}
