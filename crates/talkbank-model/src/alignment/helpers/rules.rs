//! Rule predicates used by cross-tier alignment logic.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_Scope>
//! - <https://talkbank.org/0info/manuals/CHAT.html#MorExclude_Scope>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Timing_Tier>

use crate::model::{ContentAnnotation, ReplacedWord, Separator, Word, WordCategory};

use super::domain::TierDomain;

/// Returns `true` if any annotation in the slice excludes content from alignment.
///
/// This helper is domain-agnostic; callers decide whether exclusion applies in
/// the current alignment domain.
pub fn annotations_have_alignment_ignore(annotations: &[ContentAnnotation]) -> bool {
    annotations.iter().any(is_alignment_ignore_annotation)
}

/// Returns whether one annotation indicates alignment exclusion semantics.
///
/// The `[e]` exclude marker represents suppressed material, so alignment
/// policies may choose to drop the annotated content.
///
/// Retrace markers are no longer `ContentAnnotation` variants; they are
/// handled as first-class `Retrace` content variants at the `UtteranceContent`
/// and `BracketedItem` level.
fn is_alignment_ignore_annotation(annotation: &ContentAnnotation) -> bool {
    matches!(annotation, ContentAnnotation::Exclude)
}

/// Return whether a word participates in alignment for the target domain.
///
/// The canonical domain gate. It depends on the WORD and the target tier, and
/// on nothing about the containers the word sits inside. `%wor` membership once
/// depended on retrace ancestry and deliberately no longer does: `%wor` models
/// spoken main-tier word slots directly, inside a retrace or not.
///
/// That invariant is what lets a single traversal call one leaf predicate, so
/// do not reintroduce a container-context argument here.
pub fn counts_for_tier(word: &Word, domain: TierDomain) -> bool {
    // Empty words (from parser artifacts) should never align
    if word.cleaned_text().is_empty() {
        return false;
    }

    if word
        .category
        .as_ref()
        .is_some_and(WordCategory::is_omission)
    {
        return false;
    }

    match domain {
        // %mor = linguistic/morphological content (excludes ALL fragments, untranscribed)
        TierDomain::Mor => is_linguistic_content(word),

        // %wor = word-level timing over spoken main-tier word slots.
        //
        // %wor is a timing-annotation tier: it records word-level start/end
        // bullets for tokens the FA engine can meaningfully anchor to audio.
        //
        // Excluded:
        // - Untranscribed (`xxx`, `yyy`, `www`): no known phoneme sequence;
        //   CTC alignment cannot produce timings for unknown material.
        // - Phonological fragments (`&+`, WordCategory::PhonologicalFragment):
        //   incomplete phoneme sequences.
        // - Nonwords (`&~`, WordCategory::Nonword): interactional/gestural
        //   sounds without stable phonemic content.
        //
        // Included:
        // - Regular words and fillers (`&-`, WordCategory::Filler): stable,
        //   alignable phoneme sequences.
        //
        // This matches the BA2-era reference: `&+` and `&~` classified as
        // `TokenType.ANNOT` (excluded), while `&-` was `TokenType.FP`
        // (included). See the BA2 `formats/chat/lexer.py` `__handle` method.
        TierDomain::Wor => {
            !is_wor_timing_token(word)
                && word.untranscribed().is_none()
                && !is_wor_excluded_category(word)
        }

        // %pho and %sin include everything that was phonologically/gesturally produced
        // This includes fragments, untranscribed material, etc.
        TierDomain::Pho | TierDomain::Sin => true,
    }
}

/// Return whether a word category is excluded from `%wor`.
///
/// Excluded: phonological fragments (`&+`) and nonwords (`&~`).
/// NOT excluded: fillers (`&-`), which have stable, alignable phoneme sequences.
///
/// This matches the BA2-era reference: both `&+` and `&~` were
/// `TokenType.ANNOT` (excluded from `phonated_words`), while `&-` was
/// `TokenType.FP` (included). See the BA2 `formats/chat/lexer.py`
/// `__handle` method.
fn is_wor_excluded_category(word: &Word) -> bool {
    matches!(
        word.category,
        Some(WordCategory::Nonword | WordCategory::PhonologicalFragment)
    )
}

/// Return whether the word category is fragment-like for strict domains.
///
/// Fragment-like categories are filtered only in stricter domains like `%mor`.
///
/// This is [`crate::model::WordMaterial::Sound`] under an alignment-side name:
/// `%mor` tags words, and a rendering of a noise is not one. Delegating rather
/// than listing the three categories again keeps it in step with the lexical
/// rules asking the same question; it was one of four hand-written copies.
fn is_fragment_like(word: &Word) -> bool {
    matches!(word.material(), crate::model::WordMaterial::Sound)
}

/// Return whether a word contributes linguistic content for `%mor` alignment.
///
/// Excludes:
/// - All fragments: &-markers, nonwords, fillers, phonological fragments
/// - Untranscribed material: xxx, yyy, www
fn is_linguistic_content(word: &Word) -> bool {
    !is_fragment_like(word) && word.untranscribed().is_none()
}

/// Return whether a word token is `%wor` timing metadata (`start_end` digits).
///
/// These tokens are alignment metadata rather than lexical items and therefore
/// must be excluded from lexical alignment counts.
fn is_wor_timing_token(word: &Word) -> bool {
    // %wor tiers interleave lexical tokens with timing markers like `100_200`.
    // Those markers are alignment metadata, not alignable lexical content.
    let raw = word.raw_text.as_bytes();
    let Some(split_at) = raw.iter().position(|&byte| byte == b'_') else {
        return false;
    };
    if split_at == 0 || split_at + 1 >= raw.len() || raw[split_at + 1..].contains(&b'_') {
        return false;
    }

    raw[..split_at].iter().all(|byte| byte.is_ascii_digit())
        && raw[split_at + 1..].iter().all(|byte| byte.is_ascii_digit())
}

/// Return whether a replaced word should align in `%pho`/`%sin` domains.
///
/// Omissions never align. Fragment-like words are excluded when a replacement
/// exists.
///
/// Takes the whole `ReplacedWord`, not `(&Word, has_replacement: bool)`. The
/// bool was a DERIVED value the callee can compute: all five call sites already
/// held this struct and all five passed the identical
/// `!entry.replacement.words.is_empty()`, so the only thing the parameter added
/// was five chances to pass the wrong expression, in a predicate whose answer
/// changes what aligns.
pub fn should_align_replaced_word_in_pho_sin(replaced: &ReplacedWord) -> bool {
    if replaced
        .word
        .category
        .as_ref()
        .is_some_and(WordCategory::is_omission)
    {
        return false;
    }

    if !replaced.replacement.words.is_empty() && is_fragment_like(&replaced.word) {
        return false;
    }

    true
}

/// Return whether the separator contributes a `%mor` tag-marker item.
///
/// These separators map to explicit `%mor` symbols and therefore count as
/// alignable units in morphological alignment.
pub fn is_tag_marker_separator(sep: &Separator) -> bool {
    // Tag markers that have corresponding %mor items:
    // - Tag („) -> end|end
    // - Vocative (‡) -> beg|beg
    // - Comma (,) -> cm|cm (used as tag marker in some corpora)
    matches!(
        sep,
        Separator::Tag { .. } | Separator::Vocative { .. } | Separator::Comma { .. }
    )
}
