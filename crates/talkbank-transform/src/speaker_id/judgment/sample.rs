//! Deterministic per-speaker sampling for the holistic prompt. Main-tier
//! spoken text only; first N + last N utterances per speaker; char-capped.
//!
//! The anchor speaker is always emitted first in the output, regardless of
//! document order. Remaining speakers follow in first-seen order. Within a
//! speaker the utterances are in document order: the first `head` lines, then
//! the last `tail` lines (with possible overlap suppressed when the total
//! count is below `head + tail`).

use std::collections::HashMap;

use talkbank_model::SpeakerCode;
use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::model::{ChatFile, Line, Utterance};

use super::request::{SampledUtterance, SpeakerSamples};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// First N utterances included per speaker.
const DEFAULT_HEAD: usize = 10;

/// Last N utterances included per speaker.
const DEFAULT_TAIL: usize = 10;

/// Per-utterance character cap (Unicode scalars, not bytes).
const DEFAULT_CHAR_CAP: usize = 500;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Controls how many utterances are sampled and how long each may be.
///
/// The head + tail window gives the LLM both an early baseline and a late
/// view of the session, which together are more informative than a
/// contiguous middle slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SampleBudget {
    /// Number of utterances to take from the start of the speaker's turns.
    pub head: usize,
    /// Number of utterances to take from the end of the speaker's turns.
    pub tail: usize,
    /// Maximum Unicode-scalar count per sampled utterance. Longer utterances
    /// are truncated to this length (no ellipsis appended).
    pub char_cap: usize,
}

impl Default for SampleBudget {
    fn default() -> Self {
        Self {
            head: DEFAULT_HEAD,
            tail: DEFAULT_TAIL,
            char_cap: DEFAULT_CHAR_CAP,
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Build a [`Vec<SpeakerSamples>`] from `chat` using `budget`.
///
/// Only main-tier spoken words are included (via `walk_words`); dependent
/// tiers, timestamps, and `%mor` annotations are excluded. The anchor
/// speaker's samples appear at index 0; all other speakers follow in
/// first-seen document order. The result is fully deterministic for a given
/// `(chat, anchor, budget)` triple.
///
/// Each utterance is char-capped to `budget.char_cap` Unicode scalars.
/// When a speaker has at most `head + tail` utterances all of them are
/// included; otherwise only the first `head` and the last `tail` are kept.
pub fn sample_session(
    chat: &ChatFile,
    anchor: &SpeakerCode,
    budget: SampleBudget,
) -> Vec<SpeakerSamples> {
    // Single-pass grouping that tracks first-seen order intrinsically.
    // Each entry is (speaker_code, utterance_texts_in_document_order).
    // `index_of` maps a SpeakerCode to its slot in `groups` for O(1)
    // per-utterance accumulation. SpeakerCode is Hash+Eq but not Ord, so
    // an index-map pattern is used instead of BTreeMap.
    let mut groups: Vec<(SpeakerCode, Vec<String>)> = Vec::new();
    let mut index_of: HashMap<SpeakerCode, usize> = HashMap::new();

    for line in chat.lines.0.iter() {
        if let Line::Utterance(u) = line {
            let code = u.main.speaker.clone();
            let idx = match index_of.get(&code) {
                Some(&i) => i,
                None => {
                    let i = groups.len();
                    index_of.insert(code.clone(), i);
                    groups.push((code, Vec::new()));
                    i
                }
            };
            groups[idx].1.push(utterance_spoken_text(u));
        }
    }

    // Sort: anchor comes first; all other speakers keep their relative
    // first-seen document order. A stable sort-by-key over {0, 1} achieves
    // this because sort_by_key is stable in Rust.
    groups.sort_by_key(|(code, _)| if code == anchor { 0usize } else { 1usize });

    // For each speaker, apply the head+tail window and char cap. Ownership
    // moves cleanly out of `groups` via into_iter; no fallible removal needed.
    groups
        .into_iter()
        .map(|(code, all)| {
            let utterances = apply_head_tail(&all, budget)
                .into_iter()
                .map(|t| SampledUtterance(cap_chars(t, budget.char_cap)))
                .collect();
            SpeakerSamples { code, utterances }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Extract the spoken words from one utterance's main tier as a
/// space-joined string. Uses the same `walk_words` idiom as
/// `speaker_id::identify::speaker_bag` so that cleaning is consistent.
/// No dependent-tier content or timestamp data is included.
fn utterance_spoken_text(u: &Utterance) -> String {
    let mut tokens: Vec<String> = Vec::new();
    walk_words(&u.main.content.content, None, &mut |item| {
        if let WordItem::Word(w) = item {
            tokens.push(w.cleaned_text().to_string());
        }
    });
    tokens.join(" ")
}

/// Return at most `head + tail` items from `all`. When `all.len() <=
/// head + tail` every item is returned. Otherwise the first `head` and the
/// last `tail` are concatenated in document order. Borrows slices, so no
/// unnecessary allocation on the pass-through path.
fn apply_head_tail(all: &[String], budget: SampleBudget) -> Vec<&str> {
    let window = budget.head + budget.tail;
    if all.len() <= window {
        return all.iter().map(String::as_str).collect();
    }
    let mut out: Vec<&str> = Vec::with_capacity(window);
    out.extend(all[..budget.head].iter().map(String::as_str));
    out.extend(all[all.len() - budget.tail..].iter().map(String::as_str));
    out
}

/// Truncate `s` to at most `cap` Unicode scalar values. No ellipsis is
/// appended; the result is a clean prefix of the original text.
fn cap_chars(s: &str, cap: usize) -> String {
    s.chars().take(cap).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::parse_and_validate;
    use talkbank_model::ParseValidateOptions;

    // -----------------------------------------------------------------------
    // Fixture helpers
    // -----------------------------------------------------------------------

    /// Build a CHAT document string with `n` CHI utterances and `n` PAR0
    /// utterances, interleaved (CHI, PAR0, CHI, PAR0, ...). Each utterance
    /// uses a counter word so they are all distinct. One PAR0 utterance
    /// (index 0) is padded to be well over 500 chars to exercise the cap.
    fn build_doc(chi_count: usize, par0_count: usize) -> String {
        let mut doc = String::new();
        doc.push_str("@UTF8\n");
        doc.push_str("@Begin\n");
        doc.push_str("@Languages:\teng\n");
        doc.push_str("@Participants:\tCHI Child, PAR0 Adult\n");
        doc.push_str("@ID:\teng|corpus|CHI|||||Child|||\n");
        doc.push_str("@ID:\teng|corpus|PAR0|||||Adult|||\n");

        let max = chi_count.max(par0_count);
        let mut chi_emitted = 0usize;
        let mut par0_emitted = 0usize;
        for i in 0..max {
            if chi_emitted < chi_count {
                doc.push_str(&format!("*CHI:\tword{i} one two .\n"));
                chi_emitted += 1;
            }
            if par0_emitted < par0_count {
                if par0_emitted == 0 {
                    // First PAR0 utterance: pad to exceed 500 chars.
                    let padding = "extra ".repeat(100);
                    doc.push_str(&format!("*PAR0:\tlong{i} {padding}.\n"));
                } else {
                    doc.push_str(&format!("*PAR0:\tword{i} three four .\n"));
                }
                par0_emitted += 1;
            }
        }

        doc.push_str("@End\n");
        doc
    }

    /// Parse `doc` with default options (no validation), panicking on error.
    /// This matches the idiom in `apply.rs` and `identify.rs` test helpers.
    fn parse_doc(doc: &str) -> ChatFile {
        parse_and_validate(doc, ParseValidateOptions::default())
            .expect("fixture must parse without error")
    }

    /// Build and parse the canonical 25+25 fixture.
    fn donor_25_each() -> ChatFile {
        parse_doc(&build_doc(25, 25))
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    #[test]
    fn samples_head_and_tail_per_speaker() {
        let chat = donor_25_each();
        let out = sample_session(&chat, &SpeakerCode::new("CHI"), SampleBudget::default());
        let chi = out
            .iter()
            .find(|s| s.code == SpeakerCode::new("CHI"))
            .expect("CHI must be present");
        assert_eq!(
            chi.utterances.len(),
            20,
            "expected 10 head + 10 tail = 20 utterances for CHI"
        );
    }

    #[test]
    fn takes_all_when_fewer_than_budget() {
        // 5 utterances per speaker: fewer than head(10) + tail(10) = 20, so
        // all 5 should be returned.
        let chat = parse_doc(&build_doc(5, 5));
        let out = sample_session(&chat, &SpeakerCode::new("CHI"), SampleBudget::default());
        let chi = out
            .iter()
            .find(|s| s.code == SpeakerCode::new("CHI"))
            .expect("CHI must be present");
        assert_eq!(
            chi.utterances.len(),
            5,
            "expected all 5 utterances when below budget"
        );
    }

    #[test]
    fn caps_each_utterance_length() {
        // The first PAR0 utterance in the fixture is padded to >500 chars.
        // After sampling and capping it must be exactly 500 chars.
        let chat = donor_25_each();
        let out = sample_session(&chat, &SpeakerCode::new("CHI"), SampleBudget::default());
        let par0 = out
            .iter()
            .find(|s| s.code == SpeakerCode::new("PAR0"))
            .expect("PAR0 must be present");
        // The long utterance is the first PAR0 line; it lands in the head
        // window (index 0). Confirm it is capped.
        let first = &par0.utterances[0];
        assert!(
            first.0.chars().count() <= 500,
            "utterance must be capped to at most 500 chars, got {}",
            first.0.chars().count()
        );
    }

    #[test]
    fn is_deterministic() {
        let chat = donor_25_each();
        let a = sample_session(&chat, &SpeakerCode::new("CHI"), SampleBudget::default());
        let b = sample_session(&chat, &SpeakerCode::new("CHI"), SampleBudget::default());
        assert_eq!(
            a, b,
            "two calls with the same input must produce equal output"
        );
    }

    #[test]
    fn anchor_is_emitted_first() {
        // CHI is the anchor. In the interleaved fixture CHI appears at line 0
        // (document order), so this test primarily checks the stable-sort does
        // not accidentally move a non-anchor to front. Use a separate doc
        // where PAR0 appears first.
        let doc = concat!(
            "@UTF8\n",
            "@Begin\n",
            "@Languages:\teng\n",
            "@Participants:\tCHI Child, PAR0 Adult\n",
            "@ID:\teng|corpus|CHI|||||Child|||\n",
            "@ID:\teng|corpus|PAR0|||||Adult|||\n",
            "*PAR0:\tword one two .\n",
            "*PAR0:\tword three four .\n",
            "*CHI:\tword five six .\n",
            "@End\n",
        );
        let chat = parse_doc(doc);
        let out = sample_session(&chat, &SpeakerCode::new("CHI"), SampleBudget::default());
        assert_eq!(
            out[0].code,
            SpeakerCode::new("CHI"),
            "anchor must be the first entry even when PAR0 appears first in document"
        );
    }

    #[test]
    fn dependent_tier_content_excluded_from_samples() {
        // A CHI utterance with a %mor dependent tier. The sampled text must
        // contain only the spoken words from the main tier; %mor lemmas and
        // POS tags must not appear.
        //
        // The guard is structural: utterance_spoken_text calls
        // walk_words(&u.main.content.content, ...) which traverses only the
        // main-tier AST. Dependent tiers live in u.dependent_tiers (a separate
        // SmallVec field) and are never visited by walk_words. This test pins
        // that invariant so a future refactor that accidentally passes the
        // wrong field would be caught immediately.
        let doc = concat!(
            "@UTF8\n",
            "@Begin\n",
            "@Languages:\teng\n",
            "@Participants:\tCHI Child, PAR0 Adult\n",
            "@ID:\teng|corpus|CHI|||||Child|||\n",
            "@ID:\teng|corpus|PAR0|||||Adult|||\n",
            "*CHI:\tthe dog runs .\n",
            "%mor:\tdet|the n|dog v|run-3S .\n",
            "*PAR0:\tok .\n",
            "@End\n",
        );
        let chat = parse_doc(doc);
        let budget = SampleBudget {
            head: 5,
            tail: 5,
            char_cap: 500,
        };
        let out = sample_session(&chat, &SpeakerCode::new("CHI"), budget);
        let chi = out
            .iter()
            .find(|s| s.code == SpeakerCode::new("CHI"))
            .expect("CHI must be present");
        assert_eq!(chi.utterances.len(), 1, "exactly one CHI utterance");
        let text = &chi.utterances[0].0;
        // Main-tier words must be present.
        assert!(
            text.contains("dog"),
            "spoken word 'dog' must appear in sampled text, got: {text:?}"
        );
        // %mor content must be absent.
        assert!(
            !text.contains("det|the"),
            "%mor tag 'det|the' must NOT appear in sampled text, got: {text:?}"
        );
        assert!(
            !text.contains("v|run"),
            "%mor tag 'v|run' must NOT appear in sampled text, got: {text:?}"
        );
        assert!(
            !text.contains("n|dog"),
            "%mor tag 'n|dog' must NOT appear in sampled text, got: {text:?}"
        );
    }
}
