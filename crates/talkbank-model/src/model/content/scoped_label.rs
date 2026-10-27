//! One lexical admission boundary for the two distinct scoped-marker label meanings.

use super::WriteChat;
use crate::generated::symbol_sets::{
    SCOPED_MARKER_LABEL_FORBIDDEN_PATTERN, is_scoped_marker_label_char,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use talkbank_derive::{SemanticEq, SpanShift};

/// Why text cannot form a long-event or nonvocal label.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InvalidScopedLabel {
    /// A marker must name a label.
    #[error("scoped-marker label is empty")]
    Empty,
    /// The character is outside the registry-owned label alphabet.
    #[error("invalid scoped-label character {character:?} at byte {byte_offset}")]
    InvalidCharacter {
        /// Refused Unicode scalar.
        character: char,
        /// UTF-8 byte offset in the supplied label.
        byte_offset: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SemanticEq, SpanShift)]
#[serde(try_from = "String")]
struct ScopedMarkerLabel(SmolStr);

impl ScopedMarkerLabel {
    fn new(text: impl AsRef<str>) -> Result<Self, InvalidScopedLabel> {
        let text = text.as_ref();
        if text.is_empty() {
            return Err(InvalidScopedLabel::Empty);
        }
        if let Some((byte_offset, character)) = text
            .char_indices()
            .find(|(_, c)| !is_scoped_marker_label_char(*c))
        {
            return Err(InvalidScopedLabel::InvalidCharacter {
                character,
                byte_offset,
            });
        }
        Ok(Self(SmolStr::new(text)))
    }
}

impl TryFrom<String> for ScopedMarkerLabel {
    type Error = InvalidScopedLabel;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::new(text)
    }
}

impl JsonSchema for ScopedMarkerLabel {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ScopedMarkerLabel".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string", "minLength": 1,
            "not": { "pattern": SCOPED_MARKER_LABEL_FORBIDDEN_PATTERN }
        })
    }
}

// The two labels share a lexical proof, not their domain meaning. No conversion
// between them is provided; callers must name the meaning they are constructing.
macro_rules! scoped_label {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
        #[serde(transparent)]
        pub struct $name(ScopedMarkerLabel);

        impl $name {
            /// Admit a nonempty label from the registry-owned alphabet.
            pub fn new(text: impl AsRef<str>) -> Result<Self, InvalidScopedLabel> {
                ScopedMarkerLabel::new(text).map(Self)
            }
            /// Borrow the exact admitted label.
            pub fn as_str(&self) -> &str { self.0.0.as_str() }
        }
        impl TryFrom<String> for $name {
            type Error = InvalidScopedLabel;
            fn try_from(text: String) -> Result<Self, Self::Error> { Self::new(text) }
        }
        impl TryFrom<&str> for $name {
            type Error = InvalidScopedLabel;
            fn try_from(text: &str) -> Result<Self, Self::Error> { Self::new(text) }
        }
        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str { self.as_str() }
        }
        impl std::ops::Deref for $name {
            type Target = str;
            fn deref(&self) -> &str { self.as_str() }
        }
        impl WriteChat for $name {
            fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
                w.write_str(self.as_str())
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.as_str()) }
        }
    };
}

scoped_label!(
    /// Admitted label after `l=` in a long-event begin/end marker.
    /// Labels remain project-defined; only their lexical shape is restricted.
    ///
    /// ```compile_fail
    /// use talkbank_model::{LongFeatureBegin, NonvocalLabel, Location};
    /// let other = NonvocalLabel::new("label").unwrap();
    /// let marker = LongFeatureBegin::new(Location::Unlocated, other);
    /// ```
    LongFeatureLabel
);
scoped_label!(
    /// Admitted label after `n=` in a nonvocal begin/end/simple marker.
    /// The distinct type prevents substituting a long-event label by accident.
    NonvocalLabel
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_share_lexical_admission_without_losing_their_meaning() {
        for text in ["singing", "cough", "custom_1-label@x%y", "0"] {
            let long = LongFeatureLabel::new(text).unwrap();
            let nonvocal = NonvocalLabel::new(text).unwrap();
            assert_eq!(long.as_str(), text);
            assert_eq!(nonvocal.as_str(), text);
            assert_eq!(serde_json::to_value(&long).unwrap(), text);
            assert_eq!(serde_json::to_value(&nonvocal).unwrap(), text);
            assert_eq!(
                serde_json::from_value::<LongFeatureLabel>(serde_json::json!(text)).unwrap(),
                long
            );
            assert_eq!(
                serde_json::from_value::<NonvocalLabel>(serde_json::json!(text)).unwrap(),
                nonvocal
            );
        }
        for text in ["", "two labels", "label}", "é", "label\n", "label\0", "a=b"] {
            assert!(LongFeatureLabel::new(text).is_err(), "{text:?}");
            assert!(NonvocalLabel::new(text).is_err(), "{text:?}");
            assert!(serde_json::from_value::<LongFeatureLabel>(serde_json::json!(text)).is_err());
            assert!(serde_json::from_value::<NonvocalLabel>(serde_json::json!(text)).is_err());
        }
        assert_eq!(
            LongFeatureLabel::new("labelé"),
            Err(InvalidScopedLabel::InvalidCharacter {
                character: 'é',
                byte_offset: 5,
            })
        );
    }
}
