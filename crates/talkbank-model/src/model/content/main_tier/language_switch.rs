//! Whole-utterance language-switch detection for main tiers.
//!
//! This module intentionally depends on `crate::validation::resolve_word_language`.
//! The same predicate is the detection seam behind validator E255 and the
//! `chatter debug fix-s` rewrite tooling, so it must share the validation
//! language-resolution rules rather than reimplementing them locally.

use super::MainTier;
use crate::model::LanguageCode;
use crate::model::content::word::Word;
use crate::model::content::{BracketedContent, BracketedItem, UtteranceContent};

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
fn collect_main_tier_words_for_language_check<'a>(
    content: &'a [UtteranceContent],
    out: &mut Vec<&'a Word>,
) {
    for item in content {
        collect_main_tier_word_item(item, out);
    }
}

fn collect_main_tier_word_item<'a>(item: &'a UtteranceContent, out: &mut Vec<&'a Word>) {
    match item {
        UtteranceContent::Word(word) => out.push(word),
        UtteranceContent::AnnotatedWord(annotated) => out.push(&annotated.inner),
        UtteranceContent::ReplacedWord(replaced) => {
            out.push(&replaced.word);
            for word in &replaced.replacement.words {
                out.push(word);
            }
        }
        UtteranceContent::Group(group) => {
            collect_main_tier_bracketed_items(&group.content, out);
        }
        UtteranceContent::AnnotatedGroup(annotated) => {
            collect_main_tier_bracketed_items(&annotated.inner.content, out);
        }
        UtteranceContent::PhoGroup(pho) => {
            collect_main_tier_bracketed_items(&pho.content, out);
        }
        UtteranceContent::SinGroup(sin) => {
            collect_main_tier_bracketed_items(&sin.content, out);
        }
        UtteranceContent::Quotation(quotation) => {
            collect_main_tier_bracketed_items(&quotation.content, out);
        }
        UtteranceContent::Retrace(retrace) => {
            collect_main_tier_bracketed_items(&retrace.content, out);
        }
        // Non-word items: skip (they don't carry word-level @s markers).
        UtteranceContent::Separator(_)
        | UtteranceContent::Event(_)
        | UtteranceContent::AnnotatedEvent(_)
        | UtteranceContent::Pause(_)
        | UtteranceContent::AnnotatedAction(_)
        | UtteranceContent::Freecode(_)
        | UtteranceContent::OverlapPoint(_)
        | UtteranceContent::InternalBullet(_)
        | UtteranceContent::LongFeatureBegin(_)
        | UtteranceContent::LongFeatureEnd(_)
        | UtteranceContent::UnderlineBegin(_)
        | UtteranceContent::UnderlineEnd(_)
        | UtteranceContent::NonvocalBegin(_)
        | UtteranceContent::NonvocalEnd(_)
        | UtteranceContent::NonvocalSimple(_)
        | UtteranceContent::OtherSpokenEvent(_) => {}
    }
}

fn collect_main_tier_bracketed_items<'a>(content: &'a BracketedContent, out: &mut Vec<&'a Word>) {
    for entry in &content.content {
        match entry {
            BracketedItem::Word(word) => out.push(word),
            BracketedItem::AnnotatedWord(annotated) => out.push(&annotated.inner),
            BracketedItem::ReplacedWord(replaced) => {
                out.push(&replaced.word);
                for word in &replaced.replacement.words {
                    out.push(word);
                }
            }
            BracketedItem::AnnotatedGroup(annotated) => {
                collect_main_tier_bracketed_items(&annotated.inner.content, out);
            }
            BracketedItem::PhoGroup(pho) => {
                collect_main_tier_bracketed_items(&pho.content, out);
            }
            BracketedItem::SinGroup(sin) => {
                collect_main_tier_bracketed_items(&sin.content, out);
            }
            BracketedItem::Quotation(quotation) => {
                collect_main_tier_bracketed_items(&quotation.content, out);
            }
            BracketedItem::Retrace(retrace) => {
                collect_main_tier_bracketed_items(&retrace.content, out);
            }
            // Non-word items: skip.
            BracketedItem::Event(_)
            | BracketedItem::AnnotatedEvent(_)
            | BracketedItem::Pause(_)
            | BracketedItem::Action(_)
            | BracketedItem::AnnotatedAction(_)
            | BracketedItem::OverlapPoint(_)
            | BracketedItem::Separator(_)
            | BracketedItem::InternalBullet(_)
            | BracketedItem::Freecode(_)
            | BracketedItem::LongFeatureBegin(_)
            | BracketedItem::LongFeatureEnd(_)
            | BracketedItem::UnderlineBegin(_)
            | BracketedItem::UnderlineEnd(_)
            | BracketedItem::NonvocalBegin(_)
            | BracketedItem::NonvocalEnd(_)
            | BracketedItem::NonvocalSimple(_)
            | BracketedItem::OtherSpokenEvent(_) => {}
        }
    }
}
