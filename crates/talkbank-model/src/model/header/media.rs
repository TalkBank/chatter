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

    /// What this header claims about whether a recording EXISTS.
    ///
    /// The answer is a function of the `(media_type, status)` PAIR, and that
    /// pair is what consumers kept getting wrong. Its combinations include ones
    /// with no meaning (`missing` type beside an `unlinked` status), and the
    /// distinction that matters is not the one the shape suggests: `unlinked`
    /// and `notrans` sit in the same `Option<MediaStatus>` slot as `missing`
    /// and mean the OPPOSITE, that the recording is there.
    ///
    /// It was documented on [`MediaStatus`] in prose and derived nowhere, so
    /// every consumer re-decided it. Two independent ones wrote
    /// `status.is_some()` and thereby told operators that a transcript awaiting
    /// forced alignment had declared its media absent, on the exact command
    /// that exists to align it.
    pub fn declared_recording(&self) -> DeclaredRecording {
        // Exhaustive over both halves on purpose: a status added to CHAT has to
        // be classified here, where the meaning is documented, rather than
        // falling into a catch-all in whichever crate happens to read it next.
        match (&self.media_type, &self.status) {
            (MediaType::Missing, _) | (_, Some(MediaStatus::Missing)) => DeclaredRecording::Absent,
            (_, Some(MediaStatus::Unlinked | MediaStatus::Notrans))
            | (_, Some(MediaStatus::Unsupported(_)))
            | (_, None) => DeclaredRecording::Expected,
        }
    }
}

/// What an `@Media` header claims about the recording's existence.
///
/// Two variants because two is what the question has, and the finer states
/// (linked, unlinked, untranscribed) remain on [`MediaStatus`] for anyone who
/// needs them. Deliberately NOT a `bool`: `is_missing()` reads as a property of
/// the header, and the fact wanted is a claim about a file somewhere else.
///
/// A claim, never a measurement. Measured over one 106k-file corpus, no
/// transcript declaring `missing` had a recording on disk, but that is an
/// observation about today's data and not a licence to skip looking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeclaredRecording {
    /// `@Media: name, missing`, or a `missing` status. The transcript says the
    /// recording is gone or was never captured.
    Absent,
    /// Everything else, including `unlinked` and `notrans`, both of which
    /// assert that the recording EXISTS.
    Expected,
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

#[cfg(test)]
mod declared_recording_tests {
    use super::*;
    use crate::model::header::codes::MediaFilename;

    fn header(media_type: MediaType, status: Option<MediaStatus>) -> MediaHeader {
        let filename = MediaFilename::parse("sample").expect("a bare stem is a valid filename");
        let header = MediaHeader::new(filename, media_type);
        match status {
            Some(status) => header.with_status(status),
            None => header,
        }
    }

    #[test]
    fn a_plain_declaration_expects_a_recording() {
        assert_eq!(
            header(MediaType::Audio, None).declared_recording(),
            DeclaredRecording::Expected
        );
    }

    #[test]
    fn unlinked_and_notrans_expect_a_recording() {
        // The distinction this method exists for. Both sit in the same
        // `Option<MediaStatus>` slot as `missing` and mean the opposite: the
        // recording is there, it just has no bullets (or no transcription) yet.
        // Two downstream consumers independently wrote `status.is_some()` and
        // so reported a transcript awaiting forced alignment as having declared
        // its media absent, on the very command that exists to align it.
        for status in [MediaStatus::Unlinked, MediaStatus::Notrans] {
            assert_eq!(
                header(MediaType::Audio, Some(status.clone())).declared_recording(),
                DeclaredRecording::Expected,
                "{status:?} asserts the recording exists"
            );
        }
    }

    #[test]
    fn only_missing_declares_a_recording_absent() {
        assert_eq!(
            header(MediaType::Audio, Some(MediaStatus::Missing)).declared_recording(),
            DeclaredRecording::Absent
        );
        assert_eq!(
            header(MediaType::Missing, None).declared_recording(),
            DeclaredRecording::Absent
        );
    }

    #[test]
    fn an_unrecognised_status_still_expects_a_recording() {
        // An unsupported token is a validation problem, and validation reports
        // it. It is not a reason to conclude the recording is gone, which would
        // make a typo suppress alignment.
        assert_eq!(
            header(
                MediaType::Audio,
                Some(MediaStatus::Unsupported("wat".to_owned()))
            )
            .declared_recording(),
            DeclaredRecording::Expected
        );
    }
}
