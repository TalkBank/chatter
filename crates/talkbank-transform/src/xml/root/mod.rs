//! Document-level emission, the `<CHAT>` root, `<Participants>`,
//! body-level headers and comments, and utterance orchestration.
//!
//! This file owns the "top-down" traversal of a `ChatFile`. Word-level
//! emission delegates to `super::word`; morphology subtrees delegate
//! to `super::mor`. Metadata helpers (corpus lookup, date/age/sex
//! formatting, `@Options` flags, per-speaker extras from body-level
//! `@Birthplace` / `@L1` headers) also live here because they feed
//! attributes on `<CHAT>` and `<participant>`.

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, Event};
use std::collections::HashMap;

use talkbank_model::model::{
    AgeValue, BulletContent, ChatDate, ChatFile, ChatOptionFlag, Header, Line, Month, Sex,
    SpeakerCode,
};
use talkbank_model::validation::ValidationState;

use super::error::XmlWriteError;
use super::writer::{
    SCHEMA_LOCATION, SCHEMA_VERSION, TALKBANK_NS, XSI_NS, XmlEmitter, escape_text,
};

mod utterance;

impl XmlEmitter {
    /// Serialize the full document: XML decl, `<CHAT>` root with its
    /// attributes, `<Participants>`, and the body. This is the single
    /// entry point invoked by the public `write_chat_xml` wrapper.
    pub(super) fn emit_document<S: ValidationState>(
        &mut self,
        file: &ChatFile<S>,
    ) -> Result<(), XmlWriteError> {
        // <?xml version="1.0" encoding="UTF-8"?>
        self.writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

        let corpus = find_corpus(file)?;

        let mut root = BytesStart::new("CHAT");
        root.push_attribute(("xmlns:xsi", XSI_NS));
        root.push_attribute(("xmlns", TALKBANK_NS));
        root.push_attribute(("xsi:schemaLocation", SCHEMA_LOCATION));
        // `@Media` comes before `Version` in the TalkBank XML format on
        // audio/video files. The structural comparator ignores order,
        // but we keep the TalkBank XML ordering to minimize diff noise during
        // development.
        if let Some(media) = file.media.as_deref() {
            // CHAT `@Media:` values can be bare filenames or
            // double-quoted URLs (`"https://…"`). The XSD
            // `mediaRefType` is `xs:anyURI`, which rejects the
            // embedded quotes, strip them at the emission
            // boundary so the attribute is schema-legal.
            let raw = media.filename.as_str();
            let stripped = raw
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(raw);
            root.push_attribute(("Media", stripped));
            // `Mediatypes` is a space-separated list per the XSD's
            // `mediaTypesType` enumeration
            // ({audio|video|unlinked|missing|notrans}), so emit the
            // media type and any status in the same attribute rather
            // than splitting them.
            let mediatypes = match &media.status {
                Some(status) => format!("{} {}", media.media_type.as_str(), status.as_str()),
                None => media.media_type.as_str().to_owned(),
            };
            root.push_attribute(("Mediatypes", mediatypes.as_str()));
        }
        root.push_attribute(("Version", SCHEMA_VERSION));
        root.push_attribute(("Lang", join_language_codes(file).as_str()));
        if let Some(options) = format_options_attribute(file) {
            root.push_attribute(("Options", options.as_str()));
        }
        root.push_attribute(("Corpus", corpus.as_str()));
        if let Some(pid) = find_root_pid(file) {
            root.push_attribute(("PID", pid));
        }
        if let Some(date) = find_root_date(file)? {
            root.push_attribute(("Date", date.as_str()));
        }
        // `@Types: design, activity, group` projects onto three root
        // attributes in addition to the body-level `<comment
        // type="Types">` (which is emitted during the body walk).
        // The TalkBank XML shape names these
        // `DesignType` / `ActivityType` / `GroupType`.
        if let Some(types) = find_root_types(file) {
            root.push_attribute(("DesignType", types.design.as_str()));
            root.push_attribute(("ActivityType", types.activity.as_str()));
            root.push_attribute(("GroupType", types.group.as_str()));
        }
        // Pre-Begin UI-state headers, `@Color words`, `@Font`,
        // `@Window`, round-trip as root attributes. Documented in
        // the CHAT manual even though their origin is CLAN editor
        // state. Collected in a single pass so we don't scan
        // `file.lines` three times.
        let ui = find_root_ui_attrs(file);
        if let Some(colors) = ui.colors {
            root.push_attribute(("Colorwords", colors));
        }
        if let Some(font) = ui.font {
            root.push_attribute(("Font", font));
        }
        if let Some(window) = ui.window {
            root.push_attribute(("Window", window));
        }
        self.writer.write_event(Event::Start(root))?;

        self.emit_participants(file)?;
        self.emit_body(file)?;

        self.writer.write_event(Event::End(BytesEnd::new("CHAT")))?;
        Ok(())
    }

    /// Emit the `<Participants>` block. Pulls speaker metadata from
    /// the `@Participants` / `@ID` headers plus body-level
    /// `@Birthplace` / `@L1` extras collected via
    /// [`collect_per_speaker_metadata`].
    fn emit_participants<S: ValidationState>(
        &mut self,
        file: &ChatFile<S>,
    ) -> Result<(), XmlWriteError> {
        // Pre-scan body headers that attach extra metadata to an existing
        // participant (`@Birthplace of X`, `@L1 of X`). These live outside
        // Participant itself because they are independent CHAT headers.
        let extra = collect_per_speaker_metadata(file);

        self.writer
            .write_event(Event::Start(BytesStart::new("Participants")))?;

        for participant in file.participants.values() {
            // Attribute order below matches the TalkBank XML format to make
            // manual diffing against the golden easier; the structural
            // comparator treats order as insignificant.
            let mut start = BytesStart::new("participant");
            start.push_attribute(("id", participant.code.as_str()));
            start.push_attribute(("role", participant.role.as_ref()));

            let lang = join_codes_with_space(&participant.id.language.0);
            start.push_attribute(("language", lang.as_str()));

            if let Some(age) = &participant.id.age {
                let iso = format_age_iso8601(age)?;
                start.push_attribute(("age", iso.as_str()));
            }
            if let Some(sex) = &participant.id.sex {
                start.push_attribute(("sex", sex_to_xml(sex)?));
            }
            if let Some(group) = &participant.id.group {
                start.push_attribute(("group", group.as_str()));
            }
            if let Some(name) = &participant.name {
                start.push_attribute(("name", name.as_str()));
            }
            // `SES` attribute (uppercase, per the reference XML golden). `SesValue::as_str`
            // serializes `SesOnly(UC)` as `"UC"` and `Combined { eth, ses }`
            // as `"White,MC"`, matching the TalkBank XML comma-joined form.
            let ses_rendered;
            if let Some(ses) = &participant.id.ses {
                ses_rendered = ses.as_str();
                start.push_attribute(("SES", ses_rendered.as_str()));
            }
            if let Some(education) = &participant.id.education {
                start.push_attribute(("education", education.as_str()));
            }
            if let Some(birth_date) = &participant.birth_date {
                let iso = format_chat_date_iso(birth_date)?;
                start.push_attribute(("birthday", iso.as_str()));
            }
            if let Some(meta) = extra.get(&participant.code) {
                if let Some(place) = &meta.birthplace {
                    start.push_attribute(("birthplace", place.as_str()));
                }
                if let Some(lang1) = &meta.first_language {
                    start.push_attribute(("first-language", lang1.as_str()));
                }
            }
            if let Some(custom) = &participant.id.custom_field {
                start.push_attribute(("custom-field", custom.as_str()));
            }

            self.writer.write_event(Event::Empty(start))?;
        }

        self.writer
            .write_event(Event::End(BytesEnd::new("Participants")))?;
        Ok(())
    }

    /// Walk `file.lines` in order, dispatching each line to either a
    /// body-level header emitter or an utterance emitter. Root-level
    /// headers (`@Begin`, `@Languages`, `@ID`, …) that have already
    /// contributed attributes on `<CHAT>` are filtered out inside
    /// [`Self::emit_header_if_body`].
    fn emit_body<S: ValidationState>(&mut self, file: &ChatFile<S>) -> Result<(), XmlWriteError> {
        for line in file.lines.iter() {
            match line {
                Line::Header { header, .. } => self.emit_header_if_body(header)?,
                Line::Utterance(utterance) => self.emit_utterance(utterance)?,
            }
        }
        Ok(())
    }

    /// Most headers contribute to the root element or the Participants
    /// block and have already been consumed by `emit_document`; only
    /// body-level headers emit their own XML element. Stage 1 handles
    /// `@Comment`; all other body-level headers (`@Bg`/`@Eg`, `@G`,
    /// `@Media`, `@Situation`, `@Date`, `@Pid`, `@Types`, pre-begin
    /// headers, warnings, etc.) report `FeatureNotImplemented`.
    fn emit_header_if_body(&mut self, header: &Header) -> Result<(), XmlWriteError> {
        match header {
            // Scaffold + root-attribute + per-speaker metadata headers
            // already consumed by `emit_document` / `emit_participants`.
            Header::Utf8
            | Header::Begin
            | Header::End
            | Header::Languages { .. }
            | Header::Participants { .. }
            | Header::ID(_)
            | Header::Birth { .. }
            | Header::Birthplace { .. }
            | Header::L1Of { .. }
            | Header::Options { .. }
            | Header::Media(_)
            | Header::Pid { .. } => Ok(()),

            // @Bg/@Eg/@G gems render as standalone XML elements at
            // the same level as `<u>`, not as `<comment>` children.
            // The `label` attribute is required in the XSD; when CHAT
            // source omits it, emit an empty string to stay
            // schema-valid.
            Header::BeginGem { label } => self.emit_gem("begin-gem", gem_label(label)),
            Header::EndGem { label } => self.emit_gem("end-gem", gem_label(label)),
            Header::LazyGem { label } => self.emit_gem("lazy-gem", gem_label(label)),

            // @Date appears twice: once as a root `Date="YYYY-MM-DD"`
            // attribute and once as a `<comment type="Date">DD-MMM-
            // YYYY</comment>` preserving the original CHAT text.
            Header::Date { date } => self.emit_typed_comment("Date", date.as_str()),

            Header::Comment { content } => self.emit_bullet_content_comment("Generic", content),
            Header::Location { location } => self.emit_typed_comment("Location", location.as_str()),
            Header::Situation { text } => self.emit_typed_comment("Situation", text.as_str()),
            Header::Activities { activities } => {
                self.emit_typed_comment("Activities", activities.as_str())
            }
            Header::Transcriber { transcriber } => {
                self.emit_typed_comment("Transcriber", transcriber.as_str())
            }
            Header::Transcription { transcription } => {
                self.emit_typed_comment("Transcription", transcription.as_str())
            }
            Header::Warning { text } => self.emit_typed_comment("Warning", text.as_str()),
            Header::Bck { bck } => self.emit_typed_comment("Bck", bck.as_str()),
            Header::Number { number } => self.emit_typed_comment("Number", number.as_str()),
            Header::RecordingQuality { quality } => {
                self.emit_typed_comment("Recording Quality", quality.as_str())
            }
            Header::TapeLocation { location } => {
                self.emit_typed_comment("Tape Location", location.as_str())
            }
            Header::TimeDuration { duration } => {
                self.emit_typed_comment("Time Duration", duration.as_str())
            }
            Header::TimeStart { start } => self.emit_typed_comment("Time Start", start.as_str()),
            Header::RoomLayout { layout } => {
                self.emit_typed_comment("Room Layout", layout.as_str())
            }
            Header::Page { page } => self.emit_typed_comment("Page", page.as_str()),
            Header::T { text } => self.emit_typed_comment("T", text.as_str()),

            // `@Types: design, activity, group` → `<comment
            // type="Types">design, activity, group</comment>`. The three
            // fields are always emitted, comma-space separated.
            Header::Types(types) => {
                let payload = format!(
                    "{}, {}, {}",
                    types.design.as_str(),
                    types.activity.as_str(),
                    types.group.as_str()
                );
                self.emit_typed_comment("Types", &payload)
            }

            // `@Font`, `@Window`, `@Color words` project onto the
            // root `<CHAT>` element as attributes (see
            // `find_root_font` / `find_root_window` /
            // `find_root_color_words`) rather than as body-level
            // comments. Suppress here to avoid emitting them twice.
            Header::Font { .. } => Ok(()),
            Header::Window { .. } => Ok(()),
            Header::ColorWords { .. } => Ok(()),
            // `@Videos:` has no corresponding XML element in the
            // TalkBank XSD and the TalkBank XML format silently drops it.
            // Matching that: preserve the CHAT source, emit nothing.
            Header::Videos { .. } => Ok(()),
            // Marker headers with no payload. The XSD has dedicated
            // `"New Episode"` and `"Blank"` `commentTypeType` values.
            Header::NewEpisode => self.emit_typed_comment("New Episode", ""),
            Header::Blank => self.emit_typed_comment("Blank", ""),

            // Lenient-parse fallback. Preserve the original text in a
            // generic comment so the utterance stays well-formed; the
            // diagnostic `parse_reason` / `suggested_fix` fields are
            // validator metadata, not content, and don't project to XML.
            Header::Unknown { text, .. } => self.emit_typed_comment("Generic", text.as_str()),
        }
    }

    /// Emit `<begin-gem>` / `<end-gem>` / `<lazy-gem>` with the
    /// `label` attribute. TalkBank XML emits them as standalone empty
    /// elements alongside `<u>`, not inside `<comment>`.
    fn emit_gem(&mut self, element: &'static str, label: &str) -> Result<(), XmlWriteError> {
        let mut tag = BytesStart::new(element);
        tag.push_attribute(("label", label));
        self.writer.write_event(Event::Empty(tag))?;
        Ok(())
    }
}

/// Resolve an optional [`GemLabel`] to the `label="…"` attribute
/// value for `<begin-gem>` / `<end-gem>` / `<lazy-gem>`. The XSD
/// marks `label` as required, so when CHAT source omits it we emit
/// an empty string, schema-legal and round-trippable.
fn gem_label(label: &Option<talkbank_model::model::GemLabel>) -> &str {
    label.as_ref().map(|l| l.as_str()).unwrap_or("")
}

// Re-open the `impl XmlEmitter` block for the remaining methods.
impl XmlEmitter {
    /// Emit `<comment type="X">text</comment>`. Shared by `@Comment`
    /// and the typed metadata headers that project onto comments in
    /// the XML schema.
    fn emit_typed_comment(&mut self, type_value: &str, text: &str) -> Result<(), XmlWriteError> {
        let mut start = BytesStart::new("comment");
        start.push_attribute(("type", type_value));
        self.writer.write_event(Event::Start(start))?;
        let normalized = collapse_whitespace(text);
        self.writer
            .write_event(Event::Text(escape_text(&normalized)))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("comment")))?;
        Ok(())
    }

    /// Emit a `<comment type=…>` element with mixed content: text
    /// segments as `<Text>` children, timing bullets as sibling
    /// `<media start=… end=… unit="s"/>` elements, and picture
    /// references preserved as inline `%pic:"…"` text (there's no
    /// structural XML element for those).
    ///
    /// Matches the TalkBank XML `@Comment` shape, previously
    /// our emitter flattened everything to `[start_end]` text inside
    /// the comment, which lost the structural timing.
    fn emit_bullet_content_comment(
        &mut self,
        type_value: &str,
        content: &BulletContent,
    ) -> Result<(), XmlWriteError> {
        let mut start = BytesStart::new("comment");
        start.push_attribute(("type", type_value));
        self.writer.write_event(Event::Start(start))?;
        self.emit_bullet_content_children(content)?;
        self.writer
            .write_event(Event::End(BytesEnd::new("comment")))?;
        Ok(())
    }

    /// Walk a `BulletContent` and emit its segments as XML children of
    /// the currently-open element: text segments as `Text` events,
    /// timing bullets as `<media start=… end=… unit="s"/>` empty
    /// elements, picture references as inline `%pic:"…"` text.
    ///
    /// Shared between `<comment>` (header `@Comment`) and `<a>`
    /// (dependent-tier side tiers like `%cod`, `%act`, `%com`, etc.)
    ///, both take BulletContent and both produce mixed content in
    /// the TalkBank XML output.
    pub(super) fn emit_bullet_content_children(
        &mut self,
        content: &BulletContent,
    ) -> Result<(), XmlWriteError> {
        use talkbank_model::model::BulletContentSegment;

        let mut text_buf = String::new();
        for segment in content.segments.0.iter() {
            match segment {
                BulletContentSegment::Text(text) => {
                    text_buf.push_str(&text.text);
                }
                BulletContentSegment::Bullet(bullet) => {
                    if !text_buf.is_empty() {
                        let normalized = collapse_whitespace(&text_buf);
                        self.writer
                            .write_event(Event::Text(escape_text(&normalized)))?;
                        text_buf.clear();
                    }
                    let start_s = super::wor::format_seconds(bullet.start_ms);
                    let end_s = super::wor::format_seconds(bullet.end_ms);
                    let mut media = BytesStart::new("media");
                    media.push_attribute(("start", start_s.as_str()));
                    media.push_attribute(("end", end_s.as_str()));
                    media.push_attribute(("unit", "s"));
                    self.writer.write_event(Event::Empty(media))?;
                }
                BulletContentSegment::Picture(picture) => {
                    // `%pic:"filename"` → `<mediaPic href="filename"/>`
                    // per the TalkBank XML schema. Flush any buffered text so
                    // the `<mediaPic>` lands in document order.
                    if !text_buf.is_empty() {
                        let normalized = collapse_whitespace(&text_buf);
                        self.writer
                            .write_event(Event::Text(escape_text(&normalized)))?;
                        text_buf.clear();
                    }
                    let mut tag = BytesStart::new("mediaPic");
                    tag.push_attribute(("href", picture.filename.as_str()));
                    self.writer.write_event(Event::Empty(tag))?;
                }
                BulletContentSegment::Continuation => {
                    // Tab-indented continuation, collapse_whitespace
                    // will flatten this when we flush.
                    text_buf.push(' ');
                }
            }
        }

        if !text_buf.is_empty() {
            let normalized = collapse_whitespace(&text_buf);
            self.writer
                .write_event(Event::Text(escape_text(&normalized)))?;
        }
        Ok(())
    }
}

/// Extracts the first `@ID` header's corpus field and returns it as a
/// string. Every reference-corpus file carries a populated corpus slot;
/// absence here indicates malformed input reaching the emitter.
fn find_corpus<S: ValidationState>(file: &ChatFile<S>) -> Result<String, XmlWriteError> {
    for line in file.lines.iter() {
        if let Line::Header { header, .. } = line
            && let Header::ID(id) = header.as_ref()
            && let Some(corpus) = &id.corpus
        {
            return Ok(corpus.as_ref().to_owned());
        }
    }
    Err(XmlWriteError::MissingMetadata {
        what: "Corpus attribute (no @ID header with a corpus field)".to_owned(),
    })
}

/// Space-joined language codes for the root `Lang` attribute, matching
/// the TalkBank XML format (`"eng ara"`, not `"eng, ara"`).
fn join_language_codes<S: ValidationState>(file: &ChatFile<S>) -> String {
    join_codes_with_space(&file.languages.0)
}

fn join_codes_with_space(codes: &[talkbank_model::model::LanguageCode]) -> String {
    let mut out = String::new();
    for (i, code) in codes.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(code.as_str());
    }
    out
}

/// Normalize runs of whitespace in header / comment text. CHAT source
/// formatting, double spaces for visual alignment, tab-indented
/// continuation lines, carries no semantic content once the header
/// has been parsed; the TalkBank XML format collapses those to
/// single spaces (`xs:token`-style), and we match.
fn collapse_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for tok in input.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(tok);
    }
    out
}

/// Locate the `@Types` header (if any). Returns a borrowed
/// reference so `emit_document` can push the `DesignType` /
/// `ActivityType` / `GroupType` attributes onto the `<CHAT>` root
/// without owning the underlying `TypesHeader`.
fn find_root_types<S: ValidationState>(
    file: &ChatFile<S>,
) -> Option<&talkbank_model::model::TypesHeader> {
    for line in file.lines.iter() {
        if let Line::Header { header, .. } = line
            && let Header::Types(types) = header.as_ref()
        {
            return Some(types);
        }
    }
    None
}

/// Locate the `@PID` header (if any). Returns a borrowed `&str` view
/// of the value for direct push onto the root element.
fn find_root_pid<S: ValidationState>(file: &ChatFile<S>) -> Option<&str> {
    for line in file.lines.iter() {
        if let Line::Header { header, .. } = line
            && let Header::Pid { pid } = header.as_ref()
        {
            return Some(pid.as_str());
        }
    }
    None
}

/// Pre-Begin UI-state header values destined for `<CHAT>` root
/// attributes. Populated by a single pass over `file.lines` so the
/// emitter doesn't re-scan the header block once per attribute.
#[derive(Default)]
struct RootUiAttrs<'a> {
    colors: Option<&'a str>,
    font: Option<&'a str>,
    window: Option<&'a str>,
}

fn find_root_ui_attrs<S: ValidationState>(file: &ChatFile<S>) -> RootUiAttrs<'_> {
    let mut out = RootUiAttrs::default();
    for line in file.lines.iter() {
        let Line::Header { header, .. } = line else {
            continue;
        };
        match header.as_ref() {
            Header::Window { geometry } if out.window.is_none() => {
                out.window = Some(geometry.as_str());
            }
            Header::Font { font } if out.font.is_none() => {
                out.font = Some(font.as_str());
            }
            Header::ColorWords { colors } if out.colors.is_none() => {
                out.colors = Some(colors.as_str());
            }
            _ => {}
        }
    }
    out
}

/// Find the `@Date` header (if any) and format it for the root
/// `Date="YYYY-MM-DD"` attribute. Returns `Ok(None)` when the file has
/// no `@Date`; returns an error if the date is present but unparseable.
fn find_root_date<S: ValidationState>(file: &ChatFile<S>) -> Result<Option<String>, XmlWriteError> {
    for line in file.lines.iter() {
        if let Line::Header { header, .. } = line
            && let Header::Date { date } = header.as_ref()
        {
            return Ok(Some(format_chat_date_iso(date)?));
        }
    }
    Ok(None)
}

/// Space-joined `@Options` flags for the root `Options` attribute.
/// Returns `None` when the file declares no options (so the attribute
/// is simply omitted, matching the TalkBank XML format).
fn format_options_attribute<S: ValidationState>(file: &ChatFile<S>) -> Option<String> {
    if file.options.is_empty() {
        return None;
    }
    let mut out = String::new();
    for (i, flag) in file.options.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(ChatOptionFlag::as_str(flag));
    }
    Some(out)
}

/// Per-speaker metadata that lives outside `Participant` itself but
/// gets hoisted onto the `<participant>` element in XML.
#[derive(Default)]
struct SpeakerExtras {
    birthplace: Option<String>,
    first_language: Option<String>,
}

/// Scan all body-level headers for `@Birthplace of X` / `@L1 of X`
/// entries, keyed by the participant's `SpeakerCode`. The
/// `emit_participants` pass then hoists these onto the
/// `<participant>` element whose `id` matches.
fn collect_per_speaker_metadata<S: ValidationState>(
    file: &ChatFile<S>,
) -> HashMap<SpeakerCode, SpeakerExtras> {
    let mut out: HashMap<SpeakerCode, SpeakerExtras> = HashMap::new();
    for line in file.lines.iter() {
        let Line::Header { header, .. } = line else {
            continue;
        };
        match header.as_ref() {
            Header::Birthplace { participant, place } => {
                out.entry(participant.clone()).or_default().birthplace =
                    Some(place.as_str().to_owned());
            }
            Header::L1Of {
                participant,
                language,
            } => {
                out.entry(participant.clone()).or_default().first_language =
                    Some(language.as_str().to_owned());
            }
            _ => {}
        }
    }
    out
}

/// `ChatDate` → `YYYY-MM-DD`. Rejects unsupported dates up front so the
/// emitter never writes a malformed attribute.
fn format_chat_date_iso(date: &ChatDate) -> Result<String, XmlWriteError> {
    match date {
        ChatDate::Valid {
            day, month, year, ..
        } => Ok(format!(
            "{year:04}-{month:02}-{day:02}",
            year = year,
            month = month_to_number(month),
            day = day
        )),
        ChatDate::Unsupported(raw) => Err(XmlWriteError::MissingMetadata {
            what: format!("unparseable @Birth/@Date value: {raw}"),
        }),
    }
}

fn month_to_number(month: &Month) -> u8 {
    match month {
        Month::Jan => 1,
        Month::Feb => 2,
        Month::Mar => 3,
        Month::Apr => 4,
        Month::May => 5,
        Month::Jun => 6,
        Month::Jul => 7,
        Month::Aug => 8,
        Month::Sep => 9,
        Month::Oct => 10,
        Month::Nov => 11,
        Month::Dec => 12,
    }
}

/// `AgeValue` → ISO 8601 duration. Examples:
/// - `1;08.02` → `P1Y08M02D`
/// - `43;`    → `P43Y`
/// - `2;06`   → `P2Y06M`
///
/// Months and days are zero-padded to two digits to match the TalkBank XML
/// format; years are unpadded.
fn format_age_iso8601(age: &AgeValue) -> Result<String, XmlWriteError> {
    match age {
        AgeValue::Valid {
            years,
            months,
            days,
            ..
        } => {
            let mut out = format!("P{years}Y");
            if let Some(m) = months {
                out.push_str(&format!("{m:02}M"));
            }
            if let Some(d) = days {
                out.push_str(&format!("{d:02}D"));
            }
            Ok(out)
        }
        AgeValue::Unsupported(raw) => Err(XmlWriteError::MissingMetadata {
            what: format!("unparseable @ID age value: {raw}"),
        }),
    }
}

/// Map [`Sex`] to its XML attribute value. Unsupported values escalate
/// rather than silently serializing the raw text, downstream consumers
/// expect exactly `male` or `female` in this slot.
fn sex_to_xml(sex: &Sex) -> Result<&'static str, XmlWriteError> {
    match sex {
        Sex::Male => Ok("male"),
        Sex::Female => Ok("female"),
        Sex::Unsupported(raw) => Err(XmlWriteError::MissingMetadata {
            what: format!("unsupported @ID sex value: {raw}"),
        }),
    }
}
