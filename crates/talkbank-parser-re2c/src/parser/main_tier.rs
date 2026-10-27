//! Chumsky parser combinators for main tier content parsing.
//!
//! This is the critical module: it replaces the hand-written `parse_contents_until`,
//! `parse_rich_word_with_annotations`, `parse_group`, `parse_quotation`, and related
//! functions with declarative chumsky combinators.
//!
//! The contents parser is recursive (groups contain contents which contain groups).

use chumsky::prelude::*;
use talkbank_model::model::{OverlapPointKind, WordCategory};

use crate::ast::*;
use crate::source_text::SourceText;
use crate::token::{Token, TokenDiscriminants};

use super::classify::{linker_kind, token_to_parsed_annotation};
use super::dependent_tiers::{opt_newline, ws};
use super::word_body::parse_word_body;
use super::word_language::admit_word_language;
use crate::ast::chain::BaseAnnotation;

/// Chumsky input type.
use super::Tokens;

// ═══════════════════════════════════════════════════════════
// Annotation parsing, shared combinator replacing 4 duplicate loops
// ═══════════════════════════════════════════════════════════

/// Parse an annotation, including structured replacement-word sequences.
fn annotation<'tokens, 'a: 'tokens>(
    source: SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, ParsedAnnotation<'a>> + Clone {
    replacement_annotation(source)
        .map(ParsedAnnotation::Replacement)
        .or(any().try_map(move |tok: Token<'a>, _span| {
            token_to_parsed_annotation(tok, source).ok_or_else(Default::default)
        }))
}

/// Parse replacement words through the same unannotated word productions used
/// by main-tier words. A missing word or closing bracket is a parser rejection.
fn replacement_annotation<'tokens, 'a: 'tokens>(
    source: SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, ReplacementParsed<'a>> + Clone {
    let word = unannotated_word(source);
    select! { Token::ReplacementBegin(begin) => begin }
        .then_ignore(ws())
        .then(word.clone())
        .then(ws().ignore_then(word).repeated().collect::<Vec<_>>())
        .then_ignore(ws())
        .then(select! { Token::RightBracket(end) => end })
        .try_map(move |(((begin, first), rest), end), _span| {
            ReplacementParsed::new(source, begin, first, rest, end).map_err(|_| Default::default())
        })
}

/// A separate bracket construct requires a source whitespace token.
fn annotation_gap<'tokens, 'a: 'tokens>() -> impl Parser<'tokens, Tokens<'tokens, 'a>, ()> + Clone {
    select! { Token::Whitespace(_) => (), Token::Continuation(_) => () }
        .repeated()
        .at_least(1)
        .ignored()
}

/// The grammar requires whitespace before each base annotation.
fn trailing_base_annotations<'tokens, 'a: 'tokens>(
    source: SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, Vec<BaseAnnotation<'a>>> + Clone {
    annotation_gap()
        .ignore_then(annotation(source).try_map(|annotation, _span| {
            BaseAnnotation::admit(annotation).ok_or_else(Default::default)
        }))
        .repeated()
        .collect()
}

// ═══════════════════════════════════════════════════════════
// Simple content items
// ═══════════════════════════════════════════════════════════

/// Admit a shared pause with the full extent already issued by the lexer.
fn pause<'tokens, 'a: 'tokens>()
-> impl Parser<'tokens, Tokens<'tokens, 'a>, ContentItem<'a>> + Clone {
    use talkbank_model::model::{Pause, PauseDuration, TimedPause};
    select! {
        Token::PauseLong(s) => Ok(ContentItem::Pause(Pause::new(s.span(), PauseDuration::Long))),
        Token::PauseMedium(s) => Ok(ContentItem::Pause(Pause::new(s.span(), PauseDuration::Medium))),
        Token::PauseShort(s) => Ok(ContentItem::Pause(Pause::new(s.span(), PauseDuration::Short))),
        Token::PauseTimed(s) => TimedPause::new(s.text())
            .map(|duration| ContentItem::Pause(Pause::new(s.span(), PauseDuration::Timed(duration)))),
    }.try_map(|pause, _| pause.map_err(|_| chumsky::error::EmptyErr::default()))
}

/// One separator grammar for main content and WOR, producing a located shared value.
pub(super) fn separator<'tokens, 'a: 'tokens>(
    source: SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, talkbank_model::model::Separator> + Clone {
    use talkbank_model::{Location, model::Separator};
    any().try_map(move |token: Token<'a>, _| {
        let location = Location::Located(
            source
                .span_of(token.text())
                .ok_or_else(chumsky::error::EmptyErr::default)?,
        );
        Ok(match token {
            Token::Comma(_) => Separator::Comma { location },
            Token::Semicolon(_) => Separator::Semicolon { location },
            Token::Colon(_) => Separator::Colon { location },
            Token::CaContinuationMarker(_) => Separator::CaContinuation { location },
            Token::TagMarker(_) => Separator::Tag { location },
            Token::VocativeMarker(_) => Separator::Vocative { location },
            Token::UnmarkedEnding(_) => Separator::UnmarkedEnding { location },
            Token::UptakeSymbol(_) => Separator::Uptake { location },
            Token::CaNoBreak(_) => Separator::CaNoBreak { location },
            Token::CaTechnicalBreak(_) => Separator::CaTechnicalBreak { location },
            Token::RisingToHigh(_) => Separator::RisingToHigh { location },
            Token::RisingToMid(_) => Separator::RisingToMid { location },
            Token::LevelPitch(_) => Separator::Level { location },
            Token::FallingToMid(_) => Separator::FallingToMid { location },
            Token::FallingToLow(_) => Separator::FallingToLow { location },
            _ => return Err(chumsky::error::EmptyErr::default()),
        })
    })
}

/// Parse an overlap point token.
fn overlap_point<'tokens, 'a: 'tokens>()
-> impl Parser<'tokens, Tokens<'tokens, 'a>, ContentItem<'a>> + Clone {
    select! {
        Token::OverlapTopBegin(s) => overlap_item(OverlapPointKind::TopOverlapBegin, s),
        Token::OverlapTopEnd(s) => overlap_item(OverlapPointKind::TopOverlapEnd, s),
        Token::OverlapBottomBegin(s) => overlap_item(OverlapPointKind::BottomOverlapBegin, s),
        Token::OverlapBottomEnd(s) => overlap_item(OverlapPointKind::BottomOverlapEnd, s),
    }
}

/// Parse structural marker tokens (underlines, long features, nonvocals).
fn structural_markers<'tokens, 'a: 'tokens>(
    source: SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, ContentItem<'a>> + Clone {
    // Each token kind fixes the marker meaning; its source lexeme supplies the
    // location, and its extracted label crosses the model's checked boundary.
    macro_rules! admit {
        ($text:ident, $variant:ident, $label:ident) => {
            source
                .span_of($text.lexeme())
                .ok_or_else(chumsky::error::EmptyErr::default)
                .and_then(|span| {
                    talkbank_model::$label::new($text.text())
                        .map(|label| {
                            ContentItem::$variant(talkbank_model::$variant::new(span, label))
                        })
                        .map_err(|_| chumsky::error::EmptyErr::default())
                })
        };
    }
    select! {
        Token::UnderlineBegin(s) => Ok(ContentItem::UnderlineBegin(s)),
        Token::UnderlineEnd(s) => Ok(ContentItem::UnderlineEnd(s)),
        Token::LongFeatureBegin(s) => admit!(s, LongFeatureBegin, LongFeatureLabel),
        Token::LongFeatureEnd(s) => admit!(s, LongFeatureEnd, LongFeatureLabel),
        Token::NonvocalBegin(s) => admit!(s, NonvocalBegin, NonvocalLabel),
        Token::NonvocalEnd(s) => admit!(s, NonvocalEnd, NonvocalLabel),
        Token::NonvocalSimple(s) => admit!(s, NonvocalSimple, NonvocalLabel),
    }
    .try_map(|result, _| result)
}

/// Parse a freecode annotation.
fn freecode<'tokens, 'a: 'tokens>()
-> impl Parser<'tokens, Tokens<'tokens, 'a>, ContentItem<'a>> + Clone {
    select! { Token::Freecode(s) => ContentItem::Freecode(s) }
}

/// Parse an other-spoken-event: &*SPK:word
fn other_spoken_event<'tokens, 'a: 'tokens>(
    source: SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, ContentItem<'a>> + Clone {
    select! { Token::OtherSpokenPrefix(prefix) => prefix }
        .then(unannotated_word(source))
        .try_map(move |(prefix, word), _| {
            let prefix_span = source
                .span_of(prefix.lexeme())
                .ok_or_else(Default::default)?;
            let word_span = word.source().span();
            if prefix_span.end() != word_span.start() {
                return Err(Default::default());
            }
            let location = talkbank_model::Span::new(prefix_span.start(), word_span.end());
            Ok(ContentItem::OtherSpokenEvent(
                talkbank_model::OtherSpokenEvent::new(
                    location,
                    prefix.text(),
                    crate::convert::word_from_parsed(&word),
                ),
            ))
        })
}

/// Parse inline media bullet (within content, not utterance-end).
fn inline_media_bullet<'tokens, 'a: 'tokens>()
-> impl Parser<'tokens, Tokens<'tokens, 'a>, ContentItem<'a>> + Clone {
    select! {
        Token::MediaBullet { bullet } => ContentItem::MediaBullet { bullet }
    }
}

/// Parse a bare annotation token as content.
///
/// An annotation with nothing to its left is not grammatical: `grammar.js`'s
/// `base_content_item` offers only `word_with_optional_annotations` and
/// `nonword_with_optional_annotations`, so an annotation is always ATTACHED.
/// This production exists solely so the re2c side can carry the invalid
/// construct far enough to report E759 with a useful message, which is what
/// the tree-sitter side does through its error nodes.
fn bare_annotation<'tokens, 'a: 'tokens>(
    source: SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, ContentItem<'a>> + Clone {
    annotation(source).map(ContentItem::OrphanAnnotation)
}

// ═══════════════════════════════════════════════════════════
// Rich word parser (Token::Word from lexer)
// ═══════════════════════════════════════════════════════════

/// Parse a rich `Token::Word` into a `ContentItem` (Word or Retrace).
pub fn rich_word<'tokens, 'a: 'tokens>(
    source: SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, ContentItem<'a>> + Clone {
    unannotated_word(source)
        .then(
            annotation_gap()
                .ignore_then(replacement_annotation(source))
                .or_not(),
        )
        .then(trailing_base_annotations(source))
        .map(|((mut word, replacement), annotations)| {
            word.replacement = replacement;
            Chain::Word(word).fold(annotations).into_content_item()
        })
}

/// Lower standalone word syntax directly to a shared word for WOR.
/// Unconsumed annotations reject the containing tier instead of disappearing.
pub(super) fn standalone_word<'tokens, 'a: 'tokens>(
    source: SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, talkbank_model::Word> + Clone {
    unannotated_word(source).map(|word| crate::convert::word_from_parsed(&word))
}

/// Parse lexical word syntax only; suffix admission belongs to `rich_word`.
fn unannotated_word<'tokens, 'a: 'tokens>(
    source: SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, WordWithAnnotations<'a>> + Clone {
    select! {
        Token::Word { raw_text, prefix, body, form_marker, lang_suffix, pos_tag } =>
            (raw_text, prefix, body, form_marker, lang_suffix, pos_tag),
    }
    .try_map(
        move |(raw_text, prefix, body_str, form_marker, lang_suffix_opt, pos_tag), _| {
            let span = source.span_of(raw_text).ok_or_else(Default::default)?;
            let word_source = source
                .slice(span.to_range())
                .map_err(|_| Default::default())?;
            // A word has one @ suffix, not independent form and language slots.
            if form_marker.is_some() && lang_suffix_opt.is_some() {
                return Err(Default::default());
            }
            let form_marker = form_marker
                .map(|payload| {
                    talkbank_model::model::FormType::from_payload(
                        talkbank_model::model::FormMarkerPayload::after_at(payload),
                    )
                })
                .transpose()
                .map_err(|_| Default::default())?;
            let category = match prefix {
                Some("&-") => Some(WordCategory::Filler),
                Some("&~") => Some(WordCategory::Nonword),
                Some("&+") => Some(WordCategory::PhonologicalFragment),
                Some("0") => Some(WordCategory::Omission),
                None => None,
                Some(_) => return Err(Default::default()),
            };
            let body = parse_word_body(body_str).map_err(|_| Default::default())?;
            let lang = lang_suffix_opt
                .map(|codes| admit_word_language(if codes.is_empty() { None } else { Some(codes) }))
                .transpose()
                .map_err(|_| Default::default())?;
            let word = WordWithAnnotations {
                category,
                body,
                form_marker,
                lang,
                pos_tag,
                replacement: None,
                annotations: Vec::new(),
                source: word_source,
            };
            Ok(word)
        },
    )
}

/// Parse the standalone zero action separately from lexical words.
/// Source admission precedes annotation folding; actions have no word fields.
fn zero_action<'tokens, 'a: 'tokens>(
    source: SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, ContentItem<'a>> + Clone {
    select! { Token::Zero(zero) => zero }
        .try_map(move |zero, _| {
            source
                .span_of(zero)
                .ok_or_else(Default::default)
                .map(|_| zero)
        })
        .then(trailing_base_annotations(source))
        .map(|(zero, annotations)| {
            Chain::Action(zero, Vec::new())
                .fold(annotations)
                .into_content_item()
        })
}

// ═══════════════════════════════════════════════════════════
// Event parser
// ═══════════════════════════════════════════════════════════

/// Parse an event with optional trailing annotations.
///
/// A retrace marker on an event used to be dropped here as "not semantically
/// valid", which was a judgement made in a lowering. The tree-sitter side folds
/// it (`content/marker_chain.rs`), so the two backends disagreed on
/// `I know it &=laughs [//] you were good .`, a shape attested six times in the
/// corpora. Lower it faithfully and let validation judge, which is the same
/// rule the retrace work settled on everywhere else.
fn event<'tokens, 'a: 'tokens>(
    source: SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, ContentItem<'a>> + Clone {
    select! { Token::Event(event) => event }
        .try_map(move |event, _| {
            let location = source
                .span_of(event.lexeme())
                .ok_or_else(Default::default)?;
            let event_type = talkbank_model::EventType::new(event.text())
                .map_err(|_| chumsky::error::EmptyErr::default())?;
            Ok(talkbank_model::Event::new(location, event_type))
        })
        .then(trailing_base_annotations(source))
        .map(|(event, annotations)| {
            Chain::Event(event, Vec::new())
                .fold(annotations)
                .into_content_item()
        })
}

// ═══════════════════════════════════════════════════════════
// Contents parser (recursive)
// ═══════════════════════════════════════════════════════════

/// Parser-local admission for a container's required material.
///
/// A tier body may be empty, but completing an enclosed construct may not use
/// that empty success. Only this checked constructor issues the value consumed
/// by the group/quotation builders; the native AST representation is unchanged.
struct EnclosedContents<'a>(Vec<ContentItem<'a>>);

impl<'a> EnclosedContents<'a> {
    fn new(items: Vec<ContentItem<'a>>) -> Option<Self> {
        (!items.is_empty()).then_some(Self(items))
    }

    fn into_items(self) -> Vec<ContentItem<'a>> {
        self.0
    }
}

/// Build the recursive contents parser.
///
/// Returns a parser that produces `Vec<ContentItem>`, the content items
/// of a tier body (main tier or inside groups/quotations).
pub fn contents_parser<'tokens, 'a: 'tokens>(
    source: SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, Vec<ContentItem<'a>>> + Clone {
    recursive(|contents| {
        let enclosed = contents
            .clone()
            .try_map(|items, _| EnclosedContents::new(items).ok_or_else(Default::default));
        // Group: < contents > annotations
        let group = select! { Token::LessThan(open) => open }
            .then(enclosed.clone())
            .then(select! { Token::GreaterThan(close) => close })
            .then(trailing_base_annotations(source))
            .map(
                |(((open, contents), close), annotations): (
                    ((&'a str, EnclosedContents<'a>), &'a str),
                    Vec<BaseAnnotation<'a>>,
                )| {
                    // Fold only annotations actually present in the token stream.
                    // With none, retain the written bare group; the sink-bearing
                    // dispatcher reports its missing annotation before lowering.
                    Chain::Group(Enclosure { open, close }, contents.into_items(), Vec::new())
                        .fold(annotations)
                        .into_content_item()
                },
            );

        // grammar.js: quotation_with_optional_annotations. The same ordered
        // chain owns scoped annotations and retraces for every seed kind.
        let quotation = select! { Token::LeftDoubleQuote(open) => open }
            .then(enclosed.clone())
            .then(select! { Token::RightDoubleQuote(close) => close })
            .then(trailing_base_annotations(source))
            .map(|(((open, contents), close), annotations)| {
                Chain::Quotation(Quotation {
                    enclosure: Enclosure { open, close },
                    contents: contents.into_items(),
                    annotations: Vec::new(),
                })
                .fold(annotations)
                .into_content_item()
            });

        // PhoGroup: ‹ contents ›
        let pho_group = select! { Token::PhoGroupBegin(open) => open }
            .then(enclosed.clone())
            .then(select! { Token::PhoGroupEnd(close) => close })
            .map(|((open, contents), close)| {
                ContentItem::PhoGroup(EnclosedGroup {
                    enclosure: Enclosure { open, close },
                    contents: contents.into_items(),
                })
            });

        // SinGroup: 〔 contents 〕
        let sin_group = select! { Token::SinGroupBegin(open) => open }
            .then(enclosed)
            .then(select! { Token::SinGroupEnd(close) => close })
            .map(|((open, contents), close)| {
                ContentItem::SinGroup(EnclosedGroup {
                    enclosure: Enclosure { open, close },
                    contents: contents.into_items(),
                })
            });

        // A single content item: one of many alternatives.
        // Order matters for chumsky's `choice`, more specific alternatives first.
        let content_item = choice((
            group,
            quotation,
            pho_group,
            sin_group,
            event(source),
            other_spoken_event(source),
            rich_word(source),
            overlap_point(),
            pause(),
            separator(source).map(ContentItem::Separator),
            zero_action(source),
            freecode(),
            structural_markers(source),
            inline_media_bullet(),
            bare_annotation(source),
        ));

        // Contents: one or more content items separated by whitespace
        content_item.padded_by(ws()).repeated().collect::<Vec<_>>()
    })
}

// ═══════════════════════════════════════════════════════════
// Terminator
// ═══════════════════════════════════════════════════════════

/// Admit a source-located shared terminator at the grammar boundary.
pub(super) fn terminator<'tokens, 'a: 'tokens>(
    source: SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, talkbank_model::Terminator> + Clone {
    any().try_map(move |token: Token<'a>, _| {
        crate::convert::token_to_terminator(&token, source).ok_or_else(Default::default)
    })
}

/// Parse postcodes: [+ code] tokens.
fn postcodes<'tokens, 'a: 'tokens>()
-> impl Parser<'tokens, Tokens<'tokens, 'a>, Vec<crate::token::PostcodeToken<'a>>> + Clone {
    select! { Token::Postcode(postcode) => postcode }
        .padded_by(ws())
        .repeated()
        .collect::<Vec<_>>()
}

// ═══════════════════════════════════════════════════════════
// Tier body and main tier
// ═══════════════════════════════════════════════════════════

/// Parse linkers: repeat1(linker whitespace*)
fn linkers<'tokens, 'a: 'tokens>(
    source: SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, Vec<talkbank_model::Linker>> + Clone {
    any()
        .try_map(move |tok: Token<'a>, _span| {
            linker_kind(TokenDiscriminants::from(&tok))
                .map(|kind| {
                    talkbank_model::Linker::new(crate::convert::locate(source, tok.text()), kind)
                })
                .ok_or_else(Default::default)
        })
        .then_ignore(ws())
        .repeated()
        .collect::<Vec<_>>()
}

/// Admit the shared language value and its original precode location together.
/// Both main and WOR tiers consume this transition; lowering never retries it.
pub(super) fn language_precode<'tokens, 'a: 'tokens>(
    source: SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, talkbank_model::LanguagePrecode> + Clone {
    select! { Token::Langcode(code) => code }
        .try_map(move |token, _| {
            let code = talkbank_model::LanguageCode::new(token.text())
                .map_err(|_| chumsky::error::EmptyErr::default())?;
            let location = source
                .span_of(token.lexeme())
                .ok_or_else(chumsky::error::EmptyErr::default)?;
            Ok(talkbank_model::LanguagePrecode::new(location, code))
        })
        .then_ignore(ws())
}

/// Parse a complete tier body.
///
/// grammar.js: tier_body = seq(
///   optional(linkers),
///   optional(seq(langcode, whitespaces)),
///   contents,
///   utterance_end
/// )
pub fn tier_body_parser<'tokens, 'a: 'tokens>(
    source: SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, TierBody<'a>> + Clone {
    let langcode = language_precode(source);

    // Utterance end: optional(terminator), optional(postcodes), optional(media_bullet), newline
    let media_bullet = select! { Token::MediaBullet { bullet } => bullet };

    linkers(source)
        .then(langcode.or_not())
        .then(contents_parser(source))
        .then(ws().ignore_then(terminator(source)).or_not())
        .then(postcodes())
        .then(ws().ignore_then(media_bullet).or_not())
        .then_ignore(ws())
        .then_ignore(opt_newline())
        .map(
            |(((((linkers, langcode), contents), terminator_tok), postcodes), media_bullet)| {
                TierBody {
                    linkers,
                    langcode,
                    contents,
                    terminator: terminator_tok,
                    postcodes,
                    media_bullet,
                }
            },
        )
}

/// Parse a complete main tier line.
///
/// grammar.js: main_tier = seq(star, speaker, colon, tab, tier_body)
pub fn main_tier_parser<'tokens, 'a: 'tokens>(
    source: SourceText<'a>,
) -> impl Parser<'tokens, Tokens<'tokens, 'a>, MainTier<'a>> + Clone {
    let star = select! { Token::Star(_) => () };
    let speaker = select! { tok @ Token::Speaker(_) => tok };
    let tier_sep = select! { Token::TierSep(prefix) => prefix.separator() };

    star.ignore_then(speaker)
        .then(tier_sep)
        .then(tier_body_parser(source))
        .map(|((speaker, separator), tier_body)| MainTier {
            speaker,
            separator,
            tier_body,
        })
}

/// Build an overlap content item, resolving the index digit once.
///
/// The "index is the second character" rule had been written three times in
/// the converter; recording it at construction leaves one owner.
fn overlap_item<'a>(kind: OverlapPointKind, raw: &'a str) -> ContentItem<'a> {
    ContentItem::OverlapPoint {
        kind,
        index: raw.chars().nth(1).and_then(|c| c.to_digit(10)),
        text: raw,
    }
}

#[cfg(test)]
mod base_annotation_boundary_tests {
    use super::*;

    #[test]
    fn language_precode_admission_keeps_location_and_refuses_invalid_evidence() {
        const REFERENCE: &str = include_str!("../../../../corpus/reference/tiers/wor.cha");
        let start = REFERENCE.find("[- zho]").unwrap();
        let spelling = &REFERENCE[start..start + "[- zho]".len()];
        let lexed = crate::parser::LexedSource::new(spelling, crate::lexer::COND_MAIN_CONTENT);
        let admitted = language_precode(SourceText::new(REFERENCE))
            .parse(lexed.tokens())
            .into_result()
            .unwrap();
        assert_eq!(admitted.code.as_str(), "zho");
        assert_eq!(
            admitted.location.span(),
            Some(talkbank_model::Span::new(start, start + spelling.len()))
        );

        let foreign = REFERENCE.to_owned();
        assert!(
            language_precode(SourceText::new(&foreign))
                .parse(lexed.tokens())
                .into_result()
                .is_err()
        );
        let malformed = [Token::Langcode(crate::token::Extracted::from_lexed(
            "", spelling,
        ))];
        assert!(
            language_precode(SourceText::new(REFERENCE))
                .parse(malformed.as_slice())
                .into_result()
                .is_err()
        );
    }

    #[test]
    fn rich_word_empty_shortening_is_rejected_at_admission() {
        for (body, accepted) in [("som(e)", true), ("som()", false)] {
            let tokens = [Token::Word {
                raw_text: body,
                prefix: None,
                body,
                form_marker: None,
                lang_suffix: None,
                pos_tag: None,
            }];
            assert_eq!(
                unannotated_word(SourceText::new(body))
                    .parse(tokens.as_slice())
                    .into_result()
                    .is_ok(),
                accepted
            );
        }
    }

    #[test]
    fn rich_word_source_admission_refuses_a_foreign_buffer() {
        // Reuse the shortening boundary's legal spelling with real lexer tokens.
        let original = "som(e)".to_owned();
        let foreign = original.clone();
        let lexed = crate::parser::LexedSource::new(&original, crate::lexer::COND_MAIN_CONTENT);
        assert!(
            unannotated_word(SourceText::new(&original))
                .parse(lexed.tokens())
                .into_result()
                .is_ok()
        );
        assert!(
            unannotated_word(SourceText::new(&foreign))
                .parse(lexed.tokens())
                .into_result()
                .is_err()
        );
    }

    #[test]
    fn unknown_rich_word_prefix_is_rejected_instead_of_becoming_absent() {
        for prefix in ["", "&?", "00", "unknown"] {
            let tokens = [Token::Word {
                raw_text: "word",
                prefix: Some(prefix),
                body: "word",
                form_marker: None,
                lang_suffix: None,
                pos_tag: None,
            }];
            assert!(
                unannotated_word(SourceText::new("word"))
                    .parse(tokens.as_slice())
                    .into_result()
                    .is_err()
            );
        }
    }

    /// Grammar admission is a boundary policy, not an invariant of all tokens.
    #[test]
    fn quotation_markers_exclude_word_and_tier_codes() {
        for annotation in [
            ParsedAnnotation::Replacement(
                crate::parser::parse_word("word [: replacement]")
                    .expect("replacement control")
                    .replacement
                    .expect("replacement annotation"),
            ),
            ParsedAnnotation::Langcode(crate::token::Extracted::from_lexed("eng", "[- eng]")),
            ParsedAnnotation::Postcode(crate::token::PostcodeToken::from_lexed("code", "[+ code]")),
            ParsedAnnotation::Scoped(ScopedAnnotationParsed::new(
                talkbank_model::ContentAnnotation::Unknown(talkbank_model::model::ScopedUnknown {
                    marker: "unknown".into(),
                    text: "".into(),
                }),
                SourceText::new("[unknown]").whole_slice(),
            )),
        ] {
            assert!(BaseAnnotation::admit(annotation).is_none());
        }
        for annotation in [
            ParsedAnnotation::Scoped(ScopedAnnotationParsed::new(
                talkbank_model::ContentAnnotation::Paralinguistic(
                    talkbank_model::model::ScopedParalinguistic {
                        text: "softly".into(),
                    },
                ),
                SourceText::new("[=! softly]").whole_slice(),
            )),
            ParsedAnnotation::Retrace(RetraceKindParsed::Complete, "[//]"),
        ] {
            assert!(BaseAnnotation::admit(annotation).is_some());
        }
    }
}
