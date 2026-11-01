//! String newtype wrappers for CHAT header payload fields.
//!
//! Each wrapper maps one header payload slot to a dedicated model type.
//! Per-type docs include a direct CHAT manual anchor for that header or field.
//!
//! Using distinct newtypes keeps header assembly strongly typed and prevents
//! accidental field swaps (for example, passing a `@PID` value where a
//! `@Situation` description is expected).
//! These wrappers intentionally perform no semantic normalization so parser
//! roundtrips can preserve corpus-authored header text exactly.
//!
//! Two families live here. A header payload whose whole meaning is its text
//! (`@PID`, `@Situation`, a gem label, ...) is a `non_empty_string_newtype!`:
//! it can never be empty or only whitespace, because a header with nothing
//! after its colon is `Header::EmptyContent`, decided once by the parser
//! (`HeaderContent::classify`) before any payload is built, and the payload is
//! built only from that decision's proof (`HeaderText`). The `@ID` field types stay `string_newtype!`: an `@ID` field may be
//! legitimately blank (`eng|corpus|CHI|||||Target_Child|||`), and the `@ID`
//! header keeps its own admission state (`Header::UnparsedID`).

use crate::{non_empty_string_newtype, string_newtype};

non_empty_string_newtype!(
    /// Persistent identifier recorded in `@PID`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#PID_Header>
    pub struct PidValue;
);

non_empty_string_newtype!(
    /// Description attached to `@Situation`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Situation_Header>
    pub struct SituationDescription;
);

non_empty_string_newtype!(
    /// Location text recorded in `@Tape Location`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Tape_Location_Header>
    pub struct TapeLocationDescription;
);

non_empty_string_newtype!(
    /// Location description from `@Location`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Location_Header>
    pub struct LocationDescription;
);

non_empty_string_newtype!(
    /// Room layout description from `@Room Layout`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Room_Layout_Header>
    pub struct RoomLayoutDescription;
);

non_empty_string_newtype!(
    /// Label for gem headers (`@Bg`, `@Eg`, `@G`).
    ///
    /// References:
    /// - <https://talkbank.org/0info/manuals/CHAT.html#Bg_Header>
    /// - <https://talkbank.org/0info/manuals/CHAT.html#Eg_Header>
    /// - <https://talkbank.org/0info/manuals/CHAT.html#G_Header>
    pub struct GemLabel;
);

impl GemLabel {
    /// Builds a gem label from a gem header's content: the one label joiner
    /// both parsers use.
    ///
    /// A gem label may continue onto further lines; each continuation (line
    /// breaks, the tab that opens the next line, and incidental spaces after
    /// it, `[\r\n]+\t[ ]*` in both grammars) separates label words and
    /// becomes one space. A continuation before any label text adds nothing.
    /// Every other character is kept exactly as written. The label is never
    /// empty: the content holds a character that is not whitespace, and the
    /// join keeps every such character, so no check is repeated here.
    pub fn from_label_content(text: crate::model::HeaderText<'_>) -> Self {
        let text = text.as_str();
        if !text.contains(['\r', '\n']) {
            return Self(::smol_str::SmolStr::from(text));
        }
        let mut label = String::with_capacity(text.len());
        // Line breaks only occur inside continuations, so splitting on them
        // leaves the first line, then one piece per later line that begins
        // with the continuation's tab. A run of line breaks splits into empty
        // pieces between them, which are not lines.
        let mut lines = text.split(['\r', '\n']);
        let mut saw_text = false;
        if let Some(first) = lines.next() {
            label.push_str(first);
            saw_text = !first.is_empty();
        }
        for line in lines.filter(|line| !line.is_empty()) {
            if saw_text {
                label.push(' ');
            }
            let words = line
                .strip_prefix('\t')
                .unwrap_or(line)
                .trim_start_matches(' ');
            label.push_str(words);
            saw_text |= !words.is_empty();
        }
        Self(::smol_str::SmolStr::from(label))
    }
}

non_empty_string_newtype!(
    /// Description of the birthplace in `@Birthplace of`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Birthplace_Header>
    pub struct BirthplaceDescription;
);

string_newtype!(
    /// Human-readable language name recorded in `@L1 of`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#L1_Header>
    pub struct LanguageName;
);

non_empty_string_newtype!(
    /// Transcriber name captured in `@Transcriber`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Transcriber_Header>
    pub struct TranscriberName;
);

non_empty_string_newtype!(
    /// Warning text from `@Warning`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Warning_Header>
    pub struct WarningText;
);

string_newtype!(
    /// Corpus name stored in `@ID` (field 2).
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Corpus_Field>
    pub struct CorpusName;
);

string_newtype!(
    /// Group identifier captured in `@ID` (field 6).
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Group_Field>
    pub struct GroupName;
);

// SesDescription was replaced by the typed SesValue enum in ses.rs.

string_newtype!(
    /// Education description from `@ID` (field 9).
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Education_Field>
    pub struct EducationDescription;
);

string_newtype!(
    /// Custom field text from `@ID` (field 10).
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Custom_Field>
    pub struct CustomIdField;
);

non_empty_string_newtype!(
    /// Activities list recorded in `@Activities`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Activities_Header>
    pub struct ActivitiesDescription;
);

non_empty_string_newtype!(
    /// Background context stored in `@Bck`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Bck_Header>
    pub struct BackgroundDescription;
);

non_empty_string_newtype!(
    /// Video references listed in `@Videos`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Videos_Header>
    pub struct VideoSpec;
);

non_empty_string_newtype!(
    /// Inline thumbnail marker text from `@T`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Thumbnail_Header>
    pub struct TDescription;
);

non_empty_string_newtype!(
    /// Font specification declared in `@Font`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Font_Header>
    pub struct FontSpec;
);

non_empty_string_newtype!(
    /// Window geometry captured in `@Window`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Window_Header>
    pub struct WindowGeometry;
);

non_empty_string_newtype!(
    /// Color word palette listed in `@Color words`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#ColorWords_Header>
    pub struct ColorWordList;
);

/// Media filename recorded in `@Media`: a local datafile basename, or a
/// remote URL wrapped in double quotes.
///
/// `@Media` separates the filename from the media type with a comma, so an
/// unquoted name may not contain one. [`parse`](Self::parse) is the only
/// constructor and enforces that.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
//
// Deliberately NOT a `string_newtype!`: unlike its siblings this one has an
// invariant, so there is no infallible `new`, `From<&str>` or `From<String>`
// beside `parse`. The rationale lives in this `//` comment rather than in the
// doc comment above because rustdoc on this type is published verbatim as the
// `description` of `MediaFilename` in `schema/chat-file.schema.json`, and a
// JSON consumer has no use for which Rust macro we did or did not invoke.
//
// Deserialization is deliberately LENIENT, matching `LanguageCode` and
// `RecoveryText` in policy: the wire reconstructs whatever the document
// held and the separate `Validate` pass reports the violation with a code and
// a span. A strict `try_from` here was tried and reverted; see
// `LanguageCode`'s `deserialize_empty_is_lenient`, which records the same
// thing being tried and reverted on that type by maintainer call 2026-07-04.
#[derive(
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    talkbank_derive::SemanticEq,
    talkbank_derive::SpanShift,
)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct MediaFilename(smol_str::SmolStr);

// The read/render surface every string newtype shares. Only the CONSTRUCTORS
// differ for a checked type, so only those are written out below.
crate::string_newtype_read_impls!(MediaFilename);

/// Why a string is not a usable `@Media` filename.
///
/// The offending value is carried once, on the struct, rather than repeated in
/// every variant: what went wrong and what it went wrong on are two separate
/// facts, and a caller that wants to log the bad string should not have to
/// match every reason to reach it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{reason} (value: {value:?})")]
pub struct MediaFilenameError {
    /// What is wrong with the value.
    pub reason: MediaFilenameProblem,
    /// The rejected string, as supplied.
    pub value: String,
}

/// The ways a string can fail to be a usable `@Media` filename.
///
/// Each names a value that could be STORED but could not be written to a
/// `@Media` line and read back unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MediaFilenameProblem {
    /// The filename was empty, so the header would name no file at all.
    #[error("@Media filename cannot be empty")]
    Empty,

    /// The filename began or ended with whitespace. `@Media` delimits the
    /// filename by the comma, so surrounding whitespace is not part of it and
    /// re-reading the header would report E767 rather than return this value.
    #[error("@Media filename cannot begin or end with whitespace")]
    SurroundingWhitespace,

    /// The filename contained a line break, which would split the header.
    #[error("@Media filename cannot contain a line break")]
    LineBreak,

    /// An unquoted filename contained the comma that separates the filename
    /// from the media type. A remote URL that genuinely contains a comma is
    /// legal in the quoted form.
    #[error(
        "unquoted @Media filename cannot contain a comma, which separates it \
         from the media type; quote it if this is a URL"
    )]
    UnquotedComma,

    /// A double quote appeared somewhere other than as the surrounding pair
    /// of the quoted URL form.
    #[error("@Media filename may only use double quotes as a surrounding pair")]
    StrayQuote,
}

impl MediaFilename {
    /// Whether the media token already has its standard Unicode spelling.
    /// Remote URLs are opaque and are never candidates for normalization.
    pub fn needs_unicode_normalization(&self) -> bool {
        !self.is_remote_url() && !unicode_normalization::is_nfc(self.as_str())
    }

    /// Parses a `@Media` filename, rejecting anything that could not be
    /// written to a `@Media` line and read back unchanged.
    ///
    /// Accepts either a bare local basename or a double-quoted remote URL;
    /// the quotes are part of the stored value, matching what the parser
    /// records and what [`is_remote_url`](Self::is_remote_url) tests.
    ///
    /// # Errors
    ///
    /// See [`MediaFilenameProblem`]. In short: not empty, no surrounding
    /// whitespace, no line break, no comma unless quoted, and double quotes
    /// only as the surrounding pair.
    ///
    /// ```rust
    /// # use talkbank_model::model::{MediaFilename, MediaFilenameProblem};
    /// // Dots, parentheses, interior spaces and non-ASCII are all fine.
    /// assert!(MediaFilename::parse("SD02_趙天恩_recording(1).final").is_ok());
    /// // A URL may contain commas, in the quoted form.
    /// assert!(MediaFilename::parse("\"https://example.org/a,b.mp3\"").is_ok());
    /// // A bare comma is the delimiter, so it cannot be part of the name.
    /// let err = MediaFilename::parse("take1,take2").expect_err("comma is the delimiter");
    /// assert_eq!(err.reason, MediaFilenameProblem::UnquotedComma);
    /// assert_eq!(err.value, "take1,take2");
    /// ```
    pub fn parse(value: impl AsRef<str>) -> Result<Self, MediaFilenameError> {
        let raw = value.as_ref();
        match Self::problem_with(raw) {
            // One allocation, on the error path only, rather than one per arm.
            Some(reason) => Err(MediaFilenameError {
                reason,
                value: raw.to_string(),
            }),
            None => Ok(Self(smol_str::SmolStr::from(raw))),
        }
    }

    /// The first reason `raw` cannot be a `@Media` filename, or `None` if it
    /// can.
    ///
    /// Allocation-free, so both [`parse`] and
    /// [`report_representability_issues`](Self::report_representability_issues)
    /// (which already holds a constructed value and must not allocate to ask)
    /// share one definition of the rule.
    ///
    /// [`parse`]: Self::parse
    fn problem_with(raw: &str) -> Option<MediaFilenameProblem> {
        if raw.is_empty() {
            return Some(MediaFilenameProblem::Empty);
        }
        if raw.trim().len() != raw.len() {
            return Some(MediaFilenameProblem::SurroundingWhitespace);
        }
        if raw.contains(['\n', '\r']) {
            return Some(MediaFilenameProblem::LineBreak);
        }

        let (quoted, interior) = quoted_interior(raw);
        if interior.contains('"') {
            return Some(MediaFilenameProblem::StrayQuote);
        }
        if !quoted && interior.contains(',') {
            return Some(MediaFilenameProblem::UnquotedComma);
        }
        None
    }

    /// Reports E768 if this filename could not be written to a `@Media` line
    /// and read back unchanged.
    ///
    /// The newtype owns the diagnostic, not just the predicate, so that a
    /// second caller (a JSON ingress check, merge, normalize) cannot pick a
    /// different code, severity or wording for the same fact. This mirrors
    /// [`LanguageCode::report_code_issues`], which was extracted for exactly
    /// that reason once it had two callers.
    ///
    /// A value that reached here through either parser always passes, since
    /// both end the filename at the comma; the caller that can fail is one
    /// holding a model deserialized from JSON.
    ///
    /// [`LanguageCode::report_code_issues`]: super::LanguageCode
    pub(crate) fn report_representability_issues(
        &self,
        location: crate::Location,
        errors: &impl crate::ErrorSink,
    ) {
        use crate::{DiagnosticLocation, ErrorCode, ErrorContext, ParseError, Severity};

        let Some(problem) = Self::problem_with(self.as_str()) else {
            return;
        };
        let text = self.as_str();
        errors.report(ParseError::new(
            ErrorCode::MediaFilenameNotRepresentable,
            Severity::Error,
            // Point at the start of the filename, when it has one.
            DiagnosticLocation::from(location.start_point()),
            ErrorContext::new(text, crate::Span::new(0, text.len()), "media_filename"),
            // The problem's own Display is already a full sentence naming the
            // subject, so it IS the message; prefixing it stutters.
            problem.to_string(),
        ));
    }

    /// The filename with the quoted form's surrounding quotes removed.
    ///
    /// For a bare local basename this is the whole value. Callers that need
    /// the FILE being named, rather than the header text, want this.
    pub fn unquoted(&self) -> &str {
        quoted_interior(self.as_str()).1
    }

    /// Returns true if this `@Media` filename is a remote URL (`http://` or
    /// `https://`) rather than a local datafile basename.
    ///
    /// A URL points at remote media, so the local-filename-match rule (CLAN
    /// CHECK 157, surfaced as E531) does not apply: CLAN itself accepts
    /// `@Media: "https://..."` with no error. The predicate lives on the
    /// newtype so callers (validation, and any future media resolution /
    /// forced alignment) do not re-derive it by string-hacking.
    pub fn is_remote_url(&self) -> bool {
        let unquoted = self.unquoted();
        unquoted.starts_with("http://") || unquoted.starts_with("https://")
    }
}

/// Splits the quoted URL form into "was it quoted" and the text inside.
///
/// One definition shared by `parse` and `is_remote_url`, which previously
/// derived it two different ways: `trim_matches('"')` strips any run of quotes
/// from each end independently, while the rule the format actually states is
/// exactly one matched surrounding pair. A lone `"` is not a pair, which
/// `strip_prefix` followed by `strip_suffix` handles without index arithmetic.
fn quoted_interior(raw: &str) -> (bool, &str) {
    match raw
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        Some(interior) => (true, interior),
        None => (false, raw),
    }
}
