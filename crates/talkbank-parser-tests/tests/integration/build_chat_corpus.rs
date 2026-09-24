//! Builder wire contracts from canonical parsed reference descriptions.
#![allow(clippy::expect_used, clippy::panic)]

use talkbank_model::{
    WriteChat,
    model::{Header, Line, SemanticEq},
};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::{chat_corpus::ChatCorpus, test_error::strict_parse};
use talkbank_transform::build_chat::{
    BuildChatError, ParticipantDesc, TranscriptDescription, UtteranceDesc, build_chat,
};

#[test]
fn reference_descriptions_preserve_main_tiers_and_participant_demographics() {
    let parser = TreeSitterParser::new().expect("parser");
    let corpus = ChatCorpus::reference().expect("reference corpus");
    let unsupported_input = std::fs::read_to_string(
        talkbank_parser_tests::repo_paths::workspace_root()
            .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E535_3.cha"),
    )
    .expect("authored unsupported media type mutation");
    let unsupported =
        strict_parse(parser.parse_chat_file(&unsupported_input)).expect("E535 parses");
    let unsupported_type = unsupported
        .lines
        .iter()
        .find_map(|line| match line {
            Line::Header { header, .. } => match header.as_ref() {
                Header::Media(media) => Some(media.media_type.clone()),
                _ => None,
            },
            Line::Utterance(_) => None,
        })
        .expect("E535 media declaration");
    assert!(matches!(
        unsupported_type,
        talkbank_model::model::MediaType::Unsupported(_)
    ));
    let mut built_count = 0;
    let mut timing_witnesses = 0;
    let mut media_witnesses = 0;
    for fixture in corpus.fixtures() {
        let source =
            strict_parse(parser.parse_chat_file(fixture.source())).expect("reference parses");
        // This is an explicit projection into the builder's input vocabulary,
        // not a claim that it can reconstruct every header/dependent tier.
        let mut desc = TranscriptDescription {
            langs: Vec::new(),
            participants: Vec::new(),
            media_name: None,
            media_type: None,
            pid: None,
            media_status: None,
            date: None,
            situation: None,
            options: None,
            transcriber: None,
            comments: Vec::new(),
            utterances: Vec::new(),
        };
        let mut original_ids = Vec::new();
        for line in &source.lines {
            let Line::Header { header, .. } = line else {
                continue;
            };
            match header.as_ref() {
                Header::Languages { codes } => {
                    desc.langs = codes.iter().map(|c| c.as_str().to_owned()).collect()
                }
                Header::ID(id) => {
                    let mut participant = ParticipantDesc::new(
                        id.speaker.as_str(),
                        id.role.as_str(),
                        id.corpus.as_str(),
                    );
                    participant.age = id.age.clone();
                    participant.sex = id.sex.clone();
                    participant.group = id.group.clone();
                    participant.ses = id.ses.clone();
                    participant.education = id.education.clone();
                    participant.custom = id.custom_field.clone();
                    desc.participants.push(participant);
                    original_ids.push(id);
                }
                Header::Pid { pid } => desc.pid = Some(pid.clone()),
                Header::Options { options } => desc.options = Some(options.clone()),
                Header::Date { date } => desc.date = Some(date.clone()),
                Header::Situation { text } => desc.situation = Some(text.clone()),
                Header::Transcriber { transcriber } => desc.transcriber = Some(transcriber.clone()),
                Header::Media(media) => {
                    desc.media_name = Some(media.filename.as_str().to_owned());
                    desc.media_type = Some(media.media_type.as_str().to_owned());
                    desc.media_status = media.status.clone();
                }
                _ => {}
            }
        }
        for line in &source.lines {
            let Line::Header { header, .. } = line else {
                continue;
            };
            match header.as_ref() {
                Header::Participants { entries } => {
                    for entry in entries.iter() {
                        let participant = desc
                            .participants
                            .iter_mut()
                            .find(|p| p.id == entry.speaker_code.as_str())
                            .expect("declared ID");
                        participant.name = entry.name.as_ref().map(|name| name.as_str().to_owned());
                    }
                }
                Header::L1Of {
                    participant,
                    language,
                } => {
                    let entry = desc
                        .participants
                        .iter_mut()
                        .find(|p| p.id == participant.as_str())
                        .expect("L1 participant ID");
                    entry.l1_language = Some(language.clone());
                }
                _ => {}
            }
        }
        // Test the builder's documented text boundary with serialized main-tier
        // content. No semantic facts are re-derived from those bytes.
        desc.utterances = source
            .utterances()
            .map(|u| UtteranceDesc {
                speaker: u.main.speaker.as_str().to_owned(),
                text: u.main.content.to_content_string(),
                comment: None,
                start_ms: None,
                end_ms: None,
                lang: None,
            })
            .collect();
        let built = build_chat(&desc)
            .unwrap_or_else(|e| panic!("reference description {}: {e}", fixture.path().display()));
        if desc.media_name.is_some() {
            let media_type = |file: &talkbank_model::model::ChatFile| {
                file.lines
                    .iter()
                    .find_map(|line| match line {
                        Line::Header { header, .. } => match header.as_ref() {
                            Header::Media(media) => Some(media.media_type.clone()),
                            _ => None,
                        },
                        Line::Utterance(_) => None,
                    })
                    .expect("built media")
            };
            assert_eq!(
                media_type(&built).as_str(),
                desc.media_type.as_deref().expect("source media type")
            );
            let mut implicit = desc.clone();
            implicit.media_type = None;
            assert_eq!(
                media_type(&build_chat(&implicit).expect("documented absent-type default")),
                talkbank_model::model::MediaType::Audio
            );
            let mut invalid = desc.clone();
            invalid.media_type = Some(unsupported_type.as_str().to_owned());
            assert!(
                matches!(build_chat(&invalid), Err(BuildChatError::Build(_))),
                "unsupported authored type must not silently become audio"
            );
            media_witnesses += 1;
        }
        assert_eq!(built.utterances().count(), source.utterances().count());
        for (built, original) in built.utterances().zip(source.utterances()) {
            assert!(
                built.main.semantic_eq(&original.main),
                "main-tier payload: {}\nbuilt: {:?}\noriginal: {:?}",
                fixture.path().display(),
                built.main,
                original.main
            );
        }
        let built_ids: Vec<_> = built
            .lines
            .iter()
            .filter_map(|line| match line {
                Line::Header { header, .. } => match header.as_ref() {
                    Header::ID(id) => Some(id),
                    _ => None,
                },
                Line::Utterance(_) => None,
            })
            .collect();
        assert_eq!(built_ids.len(), original_ids.len());
        for participant in &desc.participants {
            let entries = built
                .lines
                .iter()
                .find_map(|line| match line {
                    Line::Header { header, .. } => match header.as_ref() {
                        Header::Participants { entries } => Some(entries),
                        _ => None,
                    },
                    Line::Utterance(_) => None,
                })
                .expect("built participant declarations");
            let entry = entries
                .iter()
                .find(|e| e.speaker_code.as_str() == participant.id)
                .expect("built participant entry");
            assert_eq!(
                entry.name.as_ref().map(|name| name.as_str()),
                participant.name.as_deref()
            );
            let l1 = built.lines.iter().find_map(|line| match line {
                Line::Header { header, .. } => match header.as_ref() {
                    Header::L1Of {
                        participant: code,
                        language,
                    } if code.as_str() == participant.id => Some(language),
                    _ => None,
                },
                Line::Utterance(_) => None,
            });
            assert_eq!(l1, participant.l1_language.as_ref());
        }
        for (actual, original) in built_ids.iter().zip(original_ids) {
            let mut expected = original.clone();
            // The builder explicitly writes the transcript's primary language
            // into every ID; per-ID language sets are outside its input schema.
            expected.language = talkbank_model::model::LanguageCodes::new(vec![
                talkbank_model::LanguageCode::new(&desc.langs[0]).expect("parsed primary language"),
            ]);
            assert!(
                actual.semantic_eq(&expected),
                "all representable ID demographics survive"
            );
        }
        let reparsed = strict_parse(parser.parse_chat_file(&built.to_chat_string()))
            .expect("builder wire parses");
        assert!(
            built.semantic_eq(&reparsed),
            "builder participant map agrees with serialized headers"
        );
        if let Some((index, original)) = source
            .utterances()
            .enumerate()
            .find(|(_, u)| u.main.content.bullet.is_some())
        {
            let mut timed = desc.clone();
            let mut content = original.main.content.clone();
            let bullet = content.bullet.take().expect("observed terminal timing");
            let row = &mut timed.utterances[index];
            row.text = content.to_content_string();
            row.start_ms = Some(bullet.timing.start_ms);
            row.end_ms = Some(bullet.timing.end_ms);
            let completed =
                build_chat(&timed).expect("source timing supplied through structured fields");
            assert!(
                completed
                    .utterances()
                    .nth(index)
                    .expect("built timed row")
                    .main
                    .semantic_eq(&original.main)
            );
            for missing_start in [true, false] {
                let mut partial = timed.clone();
                if missing_start {
                    partial.utterances[index].start_ms = None;
                } else {
                    partial.utterances[index].end_ms = None;
                }
                assert!(
                    matches!(build_chat(&partial), Err(BuildChatError::Build(_))),
                    "a one-field timing omission must refuse, not erase supplied evidence"
                );
            }
            timed.utterances[index].text.clear();
            assert!(
                matches!(build_chat(&timed), Err(BuildChatError::Build(_))),
                "complete timing without main-tier content must not silently disappear"
            );
            timing_witnesses += 1;
        }
        desc.langs.clear();
        assert!(
            matches!(build_chat(&desc), Err(BuildChatError::NoLanguages)),
            "removing declared languages must refuse, never invent a primary language"
        );
        built_count += 1;
    }
    assert!(
        built_count > 0,
        "reference descriptions witness builder admission"
    );
    assert!(
        timing_witnesses > 0,
        "source timing witnesses complete and deliberate omission cases"
    );
    assert!(
        media_witnesses > 0,
        "reference media controls and authored unsupported-type mutation"
    );
}
