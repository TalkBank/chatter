//! English orthographic capitalization for CHAT main tiers.
//!
//! Two conventional rewrites, applied on the typed model so any generator can
//! share them (the MICASE/SBCSAE converters, the batchalign ASR post-processor):
//!
//! - **Pronoun "I"**: `i` / `i'm` / `i'll` / `i've` / `i'd` -> capitalized.
//! - **Utterance-initial**: the first "real" word of each utterance is
//!   capitalized (skipping CHAT markers `xxx`/`yyy`/`www`, `&`-prefixed
//!   fragments/nonwords, and tokens that start with a non-letter).
//!
//! Both are idempotent. Standard English orthography (capitalized "I" and
//! sentence starts) is the CHAT convention, matches the published corpora, and
//! is closer to the text distribution batchalign3 morphotag (Stanza) was
//! trained on, so applying it improves `%mor` accuracy over the all-lowercase
//! transcription style some source corpora use.
//!
//! The CHAT surface is serialized from a word's structured `content`, so the
//! rewrites mutate the first plain-text segment (the stem), not `raw_text`.

use talkbank_model::alignment::helpers::{WordItemMut, walk_words_mut};
use talkbank_model::model::content::word::{WordContent, WordText};
use talkbank_model::{ChatFile, Line};

/// Lowercased pronoun-"I" surfaces and their capitalized forms.
const I_CAP_REWRITES: &[(&str, &str)] = &[
    ("i", "I"),
    ("i'll", "I'll"),
    ("i'm", "I'm"),
    ("i've", "I've"),
    ("i'd", "I'd"),
];

/// Apply English capitalization (pronoun "I" + utterance-initial) to every
/// main tier of `chat`, in place.
pub fn capitalize_english(chat: &mut ChatFile) {
    for line in chat.lines.0.iter_mut() {
        let Line::Utterance(utt) = line else {
            continue;
        };
        let content = utt.main.content.content.0.as_mut_slice();
        let mut initial_done = false;
        walk_words_mut(content, None, &mut |item| {
            let WordItemMut::Word(word) = item else {
                return;
            };
            // The first plain-text segment (the stem) and its position.
            let Some((idx, current)) = word.content.iter().enumerate().find_map(|(i, c)| match c {
                WordContent::Text(t) => Some((i, t.to_string())),
                _ => None,
            }) else {
                return;
            };
            // Whole-word surface, for the pronoun-"I" and initial-eligibility
            // decisions.
            let cleaned = word.cleaned_text().to_string();
            let single_text = word.content.len() == 1;
            let mut next = current.clone();

            // Pronoun "I": whole-token match; only rewrite when the word is a
            // single text segment (no clitics/markers split out).
            if single_text {
                if let Some(dst) = capitalized_pronoun_i(&cleaned) {
                    next = dst.to_string();
                }
            }

            // Utterance-initial: capitalize the first real word once.
            if !initial_done && is_capitalizable_initial(&cleaned) {
                initial_done = true;
                next = capitalize_first(&next);
            }

            if next != current {
                if let Some(text) = WordText::new(next) {
                    word.content.replace_at(idx, WordContent::Text(text));
                }
            }
        });
    }
}

/// If `word` (case-insensitively) is a pronoun-"I" surface (`i`, `i'm`, `i'll`,
/// `i've`, `i'd`), return its capitalized form. Shared with generators that
/// capitalize their own word representation (the batchalign ASR post-processor).
pub fn capitalized_pronoun_i(word: &str) -> Option<&'static str> {
    let lower = word.to_lowercase();
    I_CAP_REWRITES
        .iter()
        .find(|(src, _)| *src == lower)
        .map(|(_, dst)| *dst)
}

/// Whether `text` is a "real" word eligible to be the capitalized
/// utterance-initial token (not a CHAT marker, fragment, or non-letter start).
pub fn is_capitalizable_initial(text: &str) -> bool {
    if matches!(text, "xxx" | "yyy" | "www") || text.starts_with('&') {
        return false;
    }
    text.chars().next().is_some_and(char::is_alphabetic)
}

/// Uppercase the first character of `text` if it is lowercase; otherwise return
/// it unchanged.
pub fn capitalize_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_lowercase() => {
            first.to_uppercase().collect::<String>() + chars.as_str()
        }
        _ => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::WriteChat;

    fn capitalized(main_tiers: &str) -> String {
        let parser = talkbank_parser::TreeSitterParser::new().expect("parser");
        let input = format!(
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tS1 Adult\n\
             @ID:\teng|x|S1|||||Adult|||\n{main_tiers}@End\n"
        );
        let (mut chat, _errors) = crate::parse::parse_lenient(&parser, &input);
        capitalize_english(&mut chat);
        chat.to_chat_string()
    }

    #[test]
    fn caps_pronoun_i_and_utterance_initial() {
        let out = capitalized("*S1:\ti think the dog ran .\n*S1:\tthe cat sat .\n");
        assert!(
            out.contains("*S1:\tI think the dog ran ."),
            "pronoun i + already-capital initial; got:\n{out}"
        );
        assert!(
            out.contains("*S1:\tThe cat sat ."),
            "utterance-initial capitalization; got:\n{out}"
        );
    }

    #[test]
    fn skips_markers_as_initial_and_caps_next_real_word() {
        let out = capitalized("*S1:\txxx dog barked .\n");
        assert!(
            out.contains("*S1:\txxx Dog barked ."),
            "xxx is not capitalized; the next real word is; got:\n{out}"
        );
    }

    #[test]
    fn caps_i_mid_utterance() {
        let out = capitalized("*S1:\tyeah i saw it .\n");
        assert!(
            out.contains("*S1:\tYeah I saw it ."),
            "mid-utterance i -> I and initial cap; got:\n{out}"
        );
    }

    // The three token-level helpers below are public API for generators
    // that capitalize their own word representation (the batchalign ASR
    // post-processor, per the deferred talkbank-tools-on-chatter
    // rewire); these tests are their contract.

    #[test]
    fn pronoun_i_helper_matches_all_surfaces_case_insensitively() {
        assert_eq!(capitalized_pronoun_i("i"), Some("I"));
        assert_eq!(capitalized_pronoun_i("i'm"), Some("I'm"));
        assert_eq!(capitalized_pronoun_i("i'll"), Some("I'll"));
        assert_eq!(capitalized_pronoun_i("i've"), Some("I've"));
        assert_eq!(capitalized_pronoun_i("i'd"), Some("I'd"));
        // Already-capitalized surfaces still map (idempotent callers).
        assert_eq!(capitalized_pronoun_i("I"), Some("I"));
        assert_eq!(capitalized_pronoun_i("I'M"), Some("I'm"));
    }

    #[test]
    fn pronoun_i_helper_rejects_non_pronoun_tokens() {
        for word in ["it", "in", "hi", "i's", "is", "", "island"] {
            assert_eq!(capitalized_pronoun_i(word), None, "word: {word:?}");
        }
    }

    #[test]
    fn capitalizable_initial_skips_markers_fragments_and_non_letters() {
        assert!(is_capitalizable_initial("dog"));
        assert!(is_capitalizable_initial("étude"));
        for text in ["xxx", "yyy", "www", "&-um", "&+fr", "0word", "'", ""] {
            assert!(!is_capitalizable_initial(text), "text: {text:?}");
        }
    }

    #[test]
    fn capitalize_first_uppercases_only_a_lowercase_start() {
        assert_eq!(capitalize_first("dog"), "Dog");
        assert_eq!(capitalize_first("Dog"), "Dog");
        assert_eq!(capitalize_first("étude"), "Étude");
        assert_eq!(capitalize_first("0word"), "0word");
        assert_eq!(capitalize_first(""), "");
    }
}
