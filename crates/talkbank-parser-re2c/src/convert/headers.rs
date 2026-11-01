//! Part of the AST→model conversion (see `mod.rs`); split out for file size.

use crate::ast;
use crate::source_text::SourceText;
use crate::token::Token;
use talkbank_model::model::*;

/// The model header of a located header: the line machine already lowered
/// it, once, to the shared model.
pub fn header_to_model(located: &ast::LocatedHeader<'_>) -> Header {
    located.header().clone()
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
