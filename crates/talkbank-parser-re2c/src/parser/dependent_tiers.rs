//! Chumsky parser combinators for dependent tier parsing.
//!
//! Each parser operates on `&[Token<'a>]` (span-stripped token stream) and
//! produces the corresponding AST type from `ast.rs`.
//!
//! Tier parsers: %mor, %gra, %pho, %sin, text tiers.
// Design rule 3, enforced by the compiler rather than by prose: a `_` arm over
// a content enum means a future variant compiles clean and answers wrong.
// Added per file as each is cleaned; `content_catch_alls` lists the rest.
#![deny(clippy::wildcard_enum_match_arm)]

use chumsky::prelude::*;

use crate::ast::*;
use crate::token::Token;

use crate::token::TokenDiscriminants;

use super::classify::is_terminator;
use super::main_tier;

/// Chumsky input type: a slice of tokens borrowed from the lexer output.
type Tokens<'a> = &'a [Token<'a>];

/// Whitespace/continuation combinator, skips structural whitespace tokens.
pub(super) fn ws<'a>() -> impl Parser<'a, Tokens<'a>, ()> + Clone {
    select! { Token::Whitespace(_) => (), Token::Continuation(_) => () }
        .repeated()
        .ignored()
}

/// Optional trailing newline, consumes a Newline token if present.
pub(super) fn opt_newline<'a>() -> impl Parser<'a, Tokens<'a>, ()> + Clone {
    select! { Token::Newline(_) => () }.or_not().ignored()
}

// ═══════════════════════════════════════════════════════════
// %gra tier
// grammar.js: gra_contents = seq(gra_relation+)
// ═══════════════════════════════════════════════════════════

/// Parse a `%gra` tier body: whitespace-separated `GraRelation` tokens.
pub fn gra_tier_parser<'a>() -> impl Parser<'a, Tokens<'a>, GraTier<'a>> + Clone {
    let relation = select! {
        Token::GraRelation { index, head, relation } => GraRelationParsed {
            index, head, relation,
        },
    };

    ws().ignore_then(
        relation
            .separated_by(ws())
            .allow_trailing()
            .collect::<Vec<_>>(),
    )
    .then_ignore(ws())
    .then_ignore(opt_newline())
    .map(|relations| GraTier { relations })
}

// ═══════════════════════════════════════════════════════════
// %mor tier
// grammar.js: mor_contents = seq(mor_content+, optional(terminator))
// grammar.js: mor_content = seq(mor_word, repeat(seq(tilde, mor_word)))
// ═══════════════════════════════════════════════════════════

/// Parse a single `MorWord` token into `MorWordParsed`.
pub fn mor_word_parser<'a>() -> impl Parser<'a, Tokens<'a>, MorWordParsed<'a>> + Clone {
    select! {
        Token::MorWord { pos, lemma_features } => {
            let mut parts = lemma_features.splitn(2, '-');
            let lemma = parts.next().unwrap_or("");
            let features: Vec<&str> = match parts.next() {
                Some(feat_str) => feat_str.split('-').collect(),
                None => vec![],
            };
            MorWordParsed { pos, lemma, features }
        },
    }
}

/// Parse a `%mor` item: main word + optional post-clitics (~word~word).
fn mor_item_parser<'a>() -> impl Parser<'a, Tokens<'a>, MorItem<'a>> + Clone {
    let tilde = select! { Token::MorTilde(_) => () };
    let clitic = tilde.ignore_then(mor_word_parser());

    mor_word_parser()
        .then(clitic.repeated().collect::<Vec<_>>())
        .map(|(main, post_clitics)| MorItem { main, post_clitics })
}

/// Parse a `%mor` tier body: mor items, optional terminator.
pub fn mor_tier_parser<'a>() -> impl Parser<'a, Tokens<'a>, MorTier<'a>> + Clone {
    let terminator = select! {
        tok if is_terminator(Some(TokenDiscriminants::from(&tok))) => tok,
    };

    ws().ignore_then(
        mor_item_parser()
            .separated_by(ws())
            .allow_trailing()
            .collect::<Vec<_>>(),
    )
    .then(ws().ignore_then(terminator).or_not())
    .then_ignore(ws())
    .then_ignore(opt_newline())
    .map(|(items, terminator)| MorTier { items, terminator })
}

// ═══════════════════════════════════════════════════════════
// %pho tier
// ═══════════════════════════════════════════════════════════

/// Parse a single phonological word (possibly compound with +).
fn pho_word_parser<'a>() -> impl Parser<'a, Tokens<'a>, PhoWordParsed<'a>> + Clone {
    let pho_word = select! { Token::PhoWord(s) => s };
    let plus = select! { Token::PhoPlus(_) => () };

    pho_word
        .then(plus.ignore_then(pho_word).repeated().collect::<Vec<_>>())
        .map(|(first, rest)| {
            let mut segments = vec![first];
            segments.extend(rest);
            PhoWordParsed { segments }
        })
}

/// Parse a `%pho` tier body.
pub fn pho_tier_parser<'a>() -> impl Parser<'a, Tokens<'a>, PhoTier<'a>> + Clone {
    let terminator = select! {
        tok if is_terminator(Some(TokenDiscriminants::from(&tok))) => tok,
    };

    // Pauses are skipped in %pho
    let pause = select! {
        Token::PauseLong(_) => (),
        Token::PauseMedium(_) => (),
        Token::PauseShort(_) => (),
    };

    let group_begin = select! { Token::PhoGroupBegin(_) => () };
    let group_end = select! { Token::PhoGroupEnd(_) => () };

    let pho_group = group_begin
        .ignore_then(
            pho_word_parser()
                .padded_by(ws())
                .repeated()
                .collect::<Vec<_>>(),
        )
        .then_ignore(group_end)
        .map(PhoItemParsed::Group);

    let pho_item = choice((
        pho_group,
        pho_word_parser().map(PhoItemParsed::Word),
        pause.to(PhoItemParsed::Word(PhoWordParsed { segments: vec![] })),
    ));

    ws().ignore_then(pho_item.padded_by(ws()).repeated().collect::<Vec<_>>())
        .then(ws().ignore_then(terminator).or_not())
        .then_ignore(ws())
        .then_ignore(opt_newline())
        .map(|(items, terminator): (Vec<PhoItemParsed<'_>>, _)| {
            // Filter out empty words from skipped pauses
            let items = items
                .into_iter()
                .filter(|item| !matches!(item, PhoItemParsed::Word(w) if w.segments.is_empty()))
                .collect();
            PhoTier { items, terminator }
        })
}

// ═══════════════════════════════════════════════════════════
// %sin tier
// ═══════════════════════════════════════════════════════════

/// Parse a `%sin` tier body.
pub fn sin_tier_parser<'a>() -> impl Parser<'a, Tokens<'a>, SinTierParsed<'a>> + Clone {
    let sin_word = select! {
        Token::SinWord(s) => s,
        Token::Zero(s) => s,
    };

    let group_begin = select! { Token::SinGroupBegin(_) => () };
    let group_end = select! { Token::SinGroupEnd(_) => () };

    let sin_group = group_begin
        .ignore_then(sin_word.padded_by(ws()).repeated().collect::<Vec<_>>())
        .then_ignore(group_end)
        .map(SinItemParsed::Group);

    let sin_item = choice((sin_group, sin_word.map(SinItemParsed::Token)));

    ws().ignore_then(sin_item.padded_by(ws()).repeated().collect::<Vec<_>>())
        .then_ignore(opt_newline())
        .map(|items| SinTierParsed { items })
}

// ═══════════════════════════════════════════════════════════
// Text tier (for %com, %act, %cod, %exp, etc.)
// ═══════════════════════════════════════════════════════════

/// Parse a text tier body (text_with_bullets).
pub fn text_tier_parser<'a>() -> impl Parser<'a, Tokens<'a>, TextTierParsed<'a>> + Clone {
    let segment = select! {
        Token::TextSegment(s) => TextTierSegment::Text(s),
        tok @ Token::MediaBullet { .. } => TextTierSegment::Bullet(tok),
        tok @ Token::InlinePic(_) => TextTierSegment::Pic(tok),
        // Continuation is structural, skip
    };

    // Skip continuations between segments
    let skip_structural = select! {
        Token::Continuation(_) => (),
        Token::Newline(_) => (),
    };

    choice((segment.map(Some), skip_structural.to(None)))
        .repeated()
        .collect::<Vec<_>>()
        .map(|items| TextTierParsed {
            segments: items.into_iter().flatten().collect(),
        })
}

// ═══════════════════════════════════════════════════════════
// %wor tier, words with optional inline timing bullets
// ═══════════════════════════════════════════════════════════

/// Parse a timing bullet and extract (start_ms, end_ms).
fn timing_bullet<'a>() -> impl Parser<'a, Tokens<'a>, (u64, u64)> + Clone {
    select! {
        Token::MediaBullet { start_time, end_time, .. } => {
            let (s, e) = crate::convert::bullet_times(start_time, end_time);
            (s, e)
        },
    }
}

/// Parse a `%wor` tier body: words with optional timing bullets.
pub fn wor_tier_parser<'a>() -> impl Parser<'a, Tokens<'a>, WorTierParsed<'a>> + Clone {
    let terminator = select! {
        tok if is_terminator(Some(TokenDiscriminants::from(&tok))) => tok,
    };

    // `%wor` may open with a language precode, exactly as the main tier may:
    // `grammar.js`'s `wor_tier_body` declares an optional `language_code`
    // field, and real transcripts use it heavily (`%wor:\t[- zho] ...` in the
    // Chinese and bilingual corpora). Without this the whole tier failed to
    // parse: 510 E316 across 8 of 2,158 sampled files, every one of which
    // tree-sitter reads cleanly. Same shape as `main_tier::tier_body_parser`.
    let langcode = select! { tok @ Token::Langcode(_) => tok }.then_ignore(ws());

    // Same owner as the main tier: `SeparatorKindParsed::from_token`. This used
    // to gate on `classify::is_separator`, a second copy of the same fifteen
    // tokens, whose drift would have surfaced as an E316 on valid CHAT.
    let separator = any().try_map(|tok: Token<'a>, _span| {
        SeparatorKindParsed::from_token(&tok)
            .map(WorItemParsed::Separator)
            .ok_or_else(Default::default)
    });

    /// A `%wor` entry with no word behind it: a placeholder, not a parsed word.
    fn placeholder_word(bullet: Option<(u64, u64)>) -> WorItemParsed<'static> {
        WorItemParsed::Word {
            word: WordWithAnnotations::default(),
            bullet,
        }
    }

    // A word (rich or legacy) followed by optional timing bullet
    let word_with_bullet = choice((main_tier::rich_word(), main_tier::subtoken_word()))
        .then(ws().ignore_then(timing_bullet()).or_not())
        .map(|(content_item, bullet)| {
            match content_item {
                ContentItem::Word(w) => WorItemParsed::Word { word: w, bullet },
                // If it parsed as a retrace or action, wrap the inner word
                ContentItem::Retrace(r) => {
                    if let Some(ContentItem::Word(w)) = r.content.into_iter().next() {
                        WorItemParsed::Word { word: w, bullet }
                    } else {
                        placeholder_word(bullet)
                    }
                }
                ContentItem::Action { zero, .. } => WorItemParsed::Word {
                    word: WordWithAnnotations {
                        category: Some(WordCategory::Omission),
                        raw_text: zero,
                        ..WordWithAnnotations::default()
                    },
                    bullet,
                },
                // Unreachable: `choice((rich_word(), subtoken_word()))` has range
                // `Word | Retrace | Action`, all handled above. Spelled out so a
                // widening of that range is a compile error here. The real fix is
                // narrowing what those parsers RETURN, which would delete this
                // list and `placeholder_word` with it; `classify.rs`'s `Chain` is
                // the in-repo precedent for exactly that.
                ContentItem::Pause(_)
                | ContentItem::Freecode(_)
                | ContentItem::OrphanAnnotation(_)
                | ContentItem::Separator { .. }
                | ContentItem::OverlapPoint { .. }
                | ContentItem::Group(_)
                | ContentItem::Quotation(_)
                | ContentItem::Event(_)
                | ContentItem::AnnotatedEvent { .. }
                | ContentItem::MediaBullet { .. }
                | ContentItem::UnderlineBegin
                | ContentItem::UnderlineEnd
                | ContentItem::OtherSpokenEvent { .. }
                | ContentItem::PhoGroup(_)
                | ContentItem::SinGroup(_)
                | ContentItem::LongFeatureBegin(_)
                | ContentItem::LongFeatureEnd(_)
                | ContentItem::NonvocalBegin(_)
                | ContentItem::NonvocalEnd(_)
                | ContentItem::NonvocalSimple(_) => placeholder_word(bullet),
            }
        });

    // Skip orphan bullets
    let skip_bullet = select! { Token::MediaBullet { .. } => () };

    // An orphan bullet yields no item at all. It used to yield a SEPARATOR
    // holding a whitespace token, a sentinel that was also a legal value, which
    // the collector then removed by matching that exact fake. Typing the
    // variant made that unrepresentable; `Option` says the same thing honestly.
    let wor_item = choice((
        word_with_bullet.map(Some),
        separator.map(Some),
        skip_bullet.to(None),
    ));

    ws().ignore_then(langcode.or_not())
        .then(wor_item.padded_by(ws()).repeated().collect::<Vec<_>>())
        .then(ws().ignore_then(terminator).or_not())
        .then_ignore(ws())
        .then_ignore(opt_newline())
        .map(|((langcode, items), terminator)| {
            let items: Vec<WorItemParsed<'a>> = items.into_iter().flatten().collect();
            WorTierParsed {
                langcode,
                items,
                terminator,
            }
        })
}
