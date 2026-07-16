use talkbank_model::model::{Header, Line, MediaStatus};

use super::{BuildChatError, ParticipantDesc, TranscriptDescription, UtteranceDesc, build_chat};

fn desc_with(status: Option<MediaStatus>) -> TranscriptDescription {
    TranscriptDescription {
        langs: vec!["eng".to_string()],
        participants: vec![ParticipantDesc::new("CHI", "Target_Child", "test")],
        media_name: Some("rec.mp3".to_string()),
        media_type: Some("audio".to_string()),
        pid: None,
        media_status: status,
        date: None,
        situation: None,
        options: None,
        transcriber: None,
        comments: vec![],
        utterances: vec![UtteranceDesc {
            speaker: "CHI".to_string(),
            text: "hello world .".to_string(),
            start_ms: None,
            end_ms: None,
            lang: None,
        }],
    }
}

fn media_header(chat: &talkbank_model::model::ChatFile) -> &talkbank_model::model::MediaHeader {
    chat.lines
        .iter()
        .find_map(|line| match line {
            Line::Header { header, .. } => match header.as_ref() {
                Header::Media(media) => Some(media),
                _ => None,
            },
            _ => None,
        })
        .expect("built CHAT should carry an @Media header")
}

/// The core of the MICASE pre-forced-alignment fix: a caller that names its
/// media but has no timing bullets must be able to mark it `unlinked`, so the
/// `@Media` header does not falsely assert linkage (E544).
#[test]
fn media_status_unlinked_is_emitted() {
    let chat = build_chat(&desc_with(Some(MediaStatus::Unlinked))).expect("build_chat");
    assert_eq!(media_header(&chat).status, Some(MediaStatus::Unlinked));
}

#[test]
fn media_status_absent_when_omitted() {
    let chat = build_chat(&desc_with(None)).expect("build_chat");
    assert_eq!(media_header(&chat).status, None);
}

#[test]
fn empty_participants_is_an_error() {
    let mut desc = desc_with(None);
    desc.participants.clear();
    assert!(matches!(
        build_chat(&desc),
        Err(BuildChatError::NoParticipants)
    ));
}

/// Regression: demographics set on a `ParticipantDesc` must reach the emitted
/// `@ID` header. Before this fix the input schema had nowhere to carry age /
/// sex / group / education, so every generator's `@ID` came out demographics-
/// empty (the MICASE converter dropped 1704/1759 populated records this way).
#[test]
fn participant_demographics_reach_the_id_header() {
    use talkbank_model::model::{AgeValue, EducationDescription, GroupName, Header, Sex};

    let mut desc = desc_with(Some(MediaStatus::Unlinked));
    desc.participants = vec![
        ParticipantDesc::new("S1", "Teacher", "MICASE")
            .with_age(AgeValue::from_text("60;"))
            .with_sex(Sex::Female)
            .with_group(GroupName::new("NRN"))
            .with_education(EducationDescription::new("ST")),
    ];
    desc.utterances = vec![UtteranceDesc {
        speaker: "S1".to_string(),
        text: "hello world .".to_string(),
        start_ms: None,
        end_ms: None,
        lang: None,
    }];

    let chat = build_chat(&desc).expect("build_chat");
    let id = chat
        .lines
        .iter()
        .find_map(|line| match line {
            Line::Header { header, .. } => match header.as_ref() {
                Header::ID(id) => Some(id),
                _ => None,
            },
            _ => None,
        })
        .expect("built CHAT should carry an @ID header");

    assert_eq!(id.age, Some(AgeValue::from_text("60;")));
    assert_eq!(id.sex, Some(Sex::Female));
    assert_eq!(id.group, Some(GroupName::new("NRN")));
    assert_eq!(id.education, Some(EducationDescription::new("ST")));
}

/// Regression: `@Date` / `@Situation` / `@Options` set on the description must
/// reach the emitted header block (they had no home before, so every generator
/// dropped them). Also pins the published-MICASE ordering: `@Options` before
/// the `@ID` block, `@Date`/`@Situation` after `@Media`.
#[test]
fn transcript_headers_date_situation_options_are_emitted_in_order() {
    use talkbank_model::WriteChat;
    use talkbank_model::model::{
        ChatDate, ChatOptionFlag, ChatOptionFlags, PidValue, SituationDescription, TranscriberName,
    };

    let mut desc = desc_with(Some(MediaStatus::Unlinked));
    desc.pid = Some(PidValue::new("11312/a-00065721-1"));
    desc.date = Some(ChatDate::new("07-JUL-1998"));
    desc.situation = Some(SituationDescription::new("Honors Advising Office"));
    desc.options = Some(ChatOptionFlags(vec![ChatOptionFlag::Ca]));
    desc.transcriber = Some(TranscriberName::new("Jane Doe"));

    let chat = build_chat(&desc).expect("build_chat");
    let text = chat.to_chat_string();
    assert!(text.contains("@PID:\t11312/a-00065721-1"), "expected @PID");
    // @PID sits between @UTF8 and @Begin.
    let posn = |n: &str| text.find(n).expect(n);
    assert!(posn("@UTF8") < posn("@PID:"), "@PID after @UTF8");
    assert!(posn("@PID:") < posn("@Begin"), "@PID before @Begin");
    assert!(text.contains("@Options:\tCA"), "expected @Options: CA");
    assert!(text.contains("@Date:\t07-JUL-1998"), "expected @Date");
    assert!(
        text.contains("@Situation:\tHonors Advising Office"),
        "expected @Situation"
    );
    assert!(
        text.contains("@Transcriber:\tJane Doe"),
        "expected @Transcriber"
    );
    // Ordering: @Options before @ID; @Date/@Situation after @Media.
    let pos = |needle: &str| text.find(needle).expect(needle);
    assert!(pos("@Options:") < pos("@ID:"), "@Options must precede @ID");
    assert!(pos("@Media:") < pos("@Date:"), "@Date must follow @Media");
    assert!(
        pos("@Date:") < pos("@Situation:"),
        "@Date before @Situation"
    );
}

/// Regression: `@L1 of` (per participant, immediately after the `@ID` block)
/// and `@Comment` (end of header block) must be emitted from the description.
#[test]
fn l1_of_and_comment_headers_are_emitted_in_order() {
    use talkbank_model::WriteChat;
    use talkbank_model::model::LanguageCode;

    let mut desc = desc_with(Some(MediaStatus::Unlinked));
    desc.participants = vec![
        ParticipantDesc::new("S1", "Teacher", "MICASE")
            .with_l1_language(LanguageCode::new("est").expect("non-empty literal")),
    ];
    desc.utterances = vec![UtteranceDesc {
        speaker: "S1".to_string(),
        text: "hello .".to_string(),
        start_ms: None,
        end_ms: None,
        lang: None,
    }];
    desc.comments = vec!["S1 quotation restriction: cite".to_string()];

    let text = build_chat(&desc).expect("build_chat").to_chat_string();
    assert!(text.contains("@L1 of S1:\test"), "expected coded @L1 of");
    assert!(
        text.contains("@Comment:\tS1 quotation restriction: cite"),
        "expected @Comment"
    );
    let pos = |n: &str| text.find(n).expect(n);
    assert!(pos("@ID:") < pos("@L1 of"), "@L1 of must follow @ID");
    assert!(pos("@L1 of") < pos("@Media:"), "@L1 of must precede @Media");
}

#[test]
fn text_utterance_is_parsed_into_the_model() {
    let chat = build_chat(&desc_with(Some(MediaStatus::Unlinked))).expect("build_chat");
    let has_utterance = chat
        .lines
        .iter()
        .any(|line| matches!(line, Line::Utterance(_)));
    assert!(has_utterance, "the CHI text utterance should be present");
}
