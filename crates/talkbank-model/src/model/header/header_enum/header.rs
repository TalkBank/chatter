//! Typed representation of CHAT header lines and header-list wrapper types.
//!
//! The parser stores transcript order at the file layer (`Line::Header`), while
//! this module captures the structured payload of each header for validation,
//! transformation, and serialization.
//!
//! CHAT reference anchors:
//! - [File headers](https://talkbank.org/0info/manuals/CHAT.html#File_Headers)
//! - [Participants header](https://talkbank.org/0info/manuals/CHAT.html#Participants_Header)
//! - [Languages header](https://talkbank.org/0info/manuals/CHAT.html#Languages_Header)
//! - [Options header](https://talkbank.org/0info/manuals/CHAT.html#Options_Header)

use super::super::{
    codes::{
        ActivitiesDescription, BackgroundDescription, BirthplaceDescription, ChatDate,
        ColorWordList, FontSpec, GemLabel, LanguageCode, LocationDescription, PageNumber,
        ParticipantEntry, PidValue, RoomLayoutDescription, SituationDescription, SpeakerCode,
        TDescription, TapeLocationDescription, TimeDurationValue, TimeStartValue, TranscriberName,
        VideoSpec, WarningText, WindowGeometry,
    },
    enums::{Number, RecordingQuality, Transcription},
    types_header::TypesHeader,
};
use super::empty::{HeaderContent, HeaderText};
use super::options::ChatOptionFlag;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use talkbank_derive::{SemanticEq, SpanShift};

/// Ordered payload of an `@Languages` header.
///
/// Order is semantically relevant: the first code is treated as the transcript
/// default when utterances do not override language explicitly.
///
/// # Reference
///
/// - [Languages header](https://talkbank.org/0info/manuals/CHAT.html#Languages_Header)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct LanguageCodes(Vec<LanguageCode>);

crate::collection_newtype_ops!(LanguageCodes, LanguageCode);

impl LanguageCodes {
    /// Appends every code of `other`, in order.
    ///
    /// A NAMED length-changing operation, added when `DerefMut` was removed
    /// from this family so a future invariant COULD be enforced here. None is
    /// today: `From<Vec<_>>` still bypasses it. See `collection_newtype_ops!`.
    pub fn extend(&mut self, other: impl IntoIterator<Item = LanguageCode>) {
        self.0.extend(other);
    }

    /// Wraps a language-code sequence, including an empty recovery payload.
    pub fn new(codes: Vec<LanguageCode>) -> Self {
        Self(codes)
    }

    /// Returns `true` when no language was declared.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Declares `to` wherever `from` was declared, reporting whether it moved.
    ///
    /// DEDUPLICATES rather than duplicating: retagging a code the header
    /// already declares removes the wrong entry instead of producing a second
    /// `to`. That is the common shape for a mislabelled code, because the right
    /// one is usually declared alongside it (`swe, fin, sun` -> `swe, fin`).
    ///
    /// A NAMED length-changing operation, for the reason [`Self::extend`] gives:
    /// `DerefMut` is deliberately absent from this family so an invariant on the
    /// code list COULD be enforced in one place. A caller reaching for
    /// `codes.remove(i)` would be routing around that.
    pub fn retag(&mut self, from: &LanguageCode, to: &LanguageCode) -> bool {
        // Identity is not deduplication: the existing target is the source
        // itself, so removing it would erase a declaration.
        if from == to {
            return false;
        }
        let Some(at) = self.0.iter().position(|code| code == from) else {
            return false;
        };
        if self.0.iter().any(|code| code == to) {
            self.0.remove(at);
        } else {
            self.0[at] = to.clone();
        }
        true
    }
}

impl crate::model::WriteChat for LanguageCodes {
    /// Writes comma-separated language codes: `eng, spa`.
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        for (i, code) in self.0.iter().enumerate() {
            if i > 0 {
                w.write_str(", ")?;
            }
            code.write_chat(w)?;
        }
        Ok(())
    }
}

impl Deref for LanguageCodes {
    type Target = Vec<LanguageCode>;

    /// Borrows the underlying ordered language-code list.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<LanguageCode>> for LanguageCodes {
    /// Wraps an owned vector without copying.
    fn from(codes: Vec<LanguageCode>) -> Self {
        Self(codes)
    }
}

impl<'a> IntoIterator for &'a LanguageCodes {
    type Item = &'a LanguageCode;
    type IntoIter = std::slice::Iter<'a, LanguageCode>;

    /// Iterates immutably over language codes in header order.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut LanguageCodes {
    type Item = &'a mut LanguageCode;
    type IntoIter = std::slice::IterMut<'a, LanguageCode>;

    /// Iterates mutably over language codes.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for LanguageCodes {
    type Item = LanguageCode;
    type IntoIter = std::vec::IntoIter<LanguageCode>;

    /// Consumes the wrapper and yields owned language codes.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Default for LanguageCodes {
    /// Returns an empty language-code payload, not evidence of an absent header.
    ///
    /// An enclosing `Header::Languages` remains present and validates as E507.
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl crate::validation::Validate for LanguageCodes {
    /// Enforces non-empty `@Languages` and validates each language code entry.
    fn validate(
        &self,
        context: &crate::validation::ValidationContext,
        errors: &impl crate::ErrorSink,
    ) {
        if self.is_empty() {
            errors.report(crate::ParseError::new(
                crate::ErrorCode::EmptyLanguagesHeader,
                crate::Severity::Error,
                context.field_diagnostic_location(),
                crate::ErrorContext::new("", crate::Span::new(0, 0), "languages"),
                "@Languages header should specify at least one language code",
            ));
        }

        for code in &self.0 {
            code.validate(context, errors);
        }
    }
}

/// Ordered payload of an `@Participants` header.
///
/// Order is preserved from source so tools can roundtrip the author's header
/// layout and keep participant listings stable in UIs.
///
/// # Reference
///
/// - [Participants header](https://talkbank.org/0info/manuals/CHAT.html#Participants_Header)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct ParticipantEntries(Vec<ParticipantEntry>);

crate::collection_newtype_ops!(ParticipantEntries, ParticipantEntry);

impl ParticipantEntries {
    /// Wraps parsed participant entries in source order.
    pub fn new(entries: Vec<ParticipantEntry>) -> Self {
        Self(entries)
    }

    /// Returns `true` when no participants were declared.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for ParticipantEntries {
    type Target = Vec<ParticipantEntry>;

    /// Borrows the underlying ordered participant-entry list.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<ParticipantEntry>> for ParticipantEntries {
    /// Wraps an owned vector without copying.
    fn from(entries: Vec<ParticipantEntry>) -> Self {
        Self(entries)
    }
}

impl<'a> IntoIterator for &'a ParticipantEntries {
    type Item = &'a ParticipantEntry;
    type IntoIter = std::slice::Iter<'a, ParticipantEntry>;

    /// Iterates immutably over participant entries.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut ParticipantEntries {
    type Item = &'a mut ParticipantEntry;
    type IntoIter = std::slice::IterMut<'a, ParticipantEntry>;

    /// Iterates mutably over participant entries.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for ParticipantEntries {
    type Item = ParticipantEntry;
    type IntoIter = std::vec::IntoIter<ParticipantEntry>;

    /// Consumes the wrapper and yields owned participant entries.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl crate::validation::Validate for ParticipantEntries {
    /// Enforces non-empty `@Participants` and validates each declared participant entry.
    fn validate(
        &self,
        context: &crate::validation::ValidationContext,
        errors: &impl crate::ErrorSink,
    ) {
        if self.is_empty() {
            errors.report(crate::ParseError::new(
                crate::ErrorCode::EmptyParticipantsHeader,
                crate::Severity::Error,
                context.field_diagnostic_location(),
                crate::ErrorContext::new("", crate::Span::new(0, 0), "participants"),
                "@Participants header cannot be empty",
            ));
        }

        for entry in &self.0 {
            entry.validate(context, errors);
        }
    }
}

/// Ordered payload of an `@Options` header: at least one flag.
///
/// Flags are retained in source order even though most consumers treat this as
/// a set, which keeps roundtrip output deterministic. The list is never empty:
/// an `@Options` header with nothing after its colon is
/// `Header::EmptyContent`, so every route in ([`Self::parse`], [`Self::new`],
/// `TryFrom`, JSON) refuses an empty list, and no length-shrinking operation
/// is offered.
///
/// # Reference
///
/// - [Options header](https://talkbank.org/0info/manuals/CHAT.html#Options_Header)
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct ChatOptionFlags(#[schemars(length(min = 1))] Vec<ChatOptionFlag>);

crate::collection_newtype_read_ops!(ChatOptionFlags, ChatOptionFlag);

/// Why an `@Options` list cannot be admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MalformedOptionList {
    /// The list names no option at all (an empty list from a constructor or
    /// JSON; a parsed header with no option is `Header::EmptyContent`).
    #[error("an @Options list must name at least one option")]
    NoOption,
    /// A comma-separated position holds no option name, as in `CA,,NoAlign`
    /// or a trailing comma.
    #[error("@Options option {position} is empty")]
    EmptyName {
        /// The 1-based position of the empty name in the list.
        position: usize,
    },
}

impl ChatOptionFlags {
    /// Reads an `@Options` content: comma-separated option names, each
    /// trimmed of surrounding whitespace. The one reader both parsers use.
    ///
    /// Built as a nonempty list from its first name, so no empty list is ever
    /// formed, and an empty name is refused rather than kept as an
    /// unsupported option named `""`.
    ///
    /// # Errors
    ///
    /// [`MalformedOptionList::EmptyName`] when a position between commas, or
    /// after a trailing comma, holds no name.
    pub fn parse(text: HeaderText<'_>) -> Result<Self, MalformedOptionList> {
        let mut names = text.as_str().split(',').enumerate();
        let admit = |(index, name): (usize, &str)| match HeaderContent::classify(name.trim()) {
            HeaderContent::Text(name) => Ok(ChatOptionFlag::from_text(name)),
            HeaderContent::Blank => Err(MalformedOptionList::EmptyName {
                position: index + 1,
            }),
        };
        let first = names.next().ok_or(MalformedOptionList::NoOption)?;
        let mut flags = vec![admit(first)?];
        for name in names {
            flags.push(admit(name)?);
        }
        Ok(Self(flags))
    }

    /// Wraps option flags in source order, refusing an empty list.
    ///
    /// # Errors
    ///
    /// [`MalformedOptionList::NoOption`] when `options` is empty.
    pub fn new(options: Vec<ChatOptionFlag>) -> Result<Self, MalformedOptionList> {
        if options.is_empty() {
            Err(MalformedOptionList::NoOption)
        } else {
            Ok(Self(options))
        }
    }

    /// Admits one known flag without a fallible collection check.
    pub fn from_flag(flag: ChatOptionFlag) -> Self {
        Self(vec![flag])
    }

    /// Appends one flag.
    ///
    /// `@Options` is a closed set with real rules about which flags may
    /// co-occur, so this is exactly the seam where such a rule would live.
    pub fn push(&mut self, flag: ChatOptionFlag) {
        self.0.push(flag);
    }
}

impl Deref for ChatOptionFlags {
    type Target = Vec<ChatOptionFlag>;

    /// Borrows the underlying ordered option-flag list.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<Vec<ChatOptionFlag>> for ChatOptionFlags {
    type Error = MalformedOptionList;

    /// Admits a raw list without bypassing the nonempty constraint.
    fn try_from(options: Vec<ChatOptionFlag>) -> Result<Self, Self::Error> {
        Self::new(options)
    }
}

impl<'de> Deserialize<'de> for ChatOptionFlags {
    /// Reads a list and admits it through `new`, refusing an empty one.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(Vec::<ChatOptionFlag>::deserialize(deserializer)?)
            .map_err(serde::de::Error::custom)
    }
}

impl<'a> IntoIterator for &'a ChatOptionFlags {
    type Item = &'a ChatOptionFlag;
    type IntoIter = std::slice::Iter<'a, ChatOptionFlag>;

    /// Iterates immutably over parser option flags.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut ChatOptionFlags {
    type Item = &'a mut ChatOptionFlag;
    type IntoIter = std::slice::IterMut<'a, ChatOptionFlag>;

    /// Iterates mutably over parser option flags.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for ChatOptionFlags {
    type Item = ChatOptionFlag;
    type IntoIter = std::vec::IntoIter<ChatOptionFlag>;

    /// Consumes the wrapper and yields owned option flags.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// Typed payload for a single CHAT header line.
///
/// Validation across *multiple* headers (required presence, ordering, cross-
/// header consistency) lives in `validation/header/**`; this enum models just
/// the parsed payload of one line.
///
/// Reference:
/// - [File Headers](https://talkbank.org/0info/manuals/CHAT.html#File_Headers)
/// - [Begin Header](https://talkbank.org/0info/manuals/CHAT.html#Begin_Header)
/// - [End Header](https://talkbank.org/0info/manuals/CHAT.html#End_Header)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Header {
    // =========================================================================
    // Required Structure Headers
    // =========================================================================
    /// @UTF8 - File encoding declaration (required first header)
    Utf8,

    /// @Begin - Transcript start marker (required)
    Begin,

    /// @End - Transcript end marker (required last)
    End,

    // =========================================================================
    // Participant Headers
    // =========================================================================
    /// @Languages:\tlang1, lang2, ...
    Languages {
        /// Ordered language codes (first is primary)
        codes: LanguageCodes,
    },

    /// Rejected @Languages source; never an admitted language declaration.
    UnparsedLanguages(super::super::UnparsedLanguageHeader),

    /// Rejected participant-header source; establishes no speaker identities.
    UnparsedParticipants(super::super::UnparsedParticipantHeader),

    /// Recognized ID header whose fields could not be admitted.
    /// Raw source remains invalid through serialization; no identity is inferred.
    #[serde(rename = "unparsed_id")]
    UnparsedID(super::super::UnparsedIDHeader),

    /// @Participants:\tSPK Name Role, SPK2 Name2 Role2, ...
    Participants {
        /// Participant entries (speaker code, optional name, role)
        entries: ParticipantEntries,
    },

    /// @ID:\tlang|corpus|speaker|age|sex|group|ses|role|education|custom|
    #[serde(rename = "id")]
    ID(super::super::IDHeader),

    /// A colon-bearing header with nothing after its colon.
    ///
    /// One state for every header kind that can be written this way (see
    /// [`EmptyHeader`](super::empty::EmptyHeader)), decided by the parser
    /// before any payload is built,
    /// so no payload type below has to represent "no content". Validation
    /// always refuses it (E553). `@Languages`, `@Participants` and `@ID` are
    /// not represented here: their malformed forms keep their own
    /// declaration-admission states above.
    EmptyContent(super::empty::EmptyHeader),

    // =========================================================================
    // Metadata Headers
    // =========================================================================
    /// @Date:\tDD-MMM-YYYY
    Date {
        /// Calendar date in DD-MMM-YYYY format
        date: ChatDate,
    },

    /// @Comment:\tfree text
    Comment {
        /// Comment content, which may include media bullets and pictures,
        /// proven to write something (never blank: blank is `EmptyContent`)
        content: super::comment::CommentContent,
    },

    /// @PID:\tpersistent-id
    Pid {
        /// Persistent identifier value
        pid: PidValue,
    },

    /// @Media:\tfilename, type
    #[serde(rename = "media")]
    Media(super::super::MediaHeader),

    /// @Situation:\tdescription
    Situation {
        /// Free-text situation description
        text: SituationDescription,
    },

    /// @Types:\tdesign, activity, group
    #[serde(rename = "types")]
    Types(TypesHeader),

    // =========================================================================
    // Gem Headers (Time-aligned Segments)
    // =========================================================================
    /// @Bg:\tlabel - Begin gem
    BeginGem {
        /// The gem label; `None` for the bare form `@Bg`, which has no colon.
        /// A colon followed by nothing is [`Header::EmptyContent`].
        label: Option<GemLabel>,
    },

    /// @Eg:\tlabel - End gem
    EndGem {
        /// The gem label; `None` for the bare form `@Eg`, which has no colon.
        /// A colon followed by nothing is [`Header::EmptyContent`].
        label: Option<GemLabel>,
    },

    /// @G:\tlabel - Lazy gem
    LazyGem {
        /// The gem label; `None` for the bare form `@G`, which has no colon.
        /// A colon followed by nothing is [`Header::EmptyContent`].
        label: Option<GemLabel>,
    },

    // =========================================================================
    // CLAN Display Headers (Pre-@Begin)
    // =========================================================================
    /// @Font:\tspecification
    Font {
        /// Font specification string
        font: FontSpec,
    },

    /// @Window:\tgeometry
    Window {
        /// Window geometry description
        geometry: WindowGeometry,
    },

    /// @Color words:\tcolors
    ColorWords {
        /// Color word palette
        colors: ColorWordList,
    },

    // =========================================================================
    // Recording/Session Headers
    // =========================================================================
    /// @Number:\t1-5|more|audience
    Number {
        /// Participant count
        number: Number,
    },

    /// @Recording Quality:\t1-5
    RecordingQuality {
        /// Quality rating (1-5)
        quality: RecordingQuality,
    },

    /// @Transcription:\ttype
    Transcription {
        /// Transcription quality type
        transcription: Transcription,
    },

    /// @New Episode
    NewEpisode,

    /// @Tape Location:\ttime
    TapeLocation {
        /// Tape location description
        location: TapeLocationDescription,
    },

    /// @Time Duration:\trange
    TimeDuration {
        /// Time duration value
        duration: TimeDurationValue,
    },

    /// @Time Start:\ttime
    TimeStart {
        /// Start time value
        start: TimeStartValue,
    },

    /// @Location:\tplace
    Location {
        /// Recording location description
        location: LocationDescription,
    },

    /// @Room Layout:\tdescription
    RoomLayout {
        /// Room layout description
        layout: RoomLayoutDescription,
    },

    // =========================================================================
    // Participant-Specific Headers
    // =========================================================================
    /// @Birth of SPK:\tDD-MMM-YYYY
    Birth {
        /// Speaker code of the participant
        participant: SpeakerCode,
        /// Birth date in DD-MMM-YYYY format. A birth header with no date is
        /// [`Header::EmptyContent`]; an unknown birth date is written by
        /// omitting the header.
        date: ChatDate,
    },

    /// @Birthplace of SPK:\tplace
    Birthplace {
        /// Speaker code of the participant
        participant: SpeakerCode,
        /// Birthplace description
        place: BirthplaceDescription,
    },

    /// @L1 of SPK:\tlanguage
    L1Of {
        /// Speaker code of the participant
        participant: SpeakerCode,
        /// First language as an ISO 639-3 code.
        ///
        /// Wild corpus usage is uniformly codes (verified 2026-07-16:
        /// 16 distinct values across 1,158 kept files, all three-letter
        /// registry codes), so the field is typed as [`LanguageCode`]
        /// and held to the same shared code rule set as `@Languages` /
        /// `@ID` (migrated from a free-text `LanguageName` per the
        /// 2026-07-15 @s-declaration ruling, part 2).
        ///
        /// JSON refuses blank text here, as a parser does (an `@L1 of` with
        /// nothing after its colon is `Header::EmptyContent`), while
        /// `LanguageCode` stays lenient everywhere else.
        #[serde(deserialize_with = "deserialize_l1_language")]
        language: LanguageCode,
    },

    // =========================================================================
    // Other Headers
    // =========================================================================
    /// @Blank - Blank line marker
    Blank,

    /// @Transcriber:\tname
    Transcriber {
        /// Transcriber name
        transcriber: TranscriberName,
    },

    /// @Warning:\twarning text
    Warning {
        /// Warning message text
        text: WarningText,
    },

    /// Parse-recovery fallback for unrecognized or malformed header lines.
    ///
    /// This variant should be eliminated by validation (reported as an error)
    /// before data is treated as a clean CHAT model.
    Unknown {
        /// Header text as captured from parser recovery, exactly as observed.
        text: crate::model::RecoveryText,
        /// Optional parser-side reason describing why decoding failed.
        parse_reason: Option<String>,
        /// Optional suggested correction (for example nearest known header).
        suggested_fix: Option<String>,
    },

    /// @Activities:\tlist
    Activities {
        /// Activities description list
        activities: ActivitiesDescription,
    },

    /// @Bck:\tbackground
    Bck {
        /// Background context description
        bck: BackgroundDescription,
    },

    /// @Options:\toptions (CA, CA-Unicode, bullets)
    Options {
        /// Option flags controlling parser behavior
        options: ChatOptionFlags,
    },

    /// @Page:\tnumber
    Page {
        /// Page number identifier
        page: PageNumber,
    },

    /// @Videos:\tfiles
    Videos {
        /// Video file specification
        videos: VideoSpec,
    },

    /// @T:\ttext (inline thumbnail marker)
    T {
        /// Thumbnail marker text
        text: TDescription,
    },
}

/// Admits an `@L1 of` language from JSON through the one emptiness decision,
/// so the wire cannot carry the blank value a parser would have made
/// `Header::EmptyContent`.
fn deserialize_l1_language<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<LanguageCode, D::Error> {
    let text = std::borrow::Cow::<'de, str>::deserialize(deserializer)?;
    HeaderContent::admit(&text)
        .map(|text| LanguageCode::from_non_empty(text.as_non_empty()))
        .map_err(serde::de::Error::custom)
}
