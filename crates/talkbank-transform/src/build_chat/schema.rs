//! Structured description of a transcript to assemble into CHAT.
//!
//! This is the general, text-based input to [`build_chat`](super::build_chat):
//! participants, optional media, and utterances given as pre-formatted CHAT
//! main-tier text. Any CHAT generator (the MICASE/SBCSAE converters, external
//! tools) fills this in and gets back a validated `ChatFile`.
//!
//! Raw `String` fields are parsed into typed model values at the build
//! boundary (language codes in `parser::BuildChatContext`, speaker codes and
//! roles in `headers`), following the parse-at-the-edge rule. The batchalign
//! ASR path (word-level tokens with per-word timing, retrace runs, `%wor`) is
//! deliberately NOT part of this general builder; it layers its own richer
//! schema on top downstream.

use talkbank_model::model::MediaStatus;

/// Description of a transcript to assemble into CHAT.
#[derive(Debug, Clone)]
pub struct TranscriptDescription {
    /// ISO 639-3 language codes (e.g. `["eng"]`). Empty defaults to `["eng"]`.
    pub langs: Vec<String>,
    /// Participant entries. At least one is required.
    pub participants: Vec<ParticipantDesc>,
    /// Optional media filename (e.g. `"recording.mp3"`). The extension is
    /// stripped for the `@Media` header.
    pub media_name: Option<String>,
    /// Optional media type (`"audio"` or `"video"`). Defaults to `"audio"`.
    pub media_type: Option<String>,
    /// Optional media linkage status for the `@Media` header.
    ///
    /// `MediaStatus::Unlinked` is the correct value for a transcript that
    /// names its media but has no timing bullets yet (pre-forced-alignment):
    /// omitting the status asserts the transcript IS time-linked, which fails
    /// validation (E544, MediaLinkageWithoutTiming) when no bullets are
    /// present. Forced alignment removes the status once it adds bullets.
    pub media_status: Option<MediaStatus>,
    /// Utterances in document order.
    pub utterances: Vec<UtteranceDesc>,
}

/// A participant in the transcript.
#[derive(Debug, Clone)]
pub struct ParticipantDesc {
    /// Speaker code (e.g. `"CHI"`, `"INV"`).
    pub id: String,
    /// Participant name for `@Participants`. `None` omits the name field
    /// (output `CODE Role`); `Some(..)` adds it (output `CODE Name Role`).
    pub name: Option<String>,
    /// Participant role (e.g. `"Target_Child"`, `"Investigator"`).
    pub role: String,
    /// Corpus name for `@ID`. An empty string falls back to a placeholder.
    pub corpus: String,
}

/// A single utterance, given as pre-formatted CHAT main-tier text.
#[derive(Debug, Clone)]
pub struct UtteranceDesc {
    /// Speaker code for this utterance's main tier.
    pub speaker: String,
    /// The CHAT main-tier text, parsed via tree-sitter into a validated
    /// utterance.
    pub text: String,
    /// Optional utterance-level start time in ms (emits an inline bullet).
    pub start_ms: Option<u64>,
    /// Optional utterance-level end time in ms.
    pub end_ms: Option<u64>,
    /// Optional per-utterance language (ISO 639-3). When set and different
    /// from the primary language, a `[- lang]` precode is prepended.
    pub lang: Option<String>,
}
