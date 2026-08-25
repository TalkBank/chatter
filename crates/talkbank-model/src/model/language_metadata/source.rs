//! Provenance labels for resolved word-language assignments.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SecondLanguage_Marker_Single>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SpanShift, ValidationTagged};

/// Source of language resolution for a word.
///
/// Each variant documents the mechanism it comes from. This doc used to repeat
/// them as a bullet list and the list went stale in the very commit that added
/// `SpanShortcut` and `SpanExplicit`, having also shipped that way into the
/// generated JSON schema description, so the mirror is gone.
///
/// Keeping provenance separate from `WordLanguages` lets downstream tools
/// distinguish "same resolved code, different source semantics" cases, which
/// matters for corpus QA and language-switching diagnostics.
///
/// # References
///
/// - [Language Codes](https://talkbank.org/0info/manuals/CHAT.html#Language_Codes)
/// - [Language Switching](https://talkbank.org/0info/manuals/CHAT.html#Language_Switching)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SpanShift, ValidationTagged,
)]
#[serde(rename_all = "snake_case")]
pub enum LanguageSource {
    /// Resolved from `@Languages` primary language.
    ///
    /// This is the baseline path when no utterance- or word-level override applies.
    Default,

    /// Resolved from utterance-scoped marker (`[- code]`).
    ///
    /// Applies to unmarked words while the scoped tier-language override is active.
    TierScoped,

    /// Resolved from explicit word marker (`@s:code`, `@s:eng+spa`, etc.).
    ///
    /// Used when the transcription names the word language(s) directly.
    WordExplicit,

    /// Resolved from `@s` shortcut toggling rule.
    ///
    /// In dual-language contexts this flips between primary and secondary language.
    WordShortcut,

    /// Resolved from a bare code-switch ANNOTATION, `[@s]`.
    ///
    /// Covers both of its scopes, because a scoped annotation may attach to one
    /// content item without angle brackets: `<a b> [@s]` governs the words it
    /// encloses, and `hallo [@s]` governs its own. Either way the rule applied
    /// is the one bare `word@s` uses.
    ///
    /// A separate variant from [`Self::WordShortcut`] because the provenance
    /// differs even when the resolved code is identical: a consumer asking "was
    /// this marked with a suffix or with an annotation?" must be able to tell,
    /// and a shared variant would answer neither.
    SpanShortcut,

    /// Resolved from a code-switch annotation with an explicit code,
    /// `[@s:code]`, in either of the scopes described above.
    ///
    /// Distinct from [`Self::WordExplicit`] for the same reason.
    SpanExplicit,

    /// No language could be resolved.
    ///
    /// Indicates missing/ambiguous context rather than an implicit default language.
    #[validation_tag(error)]
    Unresolved,
}
