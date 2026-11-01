//! The text a closed vocabulary does not know, proven to be neither blank
//! nor one of its known spellings.
//!
//! Every [`closed_vocabulary!`](crate::closed_vocabulary) enum keeps any text
//! it does not recognize as `Unsupported`, so the validator can name it. If
//! that variant held a plain string, a caller could build
//! `Unsupported(" ")` (blank, which is the empty header's job) or
//! `MediaType::Unsupported("audio")` (a known value wearing the unsupported
//! variant, which validation would then wrongly refuse). The payload is this
//! type instead, which only the vocabulary's own `from_text` mints, after its
//! match has refused every known spelling, from [`HeaderText`] the one
//! emptiness decision admitted.

use crate::model::{HeaderText, NonEmptyString};
use std::marker::PhantomData;

/// Text that vocabulary `V` does not know: nonblank, and none of `V`'s
/// known spellings.
///
/// The type parameter is the vocabulary, so one vocabulary's unsupported
/// value cannot be moved into another's. The only constructor is
/// crate-internal and called only by the `from_text` that
/// [`closed_vocabulary!`](crate::closed_vocabulary) generates.
///
/// ```compile_fail
/// # use talkbank_model::model::{MediaType, UnsupportedSpelling};
/// let forged: UnsupportedSpelling<MediaType> = UnsupportedSpelling::unmatched_by_vocabulary(todo!());
/// ```
#[derive(talkbank_derive::SemanticEq, talkbank_derive::SpanShift)]
pub struct UnsupportedSpelling<V> {
    text: NonEmptyString,
    /// The vocabulary is a marker: no meaning, no span.
    #[semantic_eq(skip)]
    #[span_shift(skip)]
    vocabulary: PhantomData<fn() -> V>,
}

impl<V> UnsupportedSpelling<V> {
    /// Keeps `text` as unsupported. Called only by a vocabulary's
    /// `from_text`, in the arm its match reaches after refusing every known
    /// spelling; `text` is nonblank because it is [`HeaderText`].
    pub(crate) fn unmatched_by_vocabulary(text: HeaderText<'_>) -> Self {
        Self {
            text: text.as_non_empty().into_owned(),
            vocabulary: PhantomData,
        }
    }

    /// The text as written.
    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }
}

// The standard trait impls are written by hand so they hold for every
// vocabulary `V` without asking `V` to implement anything: `V` is only a
// marker. The model traits are derived, skipping the marker.

impl<V> Clone for UnsupportedSpelling<V> {
    fn clone(&self) -> Self {
        Self {
            text: self.text.clone(),
            vocabulary: PhantomData,
        }
    }
}

impl<V> std::fmt::Debug for UnsupportedSpelling<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("UnsupportedSpelling")
            .field(&self.text.as_str())
            .finish()
    }
}

impl<V> PartialEq for UnsupportedSpelling<V> {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl<V> Eq for UnsupportedSpelling<V> {}

impl<V> std::hash::Hash for UnsupportedSpelling<V> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.text.hash(state);
    }
}

impl<V> std::fmt::Display for UnsupportedSpelling<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<V> std::ops::Deref for UnsupportedSpelling<V> {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}
