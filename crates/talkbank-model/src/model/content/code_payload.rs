//! Shared payload admission for bracketed open-vocabulary codes.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use talkbank_derive::{SemanticEq, SpanShift};

/// Admission state is private; wire import must pass through the same constructor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(from = "CodePayloadWire", into = "CodePayloadWire")]
#[schemars(with = "CodePayloadWire")]
pub(super) struct CodePayload {
    state: State,
}

#[derive(Clone, Debug, PartialEq, SemanticEq, SpanShift)]
enum State {
    Code(SmolStr),
    MissingContent(SmolStr),
    UnparsedSource(SmolStr),
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
enum CodePayloadWire {
    Text { text: SmolStr },
    Unparsed { unparsed: SmolStr },
}

impl From<CodePayloadWire> for CodePayload {
    fn from(wire: CodePayloadWire) -> Self {
        match wire {
            CodePayloadWire::Text { text } => Self::new(text),
            CodePayloadWire::Unparsed { unparsed } => Self::unparsed(unparsed),
        }
    }
}

impl From<CodePayload> for CodePayloadWire {
    fn from(value: CodePayload) -> Self {
        match value.state {
            State::Code(text) | State::MissingContent(text) => Self::Text { text },
            State::UnparsedSource(unparsed) => Self::Unparsed { unparsed },
        }
    }
}

impl CodePayload {
    pub(super) fn new(text: impl Into<SmolStr>) -> Self {
        let text = text.into();
        let state = if text.trim_end().is_empty() {
            State::MissingContent(text)
        } else {
            State::Code(text)
        };
        Self { state }
    }

    pub(super) fn unparsed(source: impl Into<SmolStr>) -> Self {
        Self {
            state: State::UnparsedSource(source.into()),
        }
    }

    pub(super) fn from_lexed(payload: &str, lexeme: &str) -> Self {
        let value = Self::new(payload.trim_end());
        match value.state {
            State::Code(_) => value,
            State::MissingContent(_) => Self::unparsed(lexeme),
            State::UnparsedSource(_) => value,
        }
    }

    pub(super) fn text(&self) -> Option<&str> {
        match &self.state {
            State::Code(text) => Some(text),
            State::MissingContent(_) | State::UnparsedSource(_) => None,
        }
    }

    pub(super) fn unparsed_source(&self) -> Option<&str> {
        match &self.state {
            State::UnparsedSource(source) => Some(source),
            State::Code(_) | State::MissingContent(_) => None,
        }
    }

    pub(super) fn write_chat<W: crate::model::ChatWriter>(
        &self,
        opening: &str,
        w: &mut W,
    ) -> Result<(), W::Error> {
        match &self.state {
            State::Code(text) | State::MissingContent(text) => {
                w.write_str(opening)?;
                w.write_str(text)?;
                w.write_char(']')
            }
            State::UnparsedSource(source) => w.write_str(source),
        }
    }
}
