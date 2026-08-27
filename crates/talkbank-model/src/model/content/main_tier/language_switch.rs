//! Whole-utterance language-switch detection for main tiers.
//!
//! This module intentionally resolves through `GoverningMarker`, the one
//! route to a word's language. The same predicate is the detection seam behind
//! validator E255 and the `chatter debug fix-s` rewrite tooling, so it must
//! share the validation language-resolution rules rather than reimplementing
//! them locally.

use super::MainTier;
use crate::model::CodeSwitchSpan;
use crate::model::LanguageCode;
use crate::model::content::UtteranceContent;
use crate::model::content::word::Word;
use crate::validation::word::language::{GoverningMarker, LanguageResolution};

/// A whole-utterance language switch, and the PROOF that no word in the
/// utterance is governed by a `<...> [@s]` span.
///
/// Its only constructor is [`MainTier::whole_utterance_language_switch_target`],
/// which refuses any span-governed word before returning one.
///
/// It exists because the guarantee used to travel as a comment. `fix_s` clears
/// each word's own `@s` after writing the `[- LANG]` precode, and for a word
/// inside a span the span would then govern it and silently change its
/// language; the code that clears markers therefore resolves with NO enclosing
/// scope, which is only sound because the predicate already refused those
/// utterances. That soundness argument sat in a five-line comment one crate
/// away from the check it depended on. Requiring this value to call the
/// clearing code moves the argument into the signature: a caller who has not
/// been through the predicate has nothing to pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnspannedSwitchTarget(LanguageCode);

impl UnspannedSwitchTarget {
    /// The language every word of the utterance resolved to.
    #[must_use]
    pub fn language(&self) -> &LanguageCode {
        &self.0
    }

    /// Does `word`'s OWN marker resolve to this target's language?
    ///
    /// THE OPERATION THE PROOF EXISTS FOR, which is why it lives here rather
    /// than at the caller. Answering it means resolving `word` with NO
    /// enclosing scope, and an unscoped resolution is the exact move that
    /// corrupted data before this type existed: for a word inside a
    /// `<...> [@s:eng]` span it reports the word's own language while the span
    /// is what actually governs it, so clearing the marker on that answer hands
    /// the word to the span and changes its language.
    ///
    /// It is sound here for one reason: this type cannot be obtained without
    /// passing [`MainTier::whole_utterance_language_switch_target`], which
    /// REFUSES any utterance containing a span-governed word. Holding a
    /// `&self` IS that refusal. Written as a free `GoverningMark::of(word,
    /// None)` at the call site, the same reasoning was a seven-line comment in
    /// another crate, and the value it depended on was passed alongside as a
    /// parameter that any `&LanguageCode` could have satisfied.
    ///
    /// KNOWN LIMIT, stated rather than implied: `self` proves that SOME
    /// utterance had no span-governed word, not that `word` came from that
    /// utterance. Pairing them in the type would mean this value borrowing the
    /// tier, which cannot coexist with the `&mut` walk that does the clearing.
    /// The remaining discipline is that a caller passes words from the tier it
    /// derived the target from.
    #[must_use]
    pub fn governs(
        &self,
        word: &Word,
        tier_language: Option<&LanguageCode>,
        declared_languages: &[LanguageCode],
    ) -> bool {
        let outcome = crate::validation::word::language::GoverningMark::of(word, None)
            .resolve(tier_language, declared_languages);
        outcome.resolution == LanguageResolution::Single(self.0.clone())
    }
}

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
    ) -> Option<UnspannedSwitchTarget> {
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
        for (word, enclosing) in words {
            word.lang.as_ref()?;

            // A word inside a `<...> [@s]` span is NOT a rewrite candidate, and
            // this guard is the whole reason the walk threads a scope here.
            //
            // The rewrite strips each word's own `@s` suffix after writing the
            // `[- LANG]` precode. For a word inside a span, the span then
            // governs it, so stripping SILENTLY CHANGES ITS LANGUAGE. Measured
            // before the fix: `<how@s:fra to@s:fra> [@s:eng] .` was rewritten
            // to `[- fra] <how to> [@s:eng] .`, whose words resolve to eng.
            // A transform advertised as a normalization relabelled every word
            // it touched, and reported success.
            //
            // Refusing is lossless; rewriting is not. Same-language spans are
            // refused too, deliberately: the predicate's job is to be sure, and
            // "the codes happen to agree today" is not a reason to edit.
            if enclosing.is_some() {
                return None;
            }

            let outcome = GoverningMarker::of(word, enclosing).resolve(
                word,
                tier_language,
                declared_languages,
            );
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

        target_lang.map(UnspannedSwitchTarget)
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
type ScopedWord<'a> = (&'a Word, Option<&'a CodeSwitchSpan>);

fn collect_main_tier_words_for_language_check<'a>(
    content: &'a [UtteranceContent],
    out: &mut Vec<ScopedWord<'a>>,
) {
    // The SCOPED walk, not `walk_words`: this predicate decides whether to
    // delete per-word `@s` markers, and that decision is wrong for any word an
    // enclosing span would inherit. Discarding the scope here is what let the
    // rewrite corrupt language.
    crate::alignment::helpers::walk_words_scoped(content, None, &mut |item, scope| match item {
        crate::alignment::helpers::WordItem::Word(word) => out.push((word, scope.span())),
        crate::alignment::helpers::WordItem::ReplacedWord(replaced) => {
            out.push((&replaced.word, scope.span()));
            out.extend(
                replaced
                    .replacement
                    .words
                    .iter()
                    .map(|word| (word, scope.span())),
            );
        }
        crate::alignment::helpers::WordItem::Separator(_) => {}
    });
}
