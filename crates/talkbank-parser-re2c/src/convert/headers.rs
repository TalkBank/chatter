//! Part of the AST→model conversion (see `mod.rs`); split out for file size.
#![allow(clippy::unreachable, clippy::unwrap_used, clippy::expect_used)]

use crate::ast;
use crate::source_text::SourceText;
use crate::token::{HeaderKind, Token};
use talkbank_model::model::*;

/// Convert a located header using its admitted source owner.
///
/// A caller cannot replace the source used to place header media bullets:
///
/// ```compile_fail
/// use talkbank_parser_re2c::{ast::LocatedHeader, convert::header_to_model};
/// use talkbank_parser_re2c::source_text::SourceText;
/// fn replace_source(header: &LocatedHeader<'_>, foreign: &str) {
///     header_to_model(header, SourceText::new(foreign));
/// }
/// ```
pub fn header_to_model(located: &ast::LocatedHeader<'_>) -> Header {
    let h = located.header();
    let source = located.source();
    let (original, prefix, content) = match h {
        ast::HeaderParsed::Shared(header) => return header.clone(),
        ast::HeaderParsed::Other {
            source,
            prefix,
            content,
        } => (*source, prefix, content),
    };
    let all_content: String = content.iter().map(|token| token.text()).collect();
    match prefix {
        Token::HeaderUtf8(_) => Header::Utf8,
        Token::HeaderBegin(_) => Header::Begin,
        Token::HeaderEnd(_) => Header::End,
        Token::HeaderBlank(_) => Header::Blank,
        Token::HeaderNewEpisode(_) => Header::NewEpisode,
        Token::HeaderPrefix(prefix) => {
            named_header_to_model(prefix.kind(), content, &all_content, original, source)
        }
        Token::HeaderBirthOf(speaker) => Header::Birth {
            participant: SpeakerCode::new(speaker.text()),
            date: ChatDate::new(&all_content),
        },
        Token::HeaderBirthplaceOf(speaker) => Header::Birthplace {
            participant: SpeakerCode::new(speaker.text()),
            place: BirthplaceDescription::new(&all_content),
        },
        Token::HeaderL1Of(speaker) => match LanguageCode::new(&all_content) {
            Ok(language) => Header::L1Of {
                participant: SpeakerCode::new(speaker.text()),
                language,
            },
            Err(_) => Header::Unknown {
                text: WarningText::new(original),
                parse_reason: Some("Empty language value in @L1 of header".to_string()),
                suggested_fix: None,
            },
        },
        _ => unknown_header(original),
    }
}

fn unknown_header(original: &str) -> Header {
    Header::Unknown {
        text: WarningText::new(original),
        parse_reason: None,
        suggested_fix: None,
    }
}

/// Exact lexical identity selects semantics; unsupported names retain source.
#[deny(clippy::wildcard_enum_match_arm)]
fn named_header_to_model(
    kind: HeaderKind,
    content: &[Token<'_>],
    text: &str,
    original: &str,
    source: SourceText<'_>,
) -> Header {
    match kind {
        HeaderKind::Comment => Header::Comment {
            content: tokens_to_bullet_content(content, source),
        },
        HeaderKind::Date => Header::Date {
            date: ChatDate::new(text),
        },
        HeaderKind::Situation => Header::Situation {
            text: SituationDescription::new(text),
        },
        HeaderKind::Location => Header::Location {
            location: LocationDescription::new(text),
        },
        HeaderKind::Activities => Header::Activities {
            activities: ActivitiesDescription::new(text),
        },
        HeaderKind::Pid => Header::Pid {
            pid: PidValue::new(text),
        },
        HeaderKind::Options => Header::Options {
            options: ChatOptionFlags::new(
                text.split(',')
                    .map(|s| ChatOptionFlag::from_text(s.trim()))
                    .collect(),
            ),
        },
        HeaderKind::BeginGem => Header::BeginGem {
            label: (!text.is_empty()).then(|| gem_label(content)),
        },
        HeaderKind::EndGem => Header::EndGem {
            label: (!text.is_empty()).then(|| gem_label(content)),
        },
        HeaderKind::LazyGem => Header::LazyGem {
            label: (!text.is_empty()).then(|| gem_label(content)),
        },
        HeaderKind::Font => Header::Font {
            font: FontSpec::new(text),
        },
        HeaderKind::Window => Header::Window {
            geometry: WindowGeometry::new(text),
        },
        HeaderKind::ColorWords => Header::ColorWords {
            colors: ColorWordList::new(text),
        },
        HeaderKind::RecordingQuality => Header::RecordingQuality {
            quality: RecordingQuality::from_text(text),
        },
        HeaderKind::Transcription => Header::Transcription {
            transcription: Transcription::from_text(text),
        },
        HeaderKind::Number => Header::Number {
            number: Number::from_text(text),
        },
        HeaderKind::RoomLayout => Header::RoomLayout {
            layout: RoomLayoutDescription::new(text),
        },
        HeaderKind::TapeLocation => Header::TapeLocation {
            location: TapeLocationDescription::new(text),
        },
        HeaderKind::TimeDuration => Header::TimeDuration {
            duration: TimeDurationValue::new(text),
        },
        HeaderKind::TimeStart => Header::TimeStart {
            start: TimeStartValue::new(text),
        },
        HeaderKind::Transcriber => Header::Transcriber {
            transcriber: TranscriberName::new(text),
        },
        HeaderKind::Warning => Header::Warning {
            text: WarningText::new(text),
        },
        HeaderKind::Videos => Header::Videos {
            videos: VideoSpec::new(text),
        },
        HeaderKind::T => Header::T {
            text: TDescription::new(text),
        },
        HeaderKind::Bck => Header::Bck {
            bck: BackgroundDescription::new(text),
        },
        // Structured declarations are lowered by the dispatcher. A raw
        // representation of one is not an admitted structured header.
        HeaderKind::Types
        | HeaderKind::Page
        | HeaderKind::Id
        | HeaderKind::Languages
        | HeaderKind::Participants
        | HeaderKind::Media
        | HeaderKind::Thumbnail
        | HeaderKind::Unknown => unknown_header(original),
    }
}

/// Convert a sequence of content tokens to BulletContent, preserving continuations.
pub(crate) fn tokens_to_bullet_content(
    tokens: &[Token<'_>],
    source: SourceText<'_>,
) -> BulletContent {
    let mut segments = Vec::new();
    for tok in tokens {
        match tok {
            Token::TextSegment(s) | Token::HeaderContent(s) => {
                segments.push(BulletContentSegment::text(*s));
            }
            Token::Continuation(_) => {
                segments.push(BulletContentSegment::continuation());
            }
            Token::MediaBullet { bullet } => match bullet.times().map(|times| times.pair()) {
                Some((0, 0)) | None => {
                    segments.push(BulletContentSegment::UnparsedBullet(UnparsedBullet::new(
                        super::locate(source, bullet.text()),
                        bullet.text(),
                    )));
                }
                Some((start_ms, end_ms)) => {
                    segments.push(BulletContentSegment::bullet(start_ms, end_ms));
                }
            },
            Token::InlinePic(s) => {
                // The token's payload is the tag-extracted filename.
                segments.push(BulletContentSegment::picture(s.text()));
            }
            _ => {
                // Other tokens (LanguageCode, ParticipantWord, etc.), include as text
                segments.push(BulletContentSegment::text(tok.text()));
            }
        }
    }
    BulletContent::new(segments)
}

/// A continuation token separates logical label words; it is not label data.
fn gem_label(content: &[Token<'_>]) -> GemLabel {
    let text: String = content
        .iter()
        .map(|token| match token {
            Token::Continuation(_) => " ",
            token => token.text(),
        })
        .collect();
    GemLabel::new(text)
}
