//! Closed vocabularies used by specific CHAT headers.
//!
//! These enums model header payloads with known variants plus an `Unsupported`
//! fallback for values the parser encounters but this implementation does not
//! recognize. The parser always succeeds; the *validator* flags unsupported
//! values. An unsupported header value is never blank: blank header content
//! is `Header::EmptyContent`, and a blank `@Media` field is a malformed
//! header.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Number_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Recording_Quality_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Transcription_Header>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift, ValidationTagged};

/// `@ID` sex field values.
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift, ValidationTagged)]
pub enum Sex {
    /// `male`
    Male,
    /// `female`
    Female,
    /// Unrecognized value preserved for validation.
    Unsupported(String),
}

impl Sex {
    /// Maps canonical CHAT token text to a typed sex value.
    ///
    /// Unknown tokens yield `Unsupported` so the validator can flag them.
    pub fn from_text(value: &str) -> Self {
        match value {
            "male" => Self::Male,
            "female" => Self::Female,
            _ => Self::Unsupported(value.to_string()),
        }
    }

    /// Returns the canonical CHAT token for this sex value.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Male => "male",
            Self::Female => "female",
            Self::Unsupported(s) => s.as_str(),
        }
    }
}

impl Serialize for Sex {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Sex {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_text(&s))
    }
}

impl JsonSchema for Sex {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Sex".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

// ---------------------------------------------------------------------------
// Header closed vocabularies
// ---------------------------------------------------------------------------
//
// One macro owns the shape every one of these shares (known values, a
// nonblank `Unsupported`, `from_text`/`as_str`, serialization, a JSON route
// that refuses blank text, the schema), so they cannot drift apart; each
// vocabulary below is only its values.

crate::closed_vocabulary! {
    /// `@Recording Quality` ratings.
    pub enum RecordingQuality {
        /// Level 1.
        Quality1 => "1",
        /// Level 2.
        Quality2 => "2",
        /// Level 3.
        Quality3 => "3",
        /// Level 4.
        Quality4 => "4",
        /// Level 5.
        Quality5 => "5",
    }
}

crate::closed_vocabulary! {
    /// `@Transcription` values.
    pub enum Transcription {
        /// `eye_dialect`
        EyeDialect => "eye_dialect",
        /// `partial`
        Partial => "partial",
        /// `full`
        Full => "full",
        /// `detailed`
        Detailed => "detailed",
        /// `coarse`
        Coarse => "coarse",
        /// `checked`
        Checked => "checked",
        /// `anonymized`
        Anonymized => "anonymized",
    }
}

crate::closed_vocabulary! {
    /// Optional third token in `@Media` describing the link/transcription state.
    ///
    /// These statuses describe the **relationship between the transcript and the
    /// media**, NOT whether the media file exists on disk. A file marked
    /// `unlinked` almost certainly has media available, the transcriber just
    /// hasn't aligned (bulletted) the utterances to timestamps yet.
    ///
    /// # CHAT Manual Reference
    ///
    /// <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
    pub enum MediaStatus {
        /// `missing`, the media file is known to be absent or lost.
        /// Processing commands that need audio (align, transcribe) should
        /// skip this file with a clear diagnostic.
        Missing => "missing",
        /// `unlinked`, the media file EXISTS but utterances have not been
        /// aligned to timestamps yet (no bullets / time marks).
        /// This is the NORMAL state for a transcript before forced alignment.
        /// Processing commands SHOULD resolve and use the media, the whole
        /// point of `align` is to create the links that are currently absent.
        Unlinked => "unlinked",
        /// `notrans`, the media exists but no transcription has been done.
        /// The file may contain only headers and `@Comment` lines.
        Notrans => "notrans",
    }
}

crate::closed_vocabulary! {
    /// Required second token in `@Media` describing capture modality.
    pub enum MediaType {
        /// `audio`
        Audio => "audio",
        /// `video`
        Video => "video",
        /// `missing`, media file itself is absent.
        Missing => "missing",
    }
}

crate::closed_vocabulary! {
    /// `@Number` values describing participant count scope.
    pub enum Number {
        /// `1`
        Number1 => "1",
        /// `2`
        Number2 => "2",
        /// `3`
        Number3 => "3",
        /// `4`
        Number4 => "4",
        /// `5`
        Number5 => "5",
        /// `more`
        More => "more",
        /// `audience`
        Audience => "audience",
    }
}
