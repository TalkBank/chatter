//! Begin marker for scoped nonvocal events (`&{n=label`).
//!
//! This marker opens a labeled nonvocal scope that may span words, groups, or
//! even utterance boundaries until a matching [`NonvocalEnd`](super::NonvocalEnd)
//! closes it.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#LongNonverbalEvent>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

use super::super::WriteChat;
use super::NonvocalLabel;

/// Begin boundary for a scoped nonvocal event.
///
/// A valid scope pairs this marker with a later [`super::NonvocalEnd`] using
/// the same label.
///
/// # CHAT Format
///
/// ```text
/// &{n=LABEL
/// ```
///
/// # Examples
///
/// ```text
/// *CHI: &{n=crying I want mommy .
/// *CHI: please &}n=crying .
/// ```
///
/// The crying event begins in the first utterance and ends in the second.
///
/// # Validation
///
/// The source span enables validation that begin markers are properly paired
/// with matching end markers. Unpaired markers generate validation errors.
///
/// # References
///
/// - [Long Nonverbal Event](https://talkbank.org/0info/manuals/CHAT.html#LongNonverbalEvent)
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct NonvocalBegin {
    /// Label used to pair begin/end boundaries.
    pub label: NonvocalLabel,

    /// Source location metadata for diagnostics (not serialized).
    #[serde(skip, default = "crate::Location::unlocated")]
    #[schemars(skip)]
    pub location: crate::Location,
}

impl PartialEq for NonvocalBegin {
    /// Equality is label-based; source span is intentionally ignored.
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label
    }
}

impl Eq for NonvocalBegin {}

impl NonvocalBegin {
    /// Construct this marker with an admitted label and explicit source location.
    ///
    /// ```compile_fail
    /// use talkbank_model::{NonvocalBegin, Location};
    /// let marker = NonvocalBegin::new(Location::Unlocated, "label");
    /// ```
    pub fn new(location: impl Into<crate::Location>, label: NonvocalLabel) -> Self {
        Self {
            label,
            location: location.into(),
        }
    }
}

impl WriteChat for NonvocalBegin {
    /// Writes this nonvocal begin marker in CHAT format.
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        {
            w.write_str("&{")?;
            w.write_str("n=")?;
            crate::model::WriteChat::write_chat(&self.label, w)
        }
    }
}

impl std::fmt::Display for NonvocalBegin {
    /// Formats this begin marker in canonical CHAT text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}
