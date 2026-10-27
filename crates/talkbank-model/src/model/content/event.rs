//! Model for CHAT event tokens (`&=...`).
//!
//! Events represent non-speech sounds/actions inline in main-tier content.
//!
//! # CHAT Format References
//!
//! - [Simple Events](https://talkbank.org/0info/manuals/CHAT.html#SimpleEvents)
//! - [Local Events](https://talkbank.org/0info/manuals/CHAT.html#Local_Event)

use super::WriteChat;
use crate::generated::symbol_sets::{EVENT_SEGMENT_FORBIDDEN_PATTERN, is_event_segment_char};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// A nonempty event descriptor admitted by the shared registry character class.
/// Project-specific labels remain open vocabulary; CHAT delimiters and NUL do not.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SemanticEq, SpanShift)]
#[serde(try_from = "String")]
pub struct EventType(smol_str::SmolStr);

/// Why raw text cannot be used as one event description.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InvalidEventType {
    /// An event marker must have a description.
    #[error("event description is empty")]
    Empty,
    /// A syntax delimiter cannot be part of an event description.
    #[error("forbidden event character {character:?} at byte {byte_offset}")]
    ForbiddenCharacter {
        /// The refused Unicode scalar.
        character: char,
        /// Its UTF-8 byte offset in the description.
        byte_offset: usize,
    },
}

impl EventType {
    /// Admit a single event descriptor, without its `&=` prefix.
    pub fn new(text: impl AsRef<str>) -> Result<Self, InvalidEventType> {
        let text = text.as_ref();
        if text.is_empty() {
            return Err(InvalidEventType::Empty);
        }
        if let Some((byte_offset, character)) = text
            .char_indices()
            .find(|(_, c)| !is_event_segment_char(*c))
        {
            return Err(InvalidEventType::ForbiddenCharacter {
                character,
                byte_offset,
            });
        }
        Ok(Self(smol_str::SmolStr::new(text)))
    }

    /// Borrow the admitted description.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<String> for EventType {
    type Error = InvalidEventType;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::new(text)
    }
}

impl TryFrom<&str> for EventType {
    type Error = InvalidEventType;
    fn try_from(text: &str) -> Result<Self, Self::Error> {
        Self::new(text)
    }
}

impl JsonSchema for EventType {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "EventType".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string", "minLength": 1,
            "not": { "pattern": EVENT_SEGMENT_FORBIDDEN_PATTERN }
        })
    }
}

impl WriteChat for EventType {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        w.write_str(self.as_str())
    }
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Inline non-speech event marker.
///
/// Serializes as `&=<event_type>`.
///
/// # CHAT Format Examples
///
/// ```text
/// *MOT: hello &=laughs there !
/// *CHI: um &=coughs I don't know .
/// ```
///
/// # References
///
/// - [Simple Events](https://talkbank.org/0info/manuals/CHAT.html#SimpleEvents)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct Event {
    /// Event descriptor token (for example `laughs`, `coughs`, `clears:throat`).
    pub event_type: EventType,

    /// Source location metadata for diagnostics (not serialized).
    #[serde(skip, default = "crate::Location::unlocated")]
    #[schemars(skip)]
    pub location: crate::Location,
}

impl Event {
    /// Build an event from an admitted descriptor and explicit source location.
    ///
    /// ```compile_fail
    /// use talkbank_model::{Event, Location};
    /// let event = Event::new(Location::Unlocated, "laughs");
    /// ```
    pub fn new(location: impl Into<crate::Location>, event_type: EventType) -> Self {
        Self {
            event_type,
            location: location.into(),
        }
    }
}

impl WriteChat for Event {
    /// Serializes canonical CHAT event syntax (`&=<event_type>`).
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        w.write_str("&=")?;
        self.event_type.write_chat(w)
    }
}

impl std::fmt::Display for Event {
    /// Formats the event in CHAT form (`&=<event_type>`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_wire_refuses_empty_and_delimited_descriptions() {
        for text in [
            "",
            "two words",
            "laughs.",
            "laughs\n*CHI:\tsecret",
            "cough°",
            "laughs\0",
        ] {
            let wire = serde_json::json!({"event_type": text});
            assert!(
                serde_json::from_value::<Event>(wire).is_err(),
                "admitted {text:?}"
            );
        }
    }

    #[test]
    fn event_admission_keeps_open_labels_and_reports_utf8_boundaries() {
        for text in [
            "laughs",
            "clears:throat",
            "ˈcough",
            "ˌcough",
            "„cough",
            "‡cough",
            "ca=fé",
            "custom_label",
        ] {
            let admitted = EventType::new(text).unwrap();
            assert_eq!(admitted.as_str(), text);
            assert_eq!(serde_json::to_value(&admitted).unwrap(), text);
            assert_eq!(
                serde_json::from_value::<EventType>(serde_json::json!(text)).unwrap(),
                admitted
            );
        }
        assert_eq!(EventType::new(""), Err(InvalidEventType::Empty));
        assert_eq!(
            EventType::new("é°"),
            Err(InvalidEventType::ForbiddenCharacter {
                character: '°',
                byte_offset: 2,
            })
        );
        use crate::generated::symbol_sets::*;
        for forbidden in EVENT_SEGMENT_FORBIDDEN_SYMBOLS
            .iter()
            .chain(EVENT_SEGMENT_FORBIDDEN_COMMON_SYMBOLS)
            .chain(WORD_ATTACHED_SYMBOLS)
            .chain(PAIRED_STRETCH_SYMBOLS)
            .copied()
            .chain(["\0"])
        {
            for text in [
                format!("{forbidden}laughs"),
                format!("la{forbidden}ughs"),
                format!("laughs{forbidden}"),
            ] {
                assert!(EventType::new(&text).is_err(), "{text:?}");
                assert!(serde_json::from_value::<EventType>(serde_json::json!(text)).is_err());
            }
        }
    }
}
