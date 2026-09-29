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

/// Participant-list consumers must distinguish absent demographics from actual
/// values; the convenience view must not substitute transcript defaults.
#[test]
fn reference_participant_views_preserve_known_and_absent_demographics() {
    use talkbank_model::model::Sex;
    use talkbank_parser_tests::repo_paths::workspace_root;

    let parser = TreeSitterParser::new().expect("parser");
    let source = std::fs::read_to_string(
        workspace_root().join("corpus/reference/core/headers-speaker-info.cha"),
    )
    .expect("speaker metadata reference");
    let file = strict_parse(parser.parse_chat_file(&source)).expect("reference parses");
    for (code, age, sex, birth) in [
        ("CHI", Some("1;08.02"), Some(Sex::Female), true),
        ("MOT", None, Some(Sex::Female), true),
        ("F_A_T", None, None, false),
    ] {
        let participant = file.get_participant(code).expect("joined participant");
        assert_eq!(participant.speaker_code(), code);
        assert_eq!(participant.age(), age);
        assert_eq!(participant.sex(), sex.as_ref());
        assert_eq!(participant.has_birth_date(), birth);
        assert_eq!(participant.corpus(), Some("corpus"));
        assert_eq!(
            participant
                .languages()
                .iter()
                .map(|code| code.as_str())
                .collect::<Vec<_>>(),
            ["eng", "ara"],
        );
    }
    assert!(file.get_participant("UNK").is_none());

    // A syntactically structured ID can still lack its corpus. A readable
    // participant view is not a certificate that validation succeeded.
    let source = std::fs::read_to_string(
        workspace_root()
            .join("crates/talkbank-parser-tests/tests/error_corpus/validation_errors/E514_1.cha"),
    )
    .expect("empty-corpus specification");
    let mut file = strict_parse(parser.parse_chat_file(&source)).expect("ID syntax parses");
    let participant = file.get_participant("CHI").expect("ID join retained");
    assert_eq!(participant.corpus(), None);
    assert_eq!(participant.age(), None);
    assert_eq!(participant.sex(), None);
    assert!(!participant.has_birth_date());
    let errors = talkbank_model::ErrorCollector::new();
    file.validate_with_alignment(&errors, talkbank_model::model::TranscriptName::Anonymous);
    assert!(
        errors
            .to_vec()
            .iter()
            .any(|error| error.code.to_string() == "E514")
    );
}

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
    let mut construction_failures = Vec::new();
    let mut timing_witnesses = 0;
    let mut media_witnesses = 0;
    let mut comment_witnessed = false;
    let mut language_override_witnessed = false;
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
                    if let Some(age) = &id.age {
                        participant = participant.with_age(age.clone());
                    }
                    if let Some(sex) = &id.sex {
                        participant = participant.with_sex(sex.clone());
                    }
                    if let Some(group) = &id.group {
                        participant = participant.with_group(group.clone());
                    }
                    if let Some(ses) = &id.ses {
                        participant = participant.with_ses(ses.clone());
                    }
                    if let Some(education) = &id.education {
                        participant = participant.with_education(education.clone());
                    }
                    if let Some(custom) = &id.custom_field {
                        participant = participant.with_custom(custom.clone());
                    }
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
        // Move each description through its consuming builder methods. No
        // placeholder or cloned participant is needed to move out of a borrow.
        desc.participants = desc
            .participants
            .into_iter()
            .map(|mut participant| {
                for line in &source.lines {
                    let Line::Header { header, .. } = line else {
                        continue;
                    };
                    match header.as_ref() {
                        Header::Participants { entries } => {
                            if let Some(entry) = entries
                                .iter()
                                .find(|entry| entry.speaker_code.as_str() == participant.id)
                                && let Some(name) = &entry.name
                            {
                                participant = participant.with_name(name.as_str());
                            }
                        }
                        Header::L1Of {
                            participant: code,
                            language,
                        } if code.as_str() == participant.id => {
                            participant = participant.with_l1_language(language.clone());
                        }
                        _ => {}
                    }
                }
                participant
            })
            .collect();
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
        let built = match build_chat(&desc) {
            Ok(built) => built,
            Err(error) => {
                construction_failures.push(format!("{}: {error}", fixture.path().display()));
                continue;
            }
        };
        if !comment_witnessed {
            let observed_comment =
                source
                    .utterances()
                    .enumerate()
                    .find_map(|(index, utterance)| {
                        utterance
                            .dependent_tiers
                            .iter()
                            .find_map(|entry| match &entry.tier {
                                talkbank_model::model::DependentTier::Com(comment) => {
                                    Some((index, comment.clone()))
                                }
                                _ => None,
                            })
                    });
            if let Some((index, comment)) = observed_comment {
                let mut commented = desc.clone();
                commented.utterances[index].comment = Some(comment.clone());
                let with_comment = build_chat(&commented).expect("reference row comment");
                let row = with_comment.utterances().nth(index).expect("comment owner");
                assert!(row.dependent_tiers.iter().any(|entry| {
                    matches!(&entry.tier, talkbank_model::model::DependentTier::Com(actual)
                        if actual == &comment)
                }));
                commented.utterances[index].text.clear();
                assert!(
                    matches!(build_chat(&commented), Err(BuildChatError::Build(_))),
                    "an orphan row comment must not disappear with empty speech"
                );
                commented.utterances[index].comment = None;
                let omitted = build_chat(&commented).expect("empty row without attached evidence");
                let remaining: Vec<_> = omitted.utterances().collect();
                let expected: Vec<_> = built
                    .utterances()
                    .enumerate()
                    .filter_map(|(i, row)| (i != index).then_some(row))
                    .collect();
                assert_eq!(remaining.len(), expected.len());
                for (actual, expected) in remaining.iter().zip(expected) {
                    assert!(
                        actual.semantic_eq(expected),
                        "surviving rows retain order and payload"
                    );
                }
                comment_witnessed = true;
            }
        }
        if !language_override_witnessed {
            let observed = source.utterances().enumerate().find(|(_, utterance)| {
                utterance
                    .main
                    .content
                    .language_code
                    .as_ref()
                    .is_some_and(|code| code.as_str() != desc.langs[0])
            });
            if let Some((index, original)) = observed {
                let mut overridden = desc.clone();
                let mut payload = original.main.content.clone();
                let code = payload.language_code.take().expect("observed override");
                overridden.utterances[index].text = payload.to_content_string();
                overridden.utterances[index].lang = Some(code.as_str().to_owned());
                let result =
                    build_chat(&overridden).expect("source language through structured input");
                assert!(
                    result
                        .utterances()
                        .nth(index)
                        .expect("overridden row")
                        .main
                        .semantic_eq(&original.main)
                );

                let mut primary = desc.clone();
                primary.utterances[index].lang = Some(desc.langs[0].clone());
                let unchanged = build_chat(&primary).expect("primary language is no override");
                assert!(unchanged.semantic_eq(&built));

                overridden.utterances[index].lang = Some(String::new());
                assert!(
                    matches!(build_chat(&overridden), Err(BuildChatError::Build(_))),
                    "empty explicit override must be refused, not replaced by a default"
                );
                language_override_witnessed = true;
            }
        }
        if desc.media_name.is_some() {
            let media_header = |file: &talkbank_model::model::ChatFile| {
                file.lines
                    .iter()
                    .find_map(|line| match line {
                        Line::Header { header, .. } => match header.as_ref() {
                            Header::Media(media) => Some(media.clone()),
                            _ => None,
                        },
                        Line::Utterance(_) => None,
                    })
                    .expect("built media")
            };
            assert_eq!(
                media_header(&built).media_type.as_str(),
                desc.media_type.as_deref().expect("source media type")
            );
            let original_media = media_header(&source);
            if !original_media.filename.is_remote_url() {
                let mut local = desc.clone();
                local.media_name =
                    Some(format!("recordings/{}.wav", media_header(&built).filename));
                assert_eq!(
                    media_header(&build_chat(&local).expect("valid directory control")).filename,
                    media_header(&built).filename,
                );
                for directory in ["bad,dir", "bad\ndir", "bad\rdir", "bad\"dir", " leading"] {
                    local.media_name = Some(format!("{directory}/{}.wav", original_media.filename));
                    assert!(
                        build_chat(&local).is_err(),
                        "discarded malformed directory: {directory:?}"
                    );
                }
            }
            if original_media.filename.is_remote_url()
                && media_header(&built).filename != original_media.filename
            {
                construction_failures.push(format!(
                    "{}: remote URL changed from {:?} to {:?}",
                    fixture.path().display(),
                    original_media.filename,
                    media_header(&built).filename,
                ));
            }
            let mut implicit = desc.clone();
            implicit.media_type = None;
            assert_eq!(
                media_header(&build_chat(&implicit).expect("documented absent-type default"))
                    .media_type,
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
        construction_failures.is_empty(),
        "{}",
        construction_failures.join("\n")
    );
    assert!(
        built_count > 0,
        "reference descriptions witness builder admission"
    );
    assert!(
        comment_witnessed,
        "reference comment witnesses ownership and refusal"
    );
    assert!(
        language_override_witnessed,
        "reference language precode witnesses override admission"
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
