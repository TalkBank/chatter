//! The utterance sequence the cross-utterance checks are allowed to see.
//!
//! In its own module ON PURPOSE. The type first lived in `cross_utterance/mod`
//! as a tuple struct with a private field, and every check module is a
//! DESCENDANT of that module, so a private field was in scope for all of them:
//! any one could write `FileUtterances(vec![...])` and rebuild by hand exactly
//! the thing the type exists to make unforgeable. A sibling module cannot, so
//! `of` is the only way in and no prose is needed to say so.

use crate::model::Utterance;

/// Every utterance of ONE file, in document order.
///
/// # Why this is a type and not `&[Utterance]`
///
/// Every check in this module reasons about NEIGHBOURS (`utterances[idx - 1]`)
/// or about balance across the whole file (a `[- eng]` opened in one utterance
/// and closed three later). A bare slice parameter permits three states that
/// all produce confidently wrong answers, and none of which anything would
/// notice:
///
/// - a PARTIAL slice, where every boundary answer is computed against the
///   wrong neighbour and an unbalanced marker looks unbalanced because its
///   partner was cropped out;
/// - a CONCATENATION of two files, where a marker opened in one closes in the
///   other;
/// - a REORDERING, where "follows" and "precedes" mean nothing.
///
/// Constructible only from a [`ChatFile`], which excludes the PARTIAL slice
/// outright: there is no way to hand these checks a crop of a file.
///
/// Be exact about the other two, because the first version of this comment was
/// not. `ChatFile::new` is public and takes an arbitrary `Vec<Line>`, so a
/// caller determined to concatenate or reorder can still build a `ChatFile`
/// that says so, and this type will faithfully report it. What is proved here
/// is narrower and still worth having: these are ALL the utterances of ONE
/// `ChatFile` value, in that value's line order. Moving the remaining two
/// invariants would mean constraining `ChatFile::new`, which is a separate
/// change with its own blast radius.
///
/// # It also holds references
///
/// The predecessor deep-cloned every `Utterance` in the file to obtain a
/// contiguous slice, because utterances are interleaved with comments in
/// `ChatFile::lines` and so are not contiguous. That clone measured 2.8% of
/// validate CPU, roughly 270 CPU-seconds per corpus run, more than the entire
/// language pass, to build something read-only and discarded immediately.
pub(crate) struct FileUtterances<'a>(Vec<&'a Utterance>);

impl<'a> FileUtterances<'a> {
    /// Every utterance of `file`, in document order.
    ///
    /// The only production constructor, deliberately: see the type docs.
    /// Generic over the file's VALIDATION STATE: a file's utterances are the
    /// same sequence whether or not it has been validated yet, so requiring one
    /// state here would force callers to launder the other.
    pub(crate) fn of(file: &'a crate::model::ChatFile) -> Self {
        Self(file.utterances().collect())
    }

    /// The utterance at `index`, or `None` past the end.
    ///
    /// `Option` rather than panicking indexing, because the callers compute
    /// neighbours (`idx - 1`, `idx + 1`) and a bounds question is a real
    /// answer at the first and last utterance rather than a bug.
    pub(crate) fn get(&self, index: usize) -> Option<&'a Utterance> {
        self.0.get(index).copied()
    }

    /// Every utterance, in document order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &'a Utterance> + '_ {
        self.0.iter().copied()
    }

    /// The utterances AFTER `index`, in document order.
    ///
    /// Named rather than exposing slicing: `utterances[idx + 1..]` reads as an
    /// arithmetic detail, and an off-by-one there silently includes the
    /// utterance being checked in its own neighbourhood.
    pub(crate) fn following(&self, index: usize) -> impl Iterator<Item = &'a Utterance> + '_ {
        self.0.iter().skip(index + 1).copied()
    }

    /// The utterances BEFORE `index`, NEAREST FIRST.
    ///
    /// The order every "precedes" rule wants, so the reversal lives here once
    /// instead of at each call site as `[..idx].iter().rev()`.
    pub(crate) fn preceding(&self, index: usize) -> impl Iterator<Item = &'a Utterance> + '_ {
        self.0.iter().take(index).rev().copied()
    }

    /// Consecutive pairs, for rules about an utterance and the one after it.
    ///
    /// The `windows(2)` a slice gave for free, named, so the pair's ORDER is
    /// stated rather than implied by index arithmetic at the call site.
    pub(crate) fn consecutive_pairs(
        &self,
    ) -> impl Iterator<Item = (&'a Utterance, &'a Utterance)> + '_ {
        self.0.windows(2).map(|pair| (pair[0], pair[1]))
    }
}
