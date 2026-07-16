//! Part of the AST→model conversion (see `mod.rs`); split out for file size.
#![allow(clippy::unreachable, clippy::unwrap_used, clippy::expect_used)]

use crate::ast;
use crate::token::Token;
use talkbank_model::model::*;

/// Convert a parsed header to model Header.
pub fn header_to_model(h: &ast::HeaderParsed<'_>) -> Header {
    let prefix_text = h.prefix.text();

    // Join all content token texts for free-text headers.
    // Preserve continuation newlines.
    let all_content: String = h.content.iter().map(|t| t.text()).collect::<String>();

    match &h.prefix {
        Token::HeaderUtf8(_) => Header::Utf8,
        Token::HeaderBegin(_) => Header::Begin,
        Token::HeaderEnd(_) => Header::End,
        Token::HeaderBlank(_) => Header::Blank,
        Token::HeaderNewEpisode(_) => Header::NewEpisode,
        Token::HeaderPrefix(p) if p.contains("@Languages") => {
            // `Token::LanguageCode`'s lexer rule requires at least one
            // character (mirrors the tree-sitter grammar's
            // `language_code: $ => /[a-z]{2,4}/`), so the text is
            // guaranteed non-empty here; `.expect()` is defensive only
            // (this crate's file-level `expect_used` allow covers exactly
            // this kind of lexer-guaranteed-non-empty case).
            let codes: Vec<LanguageCode> = h
                .content
                .iter()
                .filter(|t| matches!(t, Token::LanguageCode(_)))
                .map(|t| {
                    LanguageCode::new(t.text()).expect("lexed language_code token is non-empty")
                })
                .collect();
            Header::Languages {
                codes: LanguageCodes::new(codes),
            }
        }
        Token::HeaderPrefix(p) if p.contains("@Participants") => {
            // Split participant words on Comma tokens
            let mut entries = Vec::new();
            let mut current_words: Vec<&str> = Vec::new();
            for tok in &h.content {
                match tok {
                    Token::ParticipantWord(s) => current_words.push(s),
                    Token::Comma(_) if !current_words.is_empty() => {
                        entries.push(participant_words_to_entry(&current_words));
                        current_words.clear();
                    }
                    _ => {}
                }
            }
            if !current_words.is_empty() {
                entries.push(participant_words_to_entry(&current_words));
            }
            Header::Participants {
                entries: ParticipantEntries::new(entries),
            }
        }
        Token::HeaderPrefix(p) if p.contains("@ID") => {
            // Token struct carries all 10 fields directly, no splitting needed
            if let Some(Token::IdFields {
                language,
                corpus,
                speaker,
                age,
                sex,
                group,
                ses,
                role,
                education,
                custom,
            }) = h.content.first()
            {
                // Language field can be comma-separated: "eng, ara". Filter
                // empty pieces (e.g. a malformed "eng,,ara") before
                // constructing, mirroring the canonical tree-sitter side's
                // `id/parse.rs` guard, so a filtered-non-empty `.expect()`
                // is provably safe rather than reachable on malformed input.
                let lang_codes: Vec<LanguageCode> = language
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| {
                        LanguageCode::new(s).expect("filtered non-empty by the preceding filter")
                    })
                    .collect();
                let mut id = IDHeader::from_languages(
                    LanguageCodes::new(lang_codes),
                    SpeakerCode::new(*speaker),
                    ParticipantRole::new(*role),
                );
                if !corpus.is_empty() {
                    id = id.with_corpus(*corpus);
                }
                if !age.is_empty() {
                    id = id.with_age(*age);
                }
                if !group.is_empty() {
                    id = id.with_group(*group);
                }
                if !ses.is_empty() {
                    id = id.with_ses(*ses);
                }
                if !education.is_empty() {
                    id = id.with_education(*education);
                }
                if !custom.is_empty() {
                    id = id.with_custom_field(*custom);
                }
                if !sex.is_empty() {
                    id = id.with_sex(talkbank_model::model::Sex::from_text(sex));
                }
                return Header::ID(id);
            }
            Header::Unknown {
                text: WarningText::new(format!("{prefix_text}{all_content}")),
                parse_reason: Some("malformed @ID".to_string()),
                suggested_fix: None,
            }
        }
        Token::HeaderPrefix(p) if p.contains("@Types") => {
            if let Some(Token::TypesFields {
                design,
                activity,
                group,
            }) = h.content.first()
            {
                return Header::Types(TypesHeader::new(*design, *activity, *group));
            }
            Header::Unknown {
                text: WarningText::new(all_content),
                parse_reason: Some("malformed @Types".to_string()),
                suggested_fix: None,
            }
        }
        Token::HeaderPrefix(p) if p.contains("@Media") => {
            // Media content is MediaWord/MediaFilename tokens separated by Comma
            let words: Vec<&str> = h
                .content
                .iter()
                .filter(|t| matches!(t, Token::MediaWord(_) | Token::MediaFilename(_)))
                .map(|t| t.text())
                .collect();
            if words.len() >= 2 {
                let mut mh = MediaHeader::new(
                    words[0], // Into<MediaFilename>
                    MediaType::from_text(words[1]),
                );
                if words.len() >= 3 {
                    mh = mh.with_status(MediaStatus::from_text(words[2]));
                }
                Header::Media(mh)
            } else {
                Header::Unknown {
                    text: WarningText::new(all_content),
                    parse_reason: Some("malformed @Media".to_string()),
                    suggested_fix: None,
                }
            }
        }
        Token::HeaderPrefix(p) if p.contains("@Comment") => Header::Comment {
            content: tokens_to_bullet_content(&h.content),
        },
        Token::HeaderPrefix(p) if p.contains("@Date") => Header::Date {
            date: ChatDate::new(&all_content),
        },
        Token::HeaderPrefix(p) if p.contains("@Situation") => Header::Situation {
            text: SituationDescription::new(&all_content),
        },
        Token::HeaderPrefix(p) if p.contains("@Location:") => Header::Location {
            location: LocationDescription::new(&all_content),
        },
        Token::HeaderPrefix(p) if p.contains("@Activities") => Header::Activities {
            activities: ActivitiesDescription::new(&all_content),
        },
        Token::HeaderPrefix(p) if p.contains("@PID") => Header::Pid {
            pid: PidValue::new(&all_content),
        },
        Token::HeaderPrefix(p) if p.contains("@Options") => {
            let flags: Vec<ChatOptionFlag> = all_content
                .split(',')
                .map(|s| ChatOptionFlag::from_text(s.trim()))
                .collect();
            Header::Options {
                options: ChatOptionFlags::new(flags),
            }
        }
        Token::HeaderBirthOf(speaker) => {
            // Token carries tag-extracted speaker code directly
            Header::Birth {
                participant: SpeakerCode::new(*speaker),
                date: ChatDate::new(&all_content),
            }
        }
        Token::HeaderBirthplaceOf(speaker) => Header::Birthplace {
            participant: SpeakerCode::new(*speaker),
            place: BirthplaceDescription::new(&all_content),
        },
        // @L1 of values are ISO 639-3 codes (typed model migration,
        // 2026-07-16); an empty value cannot form a code and mirrors the
        // tree-sitter path's unknown-header fallback.
        Token::HeaderL1Of(speaker) => match LanguageCode::new(&all_content) {
            Ok(language) => Header::L1Of {
                participant: SpeakerCode::new(*speaker),
                language,
            },
            Err(_empty) => Header::Unknown {
                text: WarningText::new(all_content),
                parse_reason: Some("Empty language value in @L1 of header".to_string()),
                suggested_fix: None,
            },
        },
        Token::HeaderPrefix(p) if p.contains("@Bg") => Header::BeginGem {
            label: if all_content.is_empty() {
                None
            } else {
                Some(GemLabel::new(&all_content))
            },
        },
        Token::HeaderPrefix(p) if p.starts_with("@G:") || *p == "@G" => Header::LazyGem {
            label: if all_content.is_empty() {
                None
            } else {
                Some(GemLabel::new(&all_content))
            },
        },
        Token::HeaderPrefix(p) if p.contains("@Eg") => Header::EndGem {
            label: if all_content.is_empty() {
                None
            } else {
                Some(GemLabel::new(&all_content))
            },
        },
        _ => {
            // All other headers, use the appropriate model type based on prefix
            let ct = &all_content;
            if prefix_text.contains("@Font") {
                Header::Font {
                    font: FontSpec::new(ct),
                }
            } else if prefix_text.contains("@Window") {
                Header::Window {
                    geometry: WindowGeometry::new(ct),
                }
            } else if prefix_text.contains("@Color words") {
                Header::ColorWords {
                    colors: ColorWordList::new(ct),
                }
            } else if prefix_text.contains("@Recording Quality") {
                Header::RecordingQuality {
                    quality: RecordingQuality::from_text(ct),
                }
            } else if prefix_text.contains("@Transcription") {
                Header::Transcription {
                    transcription: Transcription::from_text(ct),
                }
            } else if prefix_text.contains("@Number") {
                Header::Number {
                    number: Number::from_text(ct),
                }
            } else if prefix_text.contains("@Room Layout") {
                Header::RoomLayout {
                    layout: RoomLayoutDescription::new(ct),
                }
            } else if prefix_text.contains("@Tape Location") {
                Header::TapeLocation {
                    location: TapeLocationDescription::new(ct),
                }
            } else if prefix_text.contains("@Time Duration") {
                Header::TimeDuration {
                    duration: TimeDurationValue::new(ct),
                }
            } else if prefix_text.contains("@Time Start") {
                Header::TimeStart {
                    start: TimeStartValue::new(ct),
                }
            } else if prefix_text.contains("@Transcriber") {
                Header::Transcriber {
                    transcriber: TranscriberName::new(ct),
                }
            } else if prefix_text.contains("@Warning") {
                Header::Warning {
                    text: WarningText::new(ct),
                }
            } else if prefix_text.contains("@Page") {
                Header::Page {
                    page: PageNumber::new(ct),
                }
            } else if prefix_text.contains("@Videos") {
                Header::Videos {
                    videos: VideoSpec::new(ct),
                }
            } else if prefix_text.starts_with("@T:") || prefix_text == "@T" {
                Header::T {
                    text: TDescription::new(ct),
                }
            } else if prefix_text.contains("@Bck") {
                Header::Bck {
                    bck: BackgroundDescription::new(ct),
                }
            } else {
                Header::Unknown {
                    text: WarningText::new(format!("{prefix_text}{ct}")),
                    parse_reason: None,
                    suggested_fix: None,
                }
            }
        }
    }
}

/// Convert a sequence of content tokens to BulletContent, preserving continuations.
pub(crate) fn tokens_to_bullet_content(tokens: &[Token<'_>]) -> BulletContent {
    let mut segments = Vec::new();
    for tok in tokens {
        match tok {
            Token::TextSegment(s) | Token::HeaderContent(s) => {
                segments.push(BulletContentSegment::text(*s));
            }
            Token::Continuation(_) => {
                segments.push(BulletContentSegment::continuation());
            }
            Token::MediaBullet {
                start_time,
                end_time,
                ..
            } => {
                let start_ms = start_time.parse().unwrap_or(0);
                let end_ms = end_time.parse().unwrap_or(0);
                segments.push(BulletContentSegment::bullet(start_ms, end_ms));
            }
            Token::InlinePic(s) => {
                // Token carries tag-extracted filename directly
                segments.push(BulletContentSegment::picture(*s));
            }
            _ => {
                // Other tokens (LanguageCode, ParticipantWord, etc.), include as text
                segments.push(BulletContentSegment::text(tok.text()));
            }
        }
    }
    BulletContent::new(segments)
}

/// Convert participant words [SPK, Name, Role] to ParticipantEntry.
pub(crate) fn participant_words_to_entry(words: &[&str]) -> ParticipantEntry {
    let speaker_code = SpeakerCode::new(words.first().copied().unwrap_or(""));
    let role = if words.len() >= 2 {
        ParticipantRole::new(*words.last().unwrap())
    } else {
        ParticipantRole::new("")
    };
    let name = if words.len() == 3 {
        Some(ParticipantName::new(words[1]))
    } else {
        None
    };
    ParticipantEntry {
        speaker_code,
        name,
        role,
    }
}

// ═══════════════════════════════════════════════════════════════
// %mor conversions
// ═══════════════════════════════════════════════════════════════

impl<'a> From<&ast::MorWordParsed<'a>> for MorWord {
    fn from(w: &ast::MorWordParsed<'a>) -> Self {
        let mut word = MorWord::new(PosCategory::new(w.pos), MorStem::new(w.lemma));
        for f in &w.features {
            word = word.with_feature(MorFeature::new(*f));
        }
        word
    }
}

impl<'a> From<&ast::MorItem<'a>> for Mor {
    fn from(item: &ast::MorItem<'a>) -> Self {
        let main = MorWord::from(&item.main);
        let mut mor = Mor::new(main);
        for clitic in &item.post_clitics {
            mor = mor.with_post_clitic(MorWord::from(clitic));
        }
        mor
    }
}
