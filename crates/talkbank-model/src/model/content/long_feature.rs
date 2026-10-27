//! Long-event boundary markers (`&{l=...` / `&}l=...`).
//!
//! These markers encode scoped phenomena that may span multiple words or even
//! multiple utterances.
//!
//! # CHAT Format References
//!
//! - [Long Event](https://talkbank.org/0info/manuals/CHAT.html#LongEvent)

use super::WriteChat;
use super::scoped_label::LongFeatureLabel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Begin boundary for a long-event scope.
///
/// A valid scope pairs this marker with a later [`LongFeatureEnd`] carrying the
/// same label.
///
/// # CHAT Format
///
/// ```text
/// &{l=LABEL
/// ```
///
/// # Examples
///
/// ```text
/// *CHI: &{l=singing happy birthday to you &}l=singing .
/// *MOT: what did you &{l=whisper say to him &}l=whisper ?
/// *CHI: &{l=overlap I want &}l=overlap one .
/// *FAT: &{l=overlap no you can't &}l=overlap .
/// ```
///
/// # Overlapping Speech
///
/// For overlapping speech between speakers, use matching `&{l=overlap` and
/// `&}l=overlap` markers in both speakers' utterances to show where the
/// simultaneous speech occurs.
///
/// # Validation
///
/// Validation checks that begin markers are properly paired
/// with matching end markers. Unpaired markers generate validation errors.
///
/// # References
///
/// - [Long Event](https://talkbank.org/0info/manuals/CHAT.html#LongEvent)
/// - [Overlaps](https://talkbank.org/0info/manuals/CHAT.html#Overlaps)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct LongFeatureBegin {
    /// Label used to pair begin/end boundaries.
    pub label: LongFeatureLabel,

    /// Source location metadata for diagnostics (not serialized).
    #[serde(skip, default = "crate::Location::unlocated")]
    #[schemars(skip)]
    pub location: crate::Location,
}

impl LongFeatureBegin {
    /// Construct this marker with an admitted label and explicit source location.
    ///
    /// ```compile_fail
    /// use talkbank_model::{LongFeatureBegin, Location};
    /// let marker = LongFeatureBegin::new(Location::Unlocated, "label");
    /// ```
    pub fn new(location: impl Into<crate::Location>, label: LongFeatureLabel) -> Self {
        Self {
            label,
            location: location.into(),
        }
    }
}

impl WriteChat for LongFeatureBegin {
    /// Writes this begin marker in CHAT long-event syntax (`&{l=...`).
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        {
            w.write_str("&{")?;
            w.write_str("l=")?;
            crate::model::WriteChat::write_chat(&self.label, w)
        }
    }
}

impl std::fmt::Display for LongFeatureBegin {
    /// Formats this begin marker in canonical CHAT text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

/// End boundary for a long-event scope.
///
/// A valid scope pairs this marker with an earlier [`LongFeatureBegin`] carrying
/// the same label.
///
/// # CHAT Format
///
/// ```text
/// &}l=LABEL
/// ```
///
/// # Examples
///
/// ```text
/// *CHI: &{l=singing happy birthday to you &}l=singing .
/// *MOT: what did you &{l=whisper say to him &}l=whisper ?
/// ```
///
/// The end marker closes the scope opened by the matching begin marker.
///
/// # Cross-Utterance Scopes
///
/// Long event scopes can span multiple utterances:
///
/// ```text
/// *CHI: &{l=singing happy birthday to you .
/// *CHI: happy birthday dear mommy &}l=singing .
/// ```
///
/// # Validation
///
/// The source span enables validation that:
/// - Each end marker has a matching begin marker
/// - Labels match between paired begin/end markers
/// - Scopes are properly nested (no crossed scopes)
///
/// # References
///
/// - [Long Event](https://talkbank.org/0info/manuals/CHAT.html#LongEvent)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct LongFeatureEnd {
    /// Label used to pair begin/end boundaries.
    pub label: LongFeatureLabel,

    /// Source location metadata for diagnostics (not serialized).
    #[serde(skip, default = "crate::Location::unlocated")]
    #[schemars(skip)]
    pub location: crate::Location,
}

impl LongFeatureEnd {
    /// Construct this marker with an admitted label and explicit source location.
    ///
    /// ```compile_fail
    /// use talkbank_model::{LongFeatureEnd, Location};
    /// let marker = LongFeatureEnd::new(Location::Unlocated, "label");
    /// ```
    pub fn new(location: impl Into<crate::Location>, label: LongFeatureLabel) -> Self {
        Self {
            label,
            location: location.into(),
        }
    }
}

impl WriteChat for LongFeatureEnd {
    /// Writes this end marker in CHAT long-event syntax (`&}l=...`).
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        {
            w.write_str("&}")?;
            w.write_str("l=")?;
            crate::model::WriteChat::write_chat(&self.label, w)
        }
    }
}

impl std::fmt::Display for LongFeatureEnd {
    /// Formats this end marker in canonical CHAT text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn scoped_marker_wire_refuses_empty_and_delimited_labels() {
        use crate::{LongFeatureBegin, LongFeatureEnd, NonvocalBegin, NonvocalEnd, NonvocalSimple};
        for label in ["", "two words", "singing}", "laugh\n*CHI:\tsecret", "café"] {
            let wire = serde_json::json!({"label": label});
            assert!(
                serde_json::from_value::<LongFeatureBegin>(wire.clone()).is_err(),
                "{label:?}"
            );
            assert!(
                serde_json::from_value::<LongFeatureEnd>(wire.clone()).is_err(),
                "{label:?}"
            );
            assert!(
                serde_json::from_value::<NonvocalBegin>(wire.clone()).is_err(),
                "{label:?}"
            );
            assert!(
                serde_json::from_value::<NonvocalEnd>(wire.clone()).is_err(),
                "{label:?}"
            );
            assert!(
                serde_json::from_value::<NonvocalSimple>(wire).is_err(),
                "{label:?}"
            );
        }
    }
}
