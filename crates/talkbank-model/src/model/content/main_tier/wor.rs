//! `%wor` generation helpers for main tiers.
// Every match over the content enums in this file is exhaustive, so the lint
// costs nothing today and makes it stay that way: a new `UtteranceContent` or
// `BracketedItem` variant becomes a COMPILE ERROR here rather than a silent
// `_ =>` that answers wrong. Four such catch-alls have already shipped as
// defects; see `talkbank-parser-tests/src/content_catch_alls.rs`.
#![deny(clippy::wildcard_enum_match_arm)]

use super::MainTier;
use crate::Span;
use crate::model::ReplacedWord;
use crate::model::WriteChat;
use crate::model::content::word::Word;
use crate::model::content::{BracketedContent, BracketedItem, UtteranceContent};
use crate::model::dependent_tier::{WorItem, WorTier};

impl MainTier {
    /// Generate a flat %wor tier from embedded timing alignment stored on words.
    ///
    /// Walks the main tier tree, extracting each alignable word's cleaned_text
    /// and timing into a flat `Vec<WorItem>`. Each word's `inline_bullet`
    /// is preserved from the main tier word. Tag-marker separators (comma,
    /// tag, vocative) are emitted as `WorItem::Separator`.
    ///
    /// # Eye Candy: Word Text is Display-Only
    ///
    /// This function copies `cleaned_text` from main tier words to %wor tier
    /// words as "eye candy" (human-readable display text). **This text is never
    /// reparsed or used for processing**; it exists solely for:
    /// - Human readability when viewing CHAT files
    /// - Error message formatting
    /// - CHAT format serialization compliance
    ///
    /// **What matters**: The `inline_bullet` timing data, which is also copied
    /// and contains the actual timing information (start_ms, end_ms) used for
    /// all timing operations.
    ///
    /// We could equally well copy `raw_text`, use placeholders, or indices,
    /// the choice is purely for human readability, not processing correctness.
    ///
    /// See: `WorTier` documentation and `docs/wor-tier-text-audit.md`.
    pub fn generate_wor_tier(&self) -> WorTier {
        let mut items: Vec<WorItem> = Vec::new();
        collect_wor_items_content(&self.content.content, &mut items);

        WorTier {
            language_code: self.content.language_code.clone(),
            items,
            terminator: self.content.terminator.clone(),
            // %wor should not carry the utterance-level bullet, that belongs
            // on the main tier only.  Word-level timing lives in each
            // WorItem's inline_bullet field.
            bullet: None,
            span: Span::DUMMY,
        }
    }
}

/// Collect flat WorItems from main tier content for %wor generation.
///
/// `%wor` generation is almost leaf-local, but replaced-word handling and
/// separator emission differ from the generic walkers. We therefore recurse
/// explicitly instead of using `walk_words()`.
fn collect_wor_items_content(content: &[UtteranceContent], out: &mut Vec<WorItem>) {
    for item in content {
        collect_wor_item(item, out);
    }
}

fn collect_wor_item(item: &UtteranceContent, out: &mut Vec<WorItem>) {
    use crate::alignment::helpers::{counts_for_tier, is_tag_marker_separator};

    match item {
        UtteranceContent::Word(word) => {
            if counts_for_tier(word, crate::alignment::TierDomain::Wor) {
                out.push(WorItem::Word(Box::new(wor_word_from_main(word))));
            }
        }
        UtteranceContent::AnnotatedWord(annotated) => {
            if counts_for_tier(&annotated.inner, crate::alignment::TierDomain::Wor) {
                out.push(WorItem::Word(Box::new(wor_word_from_main(
                    &annotated.inner,
                ))));
            }
        }
        UtteranceContent::ReplacedWord(replaced) => {
            collect_wor_replaced_word(replaced, out);
        }
        UtteranceContent::Group(group) => {
            collect_wor_bracketed_content(&group.content, out);
        }
        UtteranceContent::AnnotatedGroup(annotated) => {
            collect_wor_bracketed_content(&annotated.inner.content, out);
        }
        UtteranceContent::PhoGroup(pho) => {
            collect_wor_bracketed_content(&pho.content, out);
        }
        UtteranceContent::SinGroup(sin) => {
            collect_wor_bracketed_content(&sin.content, out);
        }
        UtteranceContent::Quotation(quotation) => {
            collect_wor_bracketed_content(&quotation.content, out);
        }
        UtteranceContent::Retrace(retrace) => {
            collect_wor_bracketed_content(&retrace.content, out);
        }
        UtteranceContent::AnnotatedRetrace(annotated) => {
            // Same handling as the bare form: the wrapper carries only
            // the annotations written after the marker, and the retraced
            // content is unchanged.
            collect_wor_bracketed_content(&annotated.inner.content, out);
        }
        UtteranceContent::Separator(sep) => {
            if is_tag_marker_separator(sep) {
                out.push(WorItem::Separator {
                    text: sep.to_chat_string(),
                    span: sep.span(),
                });
            }
        }
        UtteranceContent::Event(_)
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

fn collect_wor_bracketed_content(content: &BracketedContent, out: &mut Vec<WorItem>) {
    for item in &content.content {
        collect_wor_bracketed_item(item, out);
    }
}

fn collect_wor_bracketed_item(item: &BracketedItem, out: &mut Vec<WorItem>) {
    use crate::alignment::helpers::{counts_for_tier, is_tag_marker_separator};

    match item {
        BracketedItem::Word(word) => {
            if counts_for_tier(word, crate::alignment::TierDomain::Wor) {
                out.push(WorItem::Word(Box::new(wor_word_from_main(word))));
            }
        }
        BracketedItem::AnnotatedWord(annotated) => {
            if counts_for_tier(&annotated.inner, crate::alignment::TierDomain::Wor) {
                out.push(WorItem::Word(Box::new(wor_word_from_main(
                    &annotated.inner,
                ))));
            }
        }
        BracketedItem::ReplacedWord(replaced) => {
            collect_wor_replaced_word(replaced, out);
        }
        BracketedItem::AnnotatedGroup(annotated) => {
            collect_wor_bracketed_content(&annotated.inner.content, out);
        }
        BracketedItem::PhoGroup(pho) => {
            collect_wor_bracketed_content(&pho.content, out);
        }
        BracketedItem::SinGroup(sin) => {
            collect_wor_bracketed_content(&sin.content, out);
        }
        BracketedItem::Quotation(quotation) => {
            collect_wor_bracketed_content(&quotation.content, out);
        }
        BracketedItem::Retrace(retrace) => {
            collect_wor_bracketed_content(&retrace.content, out);
        }
        BracketedItem::AnnotatedRetrace(annotated) => {
            // Same handling as the bare form: the wrapper carries only
            // the annotations written after the marker, and the retraced
            // content is unchanged.
            collect_wor_bracketed_content(&annotated.inner.content, out);
        }
        BracketedItem::Separator(sep) => {
            if is_tag_marker_separator(sep) {
                out.push(WorItem::Separator {
                    text: sep.to_chat_string(),
                    span: sep.span(),
                });
            }
        }
        BracketedItem::Event(_)
        | BracketedItem::AnnotatedEvent(_)
        | BracketedItem::Pause(_)
        | BracketedItem::Action(_)
        | BracketedItem::AnnotatedAction(_)
        | BracketedItem::OverlapPoint(_)
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

fn collect_wor_replaced_word(entry: &ReplacedWord, out: &mut Vec<WorItem>) {
    use crate::alignment::helpers::counts_for_tier;

    if counts_for_tier(&entry.word, crate::alignment::TierDomain::Wor) {
        out.push(WorItem::Word(Box::new(wor_word_from_main(&entry.word))));
    }
}

/// Build a `%wor` word from a main-tier word, preserving inline timing.
///
/// The `%wor` word gets `cleaned_text` as both raw and cleaned (since `%wor`
/// serializes cleaned_text), and inherits the inline_bullet directly.
///
/// # Eye Candy: Word Text is Display-Only
///
/// **IMPORTANT**: The word text we copy here is "eye candy"; it's never
/// reparsed or used for processing. We could equally well use:
/// - `cleaned_text` (current choice - human readable, matches TextGrid)
/// - `raw_text` (preserves CHAT markers like `:`, `@c`)
/// - Placeholders (`_`, `w0`, etc.)
///
/// **Current choice**: We use `cleaned_text` for human readability and
/// consistency with TextGrid export, but this is a **convention**, not a
/// requirement. The text is write-only from a processing perspective.
///
/// **What matters**: The `inline_bullet` field, which contains the actual
/// timing data (start_ms, end_ms) used for all timing operations.
///
/// See: `WorTier` documentation and `docs/wor-tier-text-audit.md` for details.
fn wor_word_from_main(word: &Word) -> Word {
    // Copy cleaned_text as "eye candy" (convention: human-readable display)
    let cleaned = word.cleaned_text();
    let mut w = Word::new_unchecked(cleaned, cleaned);

    // Copy timing data (this is the REAL data that actually matters)
    if let Some(ref bullet) = word.inline_bullet {
        w.inline_bullet = Some(bullet.clone());
    }
    w
}
