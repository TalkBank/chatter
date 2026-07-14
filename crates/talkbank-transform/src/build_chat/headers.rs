use std::path::Path;

use talkbank_model::model::{
    BulletContent, Header, IDHeader, LanguageCode, LanguageCodes, Line, MediaHeader, MediaType,
    ParticipantEntries, ParticipantEntry, ParticipantName, ParticipantRole, SpeakerCode,
};

use super::TranscriptDescription;

pub(super) fn build_header_lines(
    desc: &TranscriptDescription,
    langs: &[LanguageCode],
) -> Vec<Line> {
    let participant_entries = build_participant_entries(desc);
    let id_headers = build_id_headers(desc, langs);
    let mut lines: Vec<Line> = vec![
        Line::header(Header::Utf8),
        Line::header(Header::Begin),
        Line::header(Header::Languages {
            codes: LanguageCodes::new(langs.to_vec()),
        }),
        Line::header(Header::Participants {
            entries: ParticipantEntries::new(participant_entries),
        }),
    ];

    // `@Options` sits between `@Participants` and the `@ID` block (published
    // MICASE order). Optional: emitted only when the caller supplies flags.
    if let Some(options) = &desc.options {
        lines.push(Line::header(Header::Options {
            options: options.clone(),
        }));
    }

    for id in id_headers {
        lines.push(Line::header(Header::ID(id)));
    }

    // `@L1 of` is a constant participant header and must immediately follow the
    // `@ID` block (before `@Media`). Emitted only for participants that carry a
    // first language.
    for participant in &desc.participants {
        if let Some(language) = &participant.l1_language {
            lines.push(Line::header(Header::L1Of {
                participant: SpeakerCode::new(participant.id.as_str()),
                language: language.clone(),
            }));
        }
    }

    if let Some(media_header) = build_media_header(desc) {
        lines.push(Line::header(Header::Media(media_header)));
    }

    // Changeable headers follow `@Media`, in published MICASE order:
    // `@Date` then `@Situation`. Each is optional.
    if let Some(date) = &desc.date {
        lines.push(Line::header(Header::Date { date: date.clone() }));
    }
    if let Some(situation) = &desc.situation {
        lines.push(Line::header(Header::Situation {
            text: situation.clone(),
        }));
    }

    // `@Comment` lines close the header block (e.g. speaker usage restrictions,
    // preserved provenance).
    for comment in &desc.comments {
        lines.push(Line::header(Header::Comment {
            content: BulletContent::from_text(comment),
        }));
    }

    lines
}

fn build_participant_entries(desc: &TranscriptDescription) -> Vec<ParticipantEntry> {
    desc.participants
        .iter()
        .map(|participant| ParticipantEntry {
            speaker_code: SpeakerCode::new(participant.id.as_str()),
            name: participant.name.as_ref().map(ParticipantName::new),
            role: ParticipantRole::new(participant.role.as_str()),
        })
        .collect()
}

fn build_id_headers(desc: &TranscriptDescription, langs: &[LanguageCode]) -> Vec<IDHeader> {
    // BuildChatContext guarantees a non-empty, already-validated list;
    // @ID headers carry the primary (first) language.
    let Some(lang_code) = langs.first() else {
        return Vec::new();
    };

    desc.participants
        .iter()
        .map(|participant| {
            let corpus = if participant.corpus.is_empty() {
                "corpus_name"
            } else {
                participant.corpus.as_str()
            };
            let mut header = IDHeader::new(
                lang_code.clone(),
                participant.id.as_str(),
                participant.role.as_str(),
            )
            .with_corpus(corpus);
            // Wire every demographic the caller supplied. A `None` field stays
            // an empty `@ID` slot; the previous version wired none of these, so
            // parsed demographics were silently dropped from every `@ID`.
            if let Some(age) = &participant.age {
                header = header.with_age(age.clone());
            }
            if let Some(sex) = &participant.sex {
                header = header.with_sex(sex.clone());
            }
            if let Some(group) = &participant.group {
                header = header.with_group(group.clone());
            }
            if let Some(ses) = &participant.ses {
                header = header.with_ses(ses.clone());
            }
            if let Some(education) = &participant.education {
                header = header.with_education(education.clone());
            }
            if let Some(custom) = &participant.custom {
                header = header.with_custom_field(custom.clone());
            }
            header
        })
        .collect()
}

fn build_media_header(desc: &TranscriptDescription) -> Option<MediaHeader> {
    let media_name = desc.media_name.as_ref()?;
    let normalized_media_name = normalize_media_name(media_name);
    let media_type = match desc.media_type.as_deref() {
        Some("video") => MediaType::Video,
        Some("audio") | None => MediaType::Audio,
        other => {
            tracing::warn!(media_type = ?other, "unrecognized media_type, defaulting to audio");
            MediaType::Audio
        }
    };

    let mut header = MediaHeader::new(normalized_media_name.as_str(), media_type);
    if let Some(status) = &desc.media_status {
        header = header.with_status(status.clone());
    }
    Some(header)
}

fn normalize_media_name(raw: &str) -> String {
    let candidate = Path::new(raw);
    candidate
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .or_else(|| candidate.file_name())
        .filter(|name| !name.is_empty())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| raw.to_string())
}
