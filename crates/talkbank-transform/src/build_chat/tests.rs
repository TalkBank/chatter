use talkbank_model::model::{Header, Line, MediaStatus};

use super::{BuildChatError, ParticipantDesc, TranscriptDescription, UtteranceDesc, build_chat};

fn desc_with(status: Option<MediaStatus>) -> TranscriptDescription {
    TranscriptDescription {
        langs: vec!["eng".to_string()],
        participants: vec![ParticipantDesc::new("CHI", "Target_Child", "test")],
        media_name: Some("rec.mp3".to_string()),
        media_type: Some("audio".to_string()),
        media_status: status,
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

#[test]
fn text_utterance_is_parsed_into_the_model() {
    let chat = build_chat(&desc_with(Some(MediaStatus::Unlinked))).expect("build_chat");
    let has_utterance = chat
        .lines
        .iter()
        .any(|line| matches!(line, Line::Utterance(_)));
    assert!(has_utterance, "the CHI text utterance should be present");
}
