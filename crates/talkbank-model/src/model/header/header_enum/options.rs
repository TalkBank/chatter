//! Supported `@Options` tokens with parser-facing semantics.
//!
//! `@Options` values are parsed into [`ChatOptionFlag`] so downstream code can
//! branch on behavior (`CA` parsing rules, bullet handling) without ad hoc
//! string checks. Unrecognized values are stored as `Unsupported` so the
//! validator can flag them.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Options_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Option>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Unicode_Option>

crate::closed_vocabulary! {
    /// `@Options` tokens with behavior in this implementation.
    ///
    /// Known flags carry parser-facing semantics (CA mode, alignment skip).
    /// Unrecognized values are preserved for validation but do not affect
    /// parsing. A flag is never blank: `ChatOptionFlags::parse` refuses an
    /// empty position.
    pub enum ChatOptionFlag {
        /// `CA`: enable Conversation Analysis mode.
        Ca => "CA",
        /// `NoAlign`: skip forced alignment for this file.
        NoAlign => "NoAlign",
    }
}

/// An effect that `@Options: CA` has on how a file is read.
///
/// **`@Options: CA` is a per-file flag for material judged specifically weird
/// CA. It is NOT a declaration that a file uses CA-originated notation.** A
/// transcript can use CA notation throughout and need no option at all: a
/// fixture carrying the manual's own CA and disfluency examples with no option
/// is accepted by both CLAN CHECK and `chatter validate`. The standing campaign
/// is to get corpora clean enough to DROP the option while keeping their
/// markup, so "drop the option" never means "remove the notation".
///
/// This replaced an `enables_ca_mode` predicate, whose name ran together the
/// three independent things "CA" is overloaded across here: where a symbol's
/// notation came from, what the parser does with a symbol, and this per-file
/// flag. A predicate reading "is this file CA" is exactly what invites gating
/// SYMBOL ADMISSIBILITY on the option, which would be wrong for every symbol,
/// including the ones that really are CA-originated.
///
/// **The two effects are not the same KIND of thing, which is why they are
/// named separately rather than lumped under one "leniency" predicate.** One
/// waives a requirement; the other changes what a construct MEANS. Calling both
/// leniency would be a quieter version of the same conflation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaOptionEffect {
    /// An utterance need not carry a terminator. A WAIVER: the same text is
    /// read the same way, and one rule stops being enforced.
    TerminatorRequirementWaived,
    /// A standalone parenthetical is a CA omission rather than an error. A
    /// REINTERPRETATION: the same bytes mean something different.
    ParentheticalIsCaOmission,
}

impl ChatOptionFlag {
    /// Returns `true` when this flag produces `effect`.
    ///
    /// Exhaustive on `effect` on purpose: a new effect, or a second flag that
    /// produces one, breaks this match rather than silently inheriting `Ca`'s
    /// answer.
    pub fn has_effect(&self, effect: CaOptionEffect) -> bool {
        match effect {
            CaOptionEffect::TerminatorRequirementWaived
            | CaOptionEffect::ParentheticalIsCaOmission => matches!(self, Self::Ca),
        }
    }

    /// Returns `true` when this flag indicates forced alignment should be skipped.
    pub fn skips_alignment(&self) -> bool {
        matches!(self, Self::NoAlign)
    }
}
