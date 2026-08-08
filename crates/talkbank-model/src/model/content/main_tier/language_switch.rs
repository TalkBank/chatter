//! Whole-utterance language-switch detection for main tiers.
//!
//! This module intentionally depends on `crate::validation::resolve_word_language`.
//! The same predicate is the detection seam behind validator E255 and the
//! `chatter debug fix-s` rewrite tooling, so it must share the validation
//! language-resolution rules rather than reimplementing them locally.

use super::MainTier;
use crate::model::LanguageCode;
use crate::model::content::UtteranceContent;
use crate::model::content::word::Word;

impl MainTier {
    /// Return the utterance-level language that would replace whole-tier
    /// per-word `@s` markers, if any.
    ///
    /// This is the detection seam behind E255 and fix-up tooling such as
    /// `chatter debug fix-s`: if every `%mor`-bearing lexical item resolves to
    /// the same non-default language override, the utterance should be written
    /// as `[- LANG] ...` instead of tagging each word individually.
    pub fn whole_utterance_language_switch_target(
        &self,
        default_language: Option<&LanguageCode>,
        declared_languages: &[LanguageCode],
    ) -> Option<LanguageCode> {
        let tier_language = self.content.language_code.as_ref().or(default_language);

        // Collect ALL word-bearing items (including fillers `&~`, `&-`,
        // `&+` and other nonword tokens), not just MOR-bearing ones. The
        // `[- LANG]` precode declares whole-utterance language scope, so
        // the predicate must verify every word the speaker actually
        // uttered, fillers and nonwords included, resolves to the same
        // language. Restricting to MOR-domain (the prior bug) skipped
        // tonal Cantonese fillers like `&~dang3` and silently classified
        // utterances as monolingual, producing E220 violations after the
        // rewrite, see the 2026-05-06 corpus-wide damage assessment.
        let mut words = Vec::new();
        collect_main_tier_words_for_language_check(&self.content.content, &mut words);
        // Deliberately fires on a ONE-word utterance too (`si@s .`).
        // Linguistically that is a judgment call (a lone insertion vs a
        // whole-utterance switch cannot be formalized), so the tiebreak is
        // operational (maintainer reassessment, 2026-07-30): the Batchalign
        // morphotag pipeline routes `[- LANG]`-precoded utterances wholesale
        // to that language's Stanza model, while `@s` words go through its
        // L2 splice machinery, which assumes an `@s` span is a proper
        // SUBSET of the utterance; a whole-utterance `@s` (one word is the
        // degenerate case) would exercise that machinery's unsupported
        // shape. E255 and `debug fix-s` share this predicate, so both keep
        // the one-word behavior together.
        if words.is_empty() {
            return None;
        }

        let mut target_lang: Option<LanguageCode> = None;
        for word in words {
            word.lang.as_ref()?;

            let outcome =
                crate::validation::resolve_word_language(word, tier_language, declared_languages);
            let resolved = match outcome.resolution {
                crate::validation::LanguageResolution::Single(code) => code,
                _ => return None,
            };

            if let Some(existing) = &target_lang {
                if existing != &resolved {
                    return None;
                }
            } else {
                target_lang = Some(resolved);
            }
        }

        target_lang
    }
}

/// Collect every word-bearing item from main-tier content for the
/// `[- LANG]` predicate. Includes fillers (`&~`, `&-`, `&+`),
/// nonwords, AND retrace content, every word the speaker uttered
/// counts toward the whole-utterance language scope, including
/// false-start material the speaker then corrected. The predicate's
/// per-word `lang.is_none() → return None` guard then refuses to
/// auto-promote to `[- LANG]` whenever ANY uttered word lacks an
/// explicit language attribution.
/// Collect every word on the tier, for the language-switch check.
///
/// `walk_words` with no tier domain is exactly this walk: it recurses into
/// every group kind, quotations and both retrace forms, and skips pauses,
/// events, markers and overlap points. Two hand-rolled matches used to say the
/// same thing, and each new content variant had to be added to both of them as
/// well as to the walker; the sibling module at `main_tier/mod.rs` was already
/// calling the walker.
///
/// A replaced word contributes BOTH sides: the word as produced and each word
/// of its replacement, because either can carry the `@s` marker the caller is
/// looking for. Separators carry no language and are dropped.
fn collect_main_tier_words_for_language_check<'a>(
    content: &'a [UtteranceContent],
    out: &mut Vec<&'a Word>,
) {
    crate::alignment::helpers::walk_words(content, None, &mut |item| match item {
        crate::alignment::helpers::WordItem::Word(word) => out.push(word),
        crate::alignment::helpers::WordItem::ReplacedWord(replaced) => {
            out.push(&replaced.word);
            out.extend(replaced.replacement.words.iter());
        }
        crate::alignment::helpers::WordItem::Separator(_) => {}
    });
}
