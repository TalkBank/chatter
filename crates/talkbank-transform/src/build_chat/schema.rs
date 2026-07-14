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

use talkbank_model::model::{
    AgeValue, ChatDate, ChatOptionFlags, CustomIdField, EducationDescription, GroupName,
    LanguageName, MediaStatus, PidValue, SesValue, Sex, SituationDescription, TranscriberName,
};

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
    /// Optional `@PID` (persistent TalkBank handle). Emitted between `@UTF8`
    /// and `@Begin`. This is assigned at publish, not derivable from source:
    /// preserve an existing PID here, never mint one. `None` omits the header.
    pub pid: Option<PidValue>,
    /// Optional media linkage status for the `@Media` header.
    ///
    /// `MediaStatus::Unlinked` is the correct value for a transcript that
    /// names its media but has no timing bullets yet (pre-forced-alignment):
    /// omitting the status asserts the transcript IS time-linked, which fails
    /// validation (E544, MediaLinkageWithoutTiming) when no bullets are
    /// present. Forced alignment removes the status once it adds bullets.
    pub media_status: Option<MediaStatus>,
    /// Optional `@Date` (recording/transcript date). `None` omits the header.
    pub date: Option<ChatDate>,
    /// Optional `@Situation` (setting description). `None` omits the header.
    pub situation: Option<SituationDescription>,
    /// Optional `@Options` (CHAT processing flags, e.g. `CA`). `None` omits it.
    pub options: Option<ChatOptionFlags>,
    /// Optional `@Transcriber` (transcriber name(s)). `None` omits the header.
    pub transcriber: Option<TranscriberName>,
    /// Free-text `@Comment` header lines (e.g. speaker usage restrictions,
    /// preserved provenance). Emitted in order at the end of the header block.
    pub comments: Vec<String>,
    /// Utterances in document order.
    pub utterances: Vec<UtteranceDesc>,
}

/// A participant in the transcript.
///
/// `id`, `role`, and `corpus` map to the first, eighth, and second `@ID`
/// fields; the optional demographic fields carry the remaining `@ID` slots
/// (`language|corpus|code|age|sex|group|SES|role|education|custom`). They are
/// modeled with the same typed values as [`talkbank_model::model::IDHeader`]
/// (`AgeValue`, `Sex`, ...) rather than raw strings, so a generator cannot
/// silently emit a malformed demographic field or, as a prior bug did, drop
/// demographics entirely because the input schema had nowhere to put them.
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
    /// `@ID` field 4 (age). `None` leaves the field empty.
    pub age: Option<AgeValue>,
    /// `@ID` field 5 (sex). `None` leaves the field empty.
    pub sex: Option<Sex>,
    /// `@ID` field 6 (group). `None` leaves the field empty.
    pub group: Option<GroupName>,
    /// `@ID` field 7 (socioeconomic status). `None` leaves the field empty.
    pub ses: Option<SesValue>,
    /// `@ID` field 9 (education). `None` leaves the field empty.
    pub education: Option<EducationDescription>,
    /// `@ID` field 10 (corpus-specific custom extension). `None` leaves it empty.
    pub custom: Option<CustomIdField>,
    /// Optional first language, emitted as a per-participant `@L1 of SPK:`
    /// constant header (immediately after the `@ID` block). `None` omits it.
    pub l1_language: Option<LanguageName>,
}

impl ParticipantDesc {
    /// A participant with only the three required `@ID` fields set and every
    /// optional demographic field empty. Use the `with_*` setters to add
    /// demographics; this constructor keeps call sites from having to name
    /// every field, so adding a future field never silently defaults an
    /// existing caller's data.
    pub fn new(
        id: impl Into<String>,
        role: impl Into<String>,
        corpus: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: None,
            role: role.into(),
            corpus: corpus.into(),
            age: None,
            sex: None,
            group: None,
            ses: None,
            education: None,
            custom: None,
            l1_language: None,
        }
    }

    /// Sets the `@Participants` name field.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the `@ID` age field.
    pub fn with_age(mut self, age: AgeValue) -> Self {
        self.age = Some(age);
        self
    }

    /// Sets the `@ID` sex field.
    pub fn with_sex(mut self, sex: Sex) -> Self {
        self.sex = Some(sex);
        self
    }

    /// Sets the `@ID` group field.
    pub fn with_group(mut self, group: GroupName) -> Self {
        self.group = Some(group);
        self
    }

    /// Sets the `@ID` socioeconomic-status field.
    pub fn with_ses(mut self, ses: SesValue) -> Self {
        self.ses = Some(ses);
        self
    }

    /// Sets the `@ID` education field.
    pub fn with_education(mut self, education: EducationDescription) -> Self {
        self.education = Some(education);
        self
    }

    /// Sets the `@ID` custom-extension field.
    pub fn with_custom(mut self, custom: CustomIdField) -> Self {
        self.custom = Some(custom);
        self
    }

    /// Sets the participant's first language (`@L1 of`).
    pub fn with_l1_language(mut self, language: LanguageName) -> Self {
        self.l1_language = Some(language);
        self
    }
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
