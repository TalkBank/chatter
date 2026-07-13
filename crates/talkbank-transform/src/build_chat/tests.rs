use talkbank_model::model::{Header, Line, MediaStatus};

use super::{BuildChatError, ParticipantDesc, TranscriptDescription, UtteranceDesc, build_chat};

fn desc_with(status: Option<MediaStatus>) -> TranscriptDescription {
    TranscriptDescription {
        langs: vec!["eng".to_string()],
        participants: vec![ParticipantDesc {
            id: "CHI".to_string(),
            name: None,
            role: "Target_Child".to_string(),
            corpus: "test".to_string(),
        }],
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

#[test]
fn text_utterance_is_parsed_into_the_model() {
    let chat = build_chat(&desc_with(Some(MediaStatus::Unlinked))).expect("build_chat");
    let has_utterance = chat
        .lines
        .iter()
        .any(|line| matches!(line, Line::Utterance(_)));
    assert!(has_utterance, "the CHI text utterance should be present");
}
