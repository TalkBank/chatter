// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Characterization tests for the header-dispatch layer (Task B2a of the
//! visitor-driven parser migration): `header_parser/dispatch/{mod,simple,
//! special,gem,structured}.rs`.
//!
//! These tests pin the OBSERVABLE behavior of LEVEL-1 header dispatch (which
//! `HeaderChoice` variant a concrete `header` CST node classifies to) and each
//! per-kind function's LEVEL-2 content read, at the real parser boundary
//! (`parse_chat_file_streaming` -> `ChatFile` + collected diagnostics). The
//! migration replaces `TypedTraversal.classify_header` / `TypedTraversal
//! .extract_<kind>_header` (the OLD `generated_traversal` trait receiver) with
//! the NEW backend's free `extract_header` / `extract_<kind>_header` functions,
//! but it is behavior-preserving: the model and the recovery diagnostics must
//! not change.
//!
//! The VALID fixtures are read verbatim from `corpus/reference/core/`
//! (already-passing, roundtrip-clean reference files), not hand-authored, so
//! they exercise real CHAT syntax with zero fixture-authoring risk. Together
//! `headers-episodes.cha` plus `headers-time-and-types.cha`,
//! `headers-media.cha`, and `headers-comments.cha` drive EVERY one of the
//! 25 dispatch call sites
//! except the `unsupported_header` catch-all and the `thumbnail_header` gap,
//! which have no "valid happy path" (they are inherently the
//! malformed/unmodeled cases), and are pinned separately below with a
//! hand-written fixture.
//!
//! Every diagnostic and model value pinned here was captured by RUNNING the
//! pre-migration (OLD-API) parser (`cargo nextest run -p talkbank-parser
//! --test header_dispatch_migration --no-capture`), not guessed. In
//! particular, running the malformed fixture below first showed that
//! `unsupported_header` (`simple::unsupported`) reports NO diagnostic (only
//! the model recovers to `Header::Unknown`), while the `thumbnail_header` gap
//! (`dispatch::thumbnail`) reports E525 AND rejects the line outright (it does
//! not even produce an `Unknown` fallback line): both were transcribed from
//! the actual run, not assumed from reading the source.

use std::path::{Path, PathBuf};

use talkbank_model::ErrorCollector;
use talkbank_model::model::Line;
use talkbank_parser::TreeSitterParser;

/// One diagnostic: (code, span start, span end, message).
type Diag = (String, u32, u32, String);

/// Read a fixture verbatim from `corpus/reference/<relative>`.
fn read_corpus_fixture(relative: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/reference")
        .join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read corpus fixture {}: {}", path.display(), e))
}

/// Render one line as a stable structural string: the full `{:?}` of the
/// `Header` payload for a header line (captures every field, so this pins the
/// COMPLETE model output, not just a tag), or `"Utterance"` for an utterance
/// line (utterance content is out of scope for this cluster).
fn line_repr(line: &Line) -> String {
    match line {
        Line::Header { header, .. } => format!("{header:?}"),
        Line::Utterance(_) => "Utterance".to_string(),
    }
}

fn line_reprs(lines: &[Line]) -> Vec<String> {
    lines.iter().map(line_repr).collect()
}

/// Parse `input` at the real streaming boundary and return the line
/// representations plus every collected diagnostic as `(code, start, end,
/// message)` tuples.
fn parse_lines_and_diags(input: &str) -> (Vec<String>, Vec<Diag>) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);
    let diags = errors
        .into_vec()
        .into_iter()
        .map(|d| {
            (
                d.code.as_str().to_string(),
                d.location.span.start,
                d.location.span.end,
                d.message,
            )
        })
        .collect();
    (line_reprs(&chat.lines.0), diags)
}

/// A stray inline header (`@Foo:`) that the grammar matches structurally as
/// `unsupported_header` (the catch-all for an unrecognized `@`-prefixed
/// header keyword), immediately followed by a valid `@Thumbnail:` header (the
/// ONE `header` subtype with no `Header` model variant: the `thumbnail`
/// gap-handler function). Both are syntactically VALID CHAT (the grammar
/// accepts them), but the Rust model has no representation for either. This
/// exercises the two dispatch arms the valid corpus fixtures cannot reach
/// (there is no "valid happy path" for a gap/catch-all).
const UNSUPPORTED_AND_THUMBNAIL: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Child
@ID:\teng|corpus|CHI|||||Child|||
@Foo:\tunexpected header text
@Thumbnail:\tsome thumbnail description
*CHI:\thello .
@End
";

#[test]
fn headers_episodes_fixture_parses_with_zero_diagnostics() {
    let input = read_corpus_fixture("core/headers-episodes.cha");
    let (reprs, diags) = parse_lines_and_diags(&input);

    assert!(
        diags.is_empty(),
        "headers-episodes.cha must produce zero diagnostics, got: {diags:?}"
    );

    let expected: Vec<&str> = vec![
        r#"Utf8"#,
        r#"Begin"#,
        r#"Languages { codes: LanguageCodes([LanguageCode("eng")]) }"#,
        r#"Participants { entries: ParticipantEntries([ParticipantEntry { speaker_code: SpeakerCode("CHI"), name: None, role: ParticipantRole("Child") }, ParticipantEntry { speaker_code: SpeakerCode("MOT"), name: None, role: ParticipantRole("Mother") }]) }"#,
        r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("CHI"), age: Some(Valid { years: 3, months: Some(0), days: None, raw: "3;00." }), sex: None, group: None, ses: None, role: ParticipantRole("Child"), education: None, custom_field: None })"#,
        r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("MOT"), age: None, sex: None, group: None, ses: None, role: ParticipantRole("Mother"), education: None, custom_field: None })"#,
        r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Episode and inline headers: @New Episode, @G, @Bg, @Eg, @Bck, @Blank, @Number, @Page, @T" })]) } }"#,
        r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Constructs: new_episode_header, g_header, bg_header, eg_header," }), Continuation, Text(BulletContentText { text: "bck_header, blank_header, number_header, page_header, t_header" })]) } }"#,
        r#"Bck { bck: BackgroundDescription("background noise from traffic outside") }"#,
        r#"LazyGem { label: Some(GemLabel("morning routine")) }"#,
        r#"Utterance"#,
        r#"Utterance"#,
        r#"T { text: TDescription("cereal preparation") }"#,
        r#"NewEpisode"#,
        r#"Number { number: Number2 }"#,
        r#"Page { page: PageNumber("42") }"#,
        r#"BeginGem { label: Some(GemLabel("afternoon play")) }"#,
        r#"Utterance"#,
        r#"Blank"#,
        r#"Utterance"#,
        r#"EndGem { label: Some(GemLabel("afternoon play")) }"#,
        r#"BeginGem { label: None }"#,
        r#"Utterance"#,
        r#"EndGem { label: None }"#,
        r#"End"#,
    ];
    assert_eq!(reprs, expected, "headers-episodes.cha model output changed");
}

#[test]
fn headers_time_and_types_fixture_parses_with_zero_diagnostics() {
    let input = read_corpus_fixture("core/headers-time-and-types.cha");
    let (reprs, diags) = parse_lines_and_diags(&input);

    assert!(
        diags.is_empty(),
        "headers-time-and-types.cha must produce zero diagnostics, got: {diags:?}"
    );

    let expected: Vec<&str> = vec![
        r#"Utf8"#,
        r#"Begin"#,
        r#"Languages { codes: LanguageCodes([LanguageCode("eng")]) }"#,
        r#"Participants { entries: ParticipantEntries([ParticipantEntry { speaker_code: SpeakerCode("CHI"), name: None, role: ParticipantRole("Child") }, ParticipantEntry { speaker_code: SpeakerCode("MOT"), name: None, role: ParticipantRole("Mother") }]) }"#,
        r#"Options { options: ChatOptionFlags([Ca]) }"#,
        r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("CHI"), age: Some(Valid { years: 2, months: Some(6), days: None, raw: "2;06." }), sex: Some(Male), group: Some(GroupName("TD")), ses: Some(SesOnly(UC)), role: ParticipantRole("Child"), education: Some(EducationDescription("graduate")), custom_field: Some(CustomIdField("custom field")) })"#,
        r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("MOT"), age: None, sex: Some(Female), group: None, ses: Some(Combined { eth: White, ses: MC }), role: ParticipantRole("Mother"), education: None, custom_field: None })"#,
        r#"TimeStart { start: Parsed { hours: 8, minutes: 30, seconds: 31, millis: None, raw: "8:30:31" } }"#,
        r#"TimeDuration { duration: Parsed { segments: [Range { start: TimeValue { hours: 0, minutes: 17, seconds: 30, millis: None }, end: TimeValue { hours: 0, minutes: 18, seconds: 0, millis: None } }], raw: "17:30-18:00" } }"#,
        r#"Types(TypesHeader { design: DesignType("long"), activity: ActivityType("toyplay"), group: GroupType("TD") })"#,
        r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Time, type, and option headers" })]) } }"#,
        r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Constructs: time_start_header, time_duration_header, time_duration_contents," }), Continuation, Text(BulletContentText { text: "types_header, types_design, types_activity, types_group," }), Continuation, Text(BulletContentText { text: "options_header, options_contents, option_name," }), Continuation, Text(BulletContentText { text: "id_corpus, id_age, id_sex, id_group, id_ses, id_education, id_custom_field" })]) } }"#,
        r#"Utterance"#,
        r#"Utterance"#,
        r#"End"#,
    ];
    assert_eq!(
        reprs, expected,
        "headers-time-and-types.cha model output changed"
    );
}

#[test]
fn headers_media_fixture_parses_with_zero_diagnostics() {
    let input = read_corpus_fixture("core/headers-media.cha");
    let (reprs, diags) = parse_lines_and_diags(&input);

    assert!(
        diags.is_empty(),
        "headers-media.cha must produce zero diagnostics, got: {diags:?}"
    );

    let expected: Vec<&str> = vec![
        r#"Utf8"#,
        r#"Begin"#,
        r#"Languages { codes: LanguageCodes([LanguageCode("eng")]) }"#,
        r#"Participants { entries: ParticipantEntries([ParticipantEntry { speaker_code: SpeakerCode("CHI"), name: None, role: ParticipantRole("Child") }, ParticipantEntry { speaker_code: SpeakerCode("MOT"), name: None, role: ParticipantRole("Mother") }]) }"#,
        r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("sample"), speaker: SpeakerCode("CHI"), age: Some(Valid { years: 2, months: Some(0), days: None, raw: "2;00." }), sex: None, group: None, ses: None, role: ParticipantRole("Child"), education: None, custom_field: None })"#,
        r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("sample"), speaker: SpeakerCode("MOT"), age: None, sex: None, group: None, ses: None, role: ParticipantRole("Mother"), education: None, custom_field: None })"#,
        r#"Media(MediaHeader { filename: MediaFilename("headers-media"), media_type: Video, status: Some(Unlinked) })"#,
        r#"Videos { videos: VideoSpec("1a, 1b, 1c") }"#,
        r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Media headers: @Media with video type and unlinked status, @Videos" })]) } }"#,
        r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Constructs: media_header, media_contents, media_filename, media_type," }), Continuation, Text(BulletContentText { text: "media_status, video_value, unlinked_value, videos_header" })]) } }"#,
        r#"Utterance"#,
        r#"Utterance"#,
        r#"End"#,
    ];
    assert_eq!(reprs, expected, "headers-media.cha model output changed");
}

#[test]
fn headers_comments_fixture_parses_with_zero_diagnostics() {
    let input = read_corpus_fixture("core/headers-comments.cha");
    let (reprs, diags) = parse_lines_and_diags(&input);

    assert!(
        diags.is_empty(),
        "headers-comments.cha must produce zero diagnostics, got: {diags:?}"
    );

    let expected: Vec<&str> = vec![
        r#"Utf8"#,
        r#"Begin"#,
        r#"Languages { codes: LanguageCodes([LanguageCode("eng")]) }"#,
        r#"Participants { entries: ParticipantEntries([ParticipantEntry { speaker_code: SpeakerCode("CHI"), name: None, role: ParticipantRole("Child") }, ParticipantEntry { speaker_code: SpeakerCode("MOT"), name: None, role: ParticipantRole("Mother") }]) }"#,
        r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("CHI"), age: Some(Valid { years: 3, months: Some(0), days: None, raw: "3;00." }), sex: None, group: None, ses: None, role: ParticipantRole("Child"), education: None, custom_field: None })"#,
        r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("MOT"), age: None, sex: None, group: None, ses: None, role: ParticipantRole("Mother"), education: None, custom_field: None })"#,
        r#"Media(MediaHeader { filename: MediaFilename("headers-comments"), media_type: Video, status: Some(Unlinked) })"#,
        r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Comment and warning headers with multi-line continuation" })]) } }"#,
        r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "Constructs: comment_header, warning_header, anything," }), Continuation, Text(BulletContentText { text: "text_with_bullets_and_pics, continuation, rest_of_line" })]) } }"#,
        r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "CHI points to the shelf " }), Bullet(MediaTiming { start_ms: 1234, end_ms: 1567 })]) } }"#,
        r#"Warning { text: WarningText("audio quality degrades after minute 15") }"#,
        r#"Utterance"#,
        r#"Comment { content: BulletContent { segments: BulletContentSegments([Text(BulletContentText { text: "CHI is pointing at the bookshelf while asking" }), Continuation, Text(BulletContentText { text: "this question, looking at MOT" })]) } }"#,
        r#"Utterance"#,
        r#"End"#,
    ];
    assert_eq!(reprs, expected, "headers-comments.cha model output changed");
}

#[test]
fn unsupported_header_recovers_silently_and_thumbnail_gap_rejects_with_e525() {
    let (reprs, diags) = parse_lines_and_diags(UNSUPPORTED_AND_THUMBNAIL);

    // `unsupported_header` (the catch-all `simple::unsupported`) recovers to
    // `Header::Unknown` capturing the WHOLE header text (including its
    // trailing newline) and reports NO diagnostic. `thumbnail_header` (the
    // `dispatch::thumbnail` gap handler) is REJECTED outright (no fallback
    // line at all), so it does not appear in the model; only its E525
    // diagnostic below is observable.
    let expected: Vec<&str> = vec![
        r#"Utf8"#,
        r#"Begin"#,
        r#"Languages { codes: LanguageCodes([LanguageCode("eng")]) }"#,
        r#"Participants { entries: ParticipantEntries([ParticipantEntry { speaker_code: SpeakerCode("CHI"), name: None, role: ParticipantRole("Child") }]) }"#,
        r#"ID(IDHeader { language: LanguageCodes([LanguageCode("eng")]), corpus: CorpusName("corpus"), speaker: SpeakerCode("CHI"), age: None, sex: None, group: None, ses: None, role: ParticipantRole("Child"), education: None, custom_field: None })"#,
        "Unknown { text: WarningText(\"@Foo:\\tunexpected header text\\n\"), parse_reason: Some(\"Unsupported header type\"), suggested_fix: None }",
        r#"Utterance"#,
        r#"End"#,
    ];
    assert_eq!(
        reprs, expected,
        "unsupported/thumbnail model output changed"
    );

    assert_eq!(
        diags,
        vec![(
            "E525".to_string(),
            116,
            155,
            "Unrecognized header type 'thumbnail_header'".to_string(),
        )],
        "expected exactly one E525 for the thumbnail_header gap (unsupported_header emits none), got: {diags:?}"
    );
}
