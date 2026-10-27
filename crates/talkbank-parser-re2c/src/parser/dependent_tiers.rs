//! Chumsky parser combinators for dependent tier parsing.
//!
//! Each parser operates on `&[Token<'a>]` (span-stripped token stream) and
//! produces a shared model or a parser-specific carrier. Text-body recognition
//! retains a lexer-token slice and constructs the shared BulletContent directly.
//!
//! Tier parsers: %mor, %gra, %pho, %sin, text tiers.
// Design rule 3, enforced by the compiler rather than by prose: a `_` arm over
// a content enum means a future variant compiles clean and answers wrong.
// Added per file as each is cleaned; `content_catch_alls` lists the rest.
#![deny(clippy::wildcard_enum_match_arm)]

use chumsky::prelude::*;

use crate::token::Token;
use talkbank_model::model::dependent_tier::pho::PhoGroupWords;
use talkbank_model::model::{
    PhoItem, PhoTier, PhoTierType, PhoWord, SinGroupGestures, SinItem, SinTier,
};

use crate::token::TokenDiscriminants;

use super::classify::is_terminator;
use super::main_tier;

/// Chumsky input type: a slice of tokens borrowed from the lexer output.
use super::Tokens;

/// Whitespace/continuation combinator, skips structural whitespace tokens.
pub(super) fn ws<'tokens, 'a: 'tokens>() -> impl Parser<'tokens, Tokens<'tokens, 'a>, ()> + Clone {
    select! { Token::Whitespace(_) => (), Token::Continuation(_) => () }
        .repeated()
        .ignored()
}

/// Optional trailing newline, consumes a Newline token if present.
pub(super) fn opt_newline<'tokens, 'a: 'tokens>()
-> impl Parser<'tokens, Tokens<'tokens, 'a>, ()> + Clone {
    select! { Token::Newline(_) => () }.or_not().ignored()
}

// ═══════════════════════════════════════════════════════════
// %gra tier
// grammar.js: gra_contents = seq(gra_relation+)
// ═══════════════════════════════════════════════════════════

/// Parse a `%gra` tier body: whitespace-separated `GraRelation` tokens.
pub fn gra_tier_parser<'tokens, 'a: 'tokens>(
    source: crate::source_text::SourceText<'a>,
    extent: talkbank_model::Span,
    errors: &'tokens (impl talkbank_model::ErrorSink + ?Sized),
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, talkbank_model::ParseOutcome<talkbank_model::GraTier>>
+ Clone {
    let relation = select! {
        Token::GraRelation { index, head, relation } => super::gra::relation(index, head, relation, source, errors),
    };
    ws().ignore_then(
        relation
            .separated_by(ws())
            .allow_trailing()
            .at_least(1)
            .collect::<Vec<_>>(),
    )
    .then_ignore(ws())
    .then_ignore(opt_newline())
    .map(move |relations| {
        relations
            .into_iter()
            .collect::<talkbank_model::GraLowering>()
            .finish(extent)
    })
}

// ═══════════════════════════════════════════════════════════
// %mor tier
// The shared tier requires an observed terminator; refusal is not a model value.

/// Parse a rich MOR token directly into the shared morphology word.
pub fn mor_word_parser<'tokens, 'a: 'tokens>()
-> impl Parser<'tokens, Tokens<'tokens, 'a>, talkbank_model::MorWord> + Clone {
    use talkbank_model::{MorFeature, MorStem, MorWord, PosCategory};
    select! {
        Token::MorWord { pos, lemma_features } => {
            match lemma_features.split_once('-') {
                Some((lemma, features)) => features.split('-').fold(
                    MorWord::new(PosCategory::new(pos), MorStem::new(lemma)),
                    |word, feature| word.with_feature(MorFeature::new(feature)),
                ),
                None => MorWord::new(PosCategory::new(pos), MorStem::new(lemma_features)),
            }
        },
    }
}

/// Parse a shared MOR item with its observed post-clitics.
fn mor_item_parser<'tokens, 'a: 'tokens>()
-> impl Parser<'tokens, Tokens<'tokens, 'a>, talkbank_model::Mor> + Clone {
    let clitic = select! { Token::MorTilde(_) => () }.ignore_then(mor_word_parser());
    mor_word_parser()
        .map(talkbank_model::Mor::new)
        .foldl(clitic.repeated(), |item, word| item.with_post_clitic(word))
}

/// Parse a shared MOR tier with a required, source-located terminator.
pub fn mor_tier_parser<'tokens, 'a: 'tokens>(
    source: crate::source_text::SourceText<'a>,
    location: talkbank_model::Location,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, talkbank_model::MorTier> + Clone {
    let terminator = main_tier::terminator(source);

    ws().ignore_then(
        mor_item_parser()
            .separated_by(ws())
            .allow_trailing()
            .collect::<Vec<_>>(),
    )
    .then(ws().ignore_then(terminator))
    .then_ignore(ws())
    .then_ignore(opt_newline())
    .map(move |(items, terminator)| talkbank_model::MorTier::new_mor(location, items, terminator))
}

// ═══════════════════════════════════════════════════════════
// %pho tier
// ═══════════════════════════════════════════════════════════

/// Parse a single phonological word (possibly compound with +).
fn pho_word_parser<'tokens, 'a: 'tokens>()
-> impl Parser<'tokens, Tokens<'tokens, 'a>, PhoWord> + Clone {
    let pho_word = select! { Token::PhoWord(s) => s };
    let plus = select! { Token::PhoPlus(_) => () };

    pho_word
        .map(str::to_owned)
        .foldl(
            plus.ignore_then(pho_word).repeated(),
            |mut word, segment| {
                word.push('+');
                word.push_str(segment);
                word
            },
        )
        .map(PhoWord::new)
}

/// Parse a `%pho` tier body.
pub fn pho_tier_parser<'tokens, 'a: 'tokens>(
    location: talkbank_model::Span,
    tier_type: PhoTierType,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, PhoTier> + Clone {
    let terminator = select! {
        tok if is_terminator(TokenDiscriminants::from(&tok)) => tok,
    };

    let group_begin = select! { Token::PhoGroupBegin(_) => () };
    let group_end = select! { Token::PhoGroupEnd(_) => () };

    let pho_group = group_begin
        .ignore_then(
            pho_word_parser()
                .padded_by(ws())
                .repeated()
                .at_least(1)
                .collect::<Vec<_>>(),
        )
        .then_ignore(group_end)
        .map(|words| PhoItem::Group(PhoGroupWords::new(words)));

    let pho_item = choice((pho_group, pho_word_parser().map(PhoItem::Word)));

    ws().ignore_then(pho_item.padded_by(ws()).repeated().collect::<Vec<_>>())
        .then_ignore(ws().ignore_then(terminator).or_not())
        .then_ignore(ws())
        .then_ignore(opt_newline())
        .map(move |items| PhoTier::new(location, tier_type, items))
}

// ═══════════════════════════════════════════════════════════
// %sin tier
// ═══════════════════════════════════════════════════════════

/// Parse a `%sin` tier body.
pub fn sin_tier_parser<'tokens, 'a: 'tokens>(
    location: talkbank_model::Span,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, SinTier> + Clone {
    let sin_word = select! {
        Token::SinWord(s) => s,
        Token::Zero(s) => s,
    }
    .try_map(|text, _| talkbank_model::model::SinToken::new(text).map_err(|_| Default::default()));

    let group_begin = select! { Token::SinGroupBegin(_) => () };
    let group_end = select! { Token::SinGroupEnd(_) => () };

    let sin_group = group_begin
        .ignore_then(
            sin_word
                .padded_by(ws())
                .repeated()
                .at_least(1)
                .collect::<Vec<_>>(),
        )
        .then_ignore(group_end)
        .map(|gestures| SinItem::SinGroup(SinGroupGestures::new(gestures)));

    let sin_item = choice((sin_group, sin_word.map(SinItem::Token)));

    ws().ignore_then(sin_item.padded_by(ws()).repeated().collect::<Vec<_>>())
        .then_ignore(opt_newline())
        .map(move |items| SinTier::new(location, items))
}

// ═══════════════════════════════════════════════════════════
// Text tier (for %com, %act, %cod, %exp, etc.)
// ═══════════════════════════════════════════════════════════

/// Text-tier tokens after diagnosis. Rejected numeric bullets remain available
/// for explicit shared-model recovery; a diagnostic never authorizes omission.
pub(crate) struct DiagnosedTextTierTokens<'source>(Vec<Token<'source>>);

impl<'source> DiagnosedTextTierTokens<'source> {
    pub(crate) fn admit(
        selected: super::LocatedTokens<'_, 'source>,
        errors: &impl talkbank_model::ErrorSink,
    ) -> talkbank_model::ParseOutcome<Self> {
        use talkbank_model::ParseOutcome;
        let mut admitted = ParseOutcome::Parsed(Vec::new());
        let source = selected.source();
        for (token, span) in selected.into_located() {
            if let Some(context) = token.error_context() {
                admitted = ParseOutcome::Rejected;
                errors.report(talkbank_model::ParseError::new(
                    talkbank_model::ErrorCode::UnparsableContent,
                    talkbank_model::Severity::Error,
                    talkbank_model::DiagnosticLocation::from_offsets(span.start, span.end),
                    talkbank_model::ErrorContext::new(
                        source,
                        talkbank_model::Span::new(span.start, span.end),
                        "dependent_content",
                    ),
                    context,
                ));
            } else if let Token::MediaBullet { bullet } = token
                && let Some(message) = match bullet.times().map(|times| times.pair()) {
                    Some((0, 0)) => Some("Invalid bullet: both start and end timestamps are 0"),
                    None => Some("Invalid bullet: timestamp cannot be represented as milliseconds"),
                    _ => None,
                }
            {
                errors.report(talkbank_model::ParseError::new(
                    talkbank_model::ErrorCode::InvalidMediaBullet,
                    talkbank_model::Severity::Error,
                    talkbank_model::DiagnosticLocation::from_offsets(span.start, span.end),
                    talkbank_model::ErrorContext::new(
                        source,
                        talkbank_model::Span::new(span.start, span.end),
                        "",
                    ),
                    message,
                ));
            }
            if let ParseOutcome::Parsed(tokens) = &mut admitted {
                tokens.push(token.clone());
            }
        }
        admitted.map(Self)
    }

    pub(crate) fn tokens(&self) -> &[Token<'source>] {
        &self.0
    }

    pub(crate) fn into_tokens(self) -> Vec<Token<'source>> {
        self.0
    }
}

/// Recognize one text body, retaining its original tokens for shared lowering.
pub fn text_tier_parser<'tokens, 'a: 'tokens>()
-> impl Parser<'tokens, Tokens<'tokens, 'a>, &'tokens [Token<'a>]> + Clone {
    let segment = select! {
        Token::TextSegment(_) | Token::MediaBullet { .. }
        | Token::InlinePic(_) | Token::Continuation(_) => (),
    };

    // Continuations are body segments, not bare line endings. Only one ending
    // can follow the complete body; further newlines cannot disappear into it.
    let ending = select! { Token::Newline(_) => () }.or_not();

    segment.repeated().to_slice().then_ignore(ending)
}

// ═══════════════════════════════════════════════════════════
// %wor tier, words with optional inline timing bullets
// ═══════════════════════════════════════════════════════════

/// Keep the timing token, admitted or rejected, associated with its source
/// word; shared-model construction places it at the token's own bytes.
fn timing_bullet<'tokens, 'a: 'tokens>()
-> impl Parser<'tokens, Tokens<'tokens, 'a>, crate::token::MediaBulletToken<'a>> + Clone {
    select! {
        Token::MediaBullet { bullet } => bullet,
    }
}

/// Parse a `%wor` tier body: words with optional timing bullets.
pub fn wor_tier_parser<'tokens, 'a: 'tokens>(
    source: crate::source_text::SourceText<'a>,
    location: talkbank_model::Span,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, talkbank_model::WorTier> + Clone {
    use talkbank_model::model::dependent_tier::wor::WorItem;
    let terminator = main_tier::terminator(source);

    // `%wor` may open with a language precode, exactly as the main tier may:
    // `grammar.js`'s `wor_tier_body` declares an optional `language_code`
    // field, and real transcripts use it heavily (`%wor:\t[- zho] ...` in the
    // Chinese and bilingual corpora). Without this the whole tier failed to
    // parse: 510 E316 across 8 of 2,158 sampled files, every one of which
    // tree-sitter reads cleanly. Same shape as `main_tier::tier_body_parser`.
    let langcode = main_tier::language_precode(source);

    let separator = main_tier::separator(source).map(|separator| {
        use talkbank_model::WriteChat;
        WorItem::Separator {
            text: separator.to_chat_string(),
            location: separator.location(),
        }
    });

    // Standalone syntax cannot consume or erase main-tier annotations.
    let word_with_bullet = main_tier::standalone_word(source)
        .then(ws().ignore_then(timing_bullet()).or_not())
        .try_map(move |(mut word, bullet), _| {
            if let Some(bullet) = bullet {
                let model = crate::convert::bullet_from_token(bullet, source)
                    .into_parsed()
                    .map_err(|_| chumsky::error::EmptyErr::default())?;
                word = word.with_inline_bullet(model);
            }
            Ok(WorItem::Word(Box::new(word)))
        });

    // Unassociated timing rejects the tier so file recovery retains the source.
    let wor_item = choice((word_with_bullet, separator));

    ws().ignore_then(langcode.or_not())
        .then(wor_item.padded_by(ws()).repeated().collect::<Vec<_>>())
        .then(ws().ignore_then(terminator).or_not())
        .then_ignore(ws())
        .then_ignore(opt_newline())
        .map(move |((langcode, items), terminator)| {
            talkbank_model::WorTier::new(location, items)
                .with_language_code(langcode.map(|precode| precode.code))
                .with_terminator(terminator)
        })
}
