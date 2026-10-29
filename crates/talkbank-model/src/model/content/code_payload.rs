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
    Text(TextPayload),
    UnparsedSource(SmolStr),
}

/// Text classification cannot produce retained source for an unparsed code.
#[derive(Clone, Debug, PartialEq, SemanticEq, SpanShift)]
enum TextPayload {
    Code(SmolStr),
    MissingContent(SmolStr),
}

impl TextPayload {
    fn new(text: impl Into<SmolStr>) -> Self {
        let text = text.into();
        if text.trim_end().is_empty() {
            Self::MissingContent(text)
        } else {
            Self::Code(text)
        }
    }

    fn as_str(&self) -> &str {
        match self {
            Self::Code(text) | Self::MissingContent(text) => text,
        }
    }

    fn into_text(self) -> SmolStr {
        match self {
            Self::Code(text) | Self::MissingContent(text) => text,
        }
    }
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
            State::Text(text) => Self::Text {
                text: text.into_text(),
            },
            State::UnparsedSource(unparsed) => Self::Unparsed { unparsed },
        }
    }
}

impl CodePayload {
    pub(super) fn new(text: impl Into<SmolStr>) -> Self {
        Self {
            state: State::Text(TextPayload::new(text)),
        }
    }

    pub(super) fn unparsed(source: impl Into<SmolStr>) -> Self {
        Self {
            state: State::UnparsedSource(source.into()),
        }
    }

    pub(super) fn from_lexed(payload: &str, lexeme: &str) -> Self {
        match TextPayload::new(payload.trim_end()) {
            text @ TextPayload::Code(_) => Self {
                state: State::Text(text),
            },
            TextPayload::MissingContent(_) => Self::unparsed(lexeme),
        }
    }

    pub(super) fn text(&self) -> Option<&str> {
        match &self.state {
            State::Text(TextPayload::Code(text)) => Some(text),
            State::Text(TextPayload::MissingContent(_)) | State::UnparsedSource(_) => None,
        }
    }

    pub(super) fn unparsed_source(&self) -> Option<&str> {
        match &self.state {
            State::UnparsedSource(source) => Some(source),
            State::Text(_) => None,
        }
    }

    pub(super) fn write_chat<W: crate::model::ChatWriter>(
        &self,
        opening: &str,
        w: &mut W,
    ) -> Result<(), W::Error> {
        match &self.state {
            State::Text(text) => {
                w.write_str(opening)?;
                w.write_str(text.as_str())?;
                w.write_char(']')
            }
            State::UnparsedSource(source) => w.write_str(source),
        }
    }
}
