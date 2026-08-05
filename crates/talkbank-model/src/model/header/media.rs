//! Typed model for the `@Media` header.
//!
//! CHAT format:
//! `@Media:\tfilename, media_type[, status]`
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

use super::{MediaStatus, MediaType, WriteChat, codes::MediaFilename};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Parsed payload of one `@Media` header line.
///
/// `filename` and `media_type` are required by CHAT. `status` is optional and
/// used by some corpora to mark missing or not-yet-linked assets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct MediaHeader {
    /// Media basename without extension.
    pub filename: MediaFilename,

    /// Capture modality token (`audio` or `video`).
    pub media_type: MediaType,

    /// Optional availability status (`missing`, `unlinked`, `notrans`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<MediaStatus>,

    /// Source span of illegal whitespace between the filename and the comma,
    /// if any. `None` when the comma follows the filename directly.
    ///
    /// PROVENANCE ONLY, exactly like `TierSeparator::trailing_space` carries
    /// the E758 fact: whether a space sat before the comma is a property of
    /// the source text, not of the header, so it is skipped on the wire, in
    /// the schema and in semantic comparison. It is retained because E767 has
    /// to report it, and a rule that lives in VALIDATION fires for every parser
    /// front end, whereas the same rule emitted from one parser's lowering
    /// fires only there. That is not hypothetical: E767 was emitted from the
    /// tree-sitter lowering when it was introduced, and the re2c front end
    /// silently did not report it, which is the same drift this crate already
    /// recorded for E758.
    #[serde(skip)]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    pub whitespace_before_comma: Option<crate::Span>,
}

impl MediaHeader {
    /// Builds an `@Media` payload with required fields.
    ///
    /// Takes a [`MediaFilename`] rather than anything string-like, so the
    /// only way to reach this constructor is through
    /// [`MediaFilename::parse`], which is where the "can this be written to a
    /// `@Media` line and read back" question is answered.
    pub fn new(filename: MediaFilename, media_type: MediaType) -> Self {
        Self {
            filename,
            media_type,
            status: None,
            whitespace_before_comma: None,
        }
    }

    /// Records illegal whitespace between the filename and the comma.
    ///
    /// Mirrors `TierSeparator::with_trailing_space`; see the field docs.
    pub fn with_whitespace_before_comma(mut self, span: crate::Span) -> Self {
        self.whitespace_before_comma = Some(span);
        self
    }

    /// Sets optional media-link status metadata.
    pub fn with_status(mut self, status: MediaStatus) -> Self {
        self.status = Some(status);
        self
    }
}

impl WriteChat for MediaHeader {
    /// Serializes canonical `@Media` text, including optional status.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        write!(
            w,
            "@Media:\t{}, {}",
            self.filename,
            self.media_type.as_str()
        )?;

        if let Some(ref status) = self.status {
            write!(w, ", {}", status.as_str())?;
        }

        Ok(())
    }
}

impl std::fmt::Display for MediaHeader {
    /// Formats the media header in canonical CHAT text form.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}
