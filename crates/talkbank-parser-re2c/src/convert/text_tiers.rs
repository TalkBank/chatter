//! Part of the AST→model conversion (see `mod.rs`); split out for file size.
//!
//! Clean of content-enum catch-alls since the CA-omission walk moved to
//! `talkbank-model` (the traversal now belongs to `walk_words_mut`), so this
//! file is off `UNPROTECTED`. `#![deny(clippy::wildcard_enum_match_arm)]` is
//! deliberately NOT applied: the remaining wildcard is over `Token`, where
//! enumerating ~180 variants buys nothing. Design rule 3 is about the CONTENT
//! enums, and the textual ratchet in `talkbank-parser-tests` holds that line.
#![allow(clippy::unreachable, clippy::unwrap_used, clippy::expect_used)]

use crate::ast;
use crate::token::Token;
use talkbank_model::model::*;

use super::*;

/// Error class for re2c → model conversion of `%mor:` tiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MorTierConvertError {
    /// AST has no terminator. `MorTier.terminator` is non-optional,
    /// so the caller must produce a typed parse-outcome diagnostic
    /// rather than constructing a MorTier.
    MissingTerminator,
    /// AST terminator string is not a recognized CHAT terminator.
    UnrecognizedTerminator(String),
}

impl<'a> TryFrom<&ast::MorTier<'a>> for MorTier {
    type Error = MorTierConvertError;

    fn try_from(tier: &ast::MorTier<'a>) -> Result<Self, Self::Error> {
        use talkbank_model::Terminator;

        let items: Vec<Mor> = tier.items.iter().map(Mor::from).collect();
        let terminator_node = tier
            .terminator
            .as_ref()
            .ok_or(MorTierConvertError::MissingTerminator)?;
        let terminator =
            Terminator::try_from_chat_str(terminator_node.text().trim()).ok_or_else(|| {
                MorTierConvertError::UnrecognizedTerminator(terminator_node.text().to_string())
            })?;
        Ok(MorTier::new_mor(items, terminator))
    }
}

// ═══════════════════════════════════════════════════════════════
// %gra conversions
// ═══════════════════════════════════════════════════════════════

impl<'a> From<&ast::GraRelationParsed<'a>> for GrammaticalRelation {
    fn from(r: &ast::GraRelationParsed<'a>) -> Self {
        GrammaticalRelation {
            index: r.index.parse().unwrap_or(0),
            head: r.head.parse().unwrap_or(0),
            relation: GrammaticalRelationType::new(r.relation),
        }
    }
}

impl<'a> From<&ast::GraTier<'a>> for GraTier {
    fn from(tier: &ast::GraTier<'a>) -> Self {
        let relations: Vec<GrammaticalRelation> = tier
            .relations
            .iter()
            .map(GrammaticalRelation::from)
            .collect();
        GraTier::new_gra(relations)
    }
}

// ═══════════════════════════════════════════════════════════════
// @Languages conversion
// ═══════════════════════════════════════════════════════════════

impl<'a> From<&ast::LanguagesHeaderParsed<'a>> for LanguageCodes {
    fn from(langs: &ast::LanguagesHeaderParsed<'a>) -> Self {
        // Each AST code is lexed via the `language_code` token rule
        // (guaranteed non-empty, mirrors the tree-sitter grammar's
        // `/[a-z]{2,4}/`), so `.expect()` is defensive only.
        LanguageCodes::new(
            langs
                .codes
                .iter()
                .map(|c| LanguageCode::new(*c).expect("lexer-guaranteed non-empty code"))
                .collect(),
        )
    }
}

// ═══════════════════════════════════════════════════════════════
// PhoTier conversion
// ═══════════════════════════════════════════════════════════════

/// Convert our parsed PhoTier to model PhoTier.
pub(crate) fn convert_pho_tier(
    pho: &ast::PhoTier<'_>,
    tier_type: talkbank_model::model::dependent_tier::pho::PhoTierType,
) -> talkbank_model::model::PhoTier {
    use talkbank_model::model::dependent_tier::pho::{PhoGroupWords, PhoItem, PhoWord};

    fn pho_word_to_model(w: &ast::PhoWordParsed<'_>) -> PhoWord {
        // Compound words: segments joined by +. Model stores full text.
        PhoWord::new(w.segments.join("+"))
    }

    let items: Vec<PhoItem> = pho
        .items
        .iter()
        .map(|item| match item {
            ast::PhoItemParsed::Word(w) => PhoItem::Word(pho_word_to_model(w)),
            ast::PhoItemParsed::Group(words) => PhoItem::Group(PhoGroupWords::new(
                words.iter().map(pho_word_to_model).collect(),
            )),
        })
        .collect();
    talkbank_model::model::PhoTier::new(tier_type, items)
}

/// Convert our parsed SinTier to model SinTier.
pub(crate) fn convert_sin_tier(sin: &ast::SinTierParsed<'_>) -> talkbank_model::model::SinTier {
    use talkbank_model::model::dependent_tier::sin::{SinGroupGestures, SinItem, SinToken};
    let items: Vec<SinItem> = sin
        .items
        .iter()
        .map(|item| match item {
            ast::SinItemParsed::Token(s) => SinItem::Token(SinToken::new_unchecked(s)),
            ast::SinItemParsed::Group(words) => SinItem::SinGroup(SinGroupGestures::new(
                words.iter().map(SinToken::new_unchecked).collect(),
            )),
        })
        .collect();
    talkbank_model::model::SinTier::new(items)
}

// ═══════════════════════════════════════════════════════════════
// Public aliases and missing conversion functions
// (required by chat_parser_impl.rs for ChatParser trait)
// ═══════════════════════════════════════════════════════════════

/// Alias for `header_to_model`, used by ChatParser trait impl.
pub fn header_parsed_to_model(h: &ast::HeaderParsed<'_>) -> Header {
    header_to_model(h)
}

/// Convert text tier parsed AST to BulletContent.
pub(crate) fn text_tier_to_bullet_content(parsed: &ast::TextTierParsed<'_>) -> BulletContent {
    let segments: Vec<BulletContentSegment> = parsed
        .segments
        .iter()
        .map(|seg| match seg {
            ast::TextTierSegment::Text(s) => BulletContentSegment::text(*s),
            ast::TextTierSegment::Bullet(tok) => match tok {
                Token::MediaBullet {
                    start_time,
                    end_time,
                    ..
                } => {
                    let (s, e) = super::items::bullet_times(start_time, end_time);
                    BulletContentSegment::bullet(s, e)
                }
                _ => BulletContentSegment::text(tok.text()),
            },
            ast::TextTierSegment::Pic(tok) => BulletContentSegment::picture(tok.text()),
        })
        .collect();
    BulletContent::new(segments)
}

/// Convert parsed text tier to model ActTier.
pub fn to_act_tier(parsed: &ast::TextTierParsed<'_>) -> ActTier {
    ActTier::new(text_tier_to_bullet_content(parsed))
}

/// Convert parsed text tier to model CodTier.
pub fn to_cod_tier(parsed: &ast::TextTierParsed<'_>) -> CodTier {
    CodTier::new(text_tier_to_bullet_content(parsed))
}

/// Convert parsed text tier to model ComTier.
pub fn to_com_tier(parsed: &ast::TextTierParsed<'_>) -> ComTier {
    ComTier::new(text_tier_to_bullet_content(parsed))
}

/// Convert parsed text tier to model ExpTier.
pub fn to_exp_tier(parsed: &ast::TextTierParsed<'_>) -> ExpTier {
    ExpTier::new(text_tier_to_bullet_content(parsed))
}

/// Convert parsed text tier to model AddTier.
pub fn to_add_tier(parsed: &ast::TextTierParsed<'_>) -> AddTier {
    AddTier::new(text_tier_to_bullet_content(parsed))
}

/// Convert parsed text tier to model GpxTier.
pub fn to_gpx_tier(parsed: &ast::TextTierParsed<'_>) -> GpxTier {
    GpxTier::new(text_tier_to_bullet_content(parsed))
}

/// Convert parsed text tier to model IntTier.
pub fn to_int_tier(parsed: &ast::TextTierParsed<'_>) -> IntTier {
    IntTier::new(text_tier_to_bullet_content(parsed))
}

/// Convert parsed text tier to model SpaTier.
pub fn to_spa_tier(parsed: &ast::TextTierParsed<'_>) -> SpaTier {
    SpaTier::new(text_tier_to_bullet_content(parsed))
}

/// Convert parsed text tier to model SitTier.
pub fn to_sit_tier(parsed: &ast::TextTierParsed<'_>) -> SitTier {
    SitTier::new(text_tier_to_bullet_content(parsed))
}

/// Parse %sin tier content and convert to model SinTier.
pub fn sin_tier_from_text(input: &str) -> talkbank_model::model::SinTier {
    use talkbank_model::model::dependent_tier::sin::{SinGroupGestures, SinItem, SinToken};
    // Simple word-based parsing: split on whitespace, handle 〔groups〕
    let mut items = Vec::new();
    let mut in_group = false;
    let mut group_words = Vec::new();
    for word in input.split_whitespace() {
        if word.starts_with('\u{3014}') {
            // 〔 group start
            in_group = true;
            let text = word.trim_start_matches('\u{3014}');
            if !text.is_empty() {
                group_words.push(SinToken::new_unchecked(text));
            }
        } else if word.ends_with('\u{3015}') {
            // 〕 group end
            let text = word.trim_end_matches('\u{3015}');
            if !text.is_empty() {
                group_words.push(SinToken::new_unchecked(text));
            }
            items.push(SinItem::SinGroup(SinGroupGestures::new(std::mem::take(
                &mut group_words,
            ))));
            in_group = false;
        } else if in_group {
            group_words.push(SinToken::new_unchecked(word));
        } else {
            items.push(SinItem::Token(SinToken::new_unchecked(word)));
        }
    }
    talkbank_model::model::SinTier::new(items)
}

/// Parse `%wor` tier content and convert to a model `WorTier`.
///
/// `None` when the tier does not parse, so the caller can reject it. It used to
/// return a bare `WorTier`, substituting an EMPTY one on failure, which
/// reported an unparsable tier as a successfully parsed tier with no words.
/// The seam to say otherwise already existed at both call sites: one holds an
/// `ErrorSink` it was ignoring, the other an outcome type with a `rejected`
/// variant. Matching the `Option`-then-`rejected` shape the three sibling
/// entry points in `chat_parser_impl` already use.
///
/// Delegates to `wor_tier_parser`, the same parser the file-level path uses.
/// It used to be a SECOND implementation, parsing with the MAIN-TIER
/// `contents_parser` and keeping only bare words and separators from a flat
/// loop, so it disagreed with the real one about timing bullets, language
/// precodes and terminators. Two parsers for one tier is a divergence with
/// nothing holding it shut, and this one was the fallback the other fell back
/// TO, so both had to fail before anyone saw a difference.
pub fn wor_tier_from_input(input: &str) -> Option<WorTier> {
    use chumsky::Parser as _;

    let tokens = crate::parser::lex_to_tokens(input, crate::lexer::COND_MAIN_CONTENT);
    crate::parser::dependent_tiers::wor_tier_parser()
        .parse(tokens)
        .into_result()
        .ok()
        .map(|parsed| crate::convert::tiers::wor_tier_to_model(&parsed))
}

// From impls that are now possible (no source needed)

impl<'a> From<&ast::MainTier<'a>> for MainTier {
    fn from(mt: &ast::MainTier<'a>) -> Self {
        main_tier_to_model(mt)
    }
}

impl<'a> From<&ast::Utterance<'a>> for talkbank_model::model::Utterance {
    fn from(u: &ast::Utterance<'a>) -> Self {
        utterance_to_model(u)
    }
}

impl<'a> From<&ast::WordWithAnnotations<'a>> for Word {
    fn from(w: &ast::WordWithAnnotations<'a>) -> Self {
        word_from_parsed(w)
    }
}

impl<'a> From<&ast::IdHeaderParsed<'a>> for IDHeader {
    fn from(id: &ast::IdHeaderParsed<'a>) -> Self {
        // Filter empty pieces (e.g. a malformed "eng,,ara") before
        // constructing, mirroring the canonical tree-sitter side's
        // `id/parse.rs` guard, so a filtered-non-empty `.expect()` is
        // provably safe rather than reachable on malformed input.
        let lang_codes: Vec<LanguageCode> = id
            .language
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| LanguageCode::new(s).expect("filtered non-empty by the preceding filter"))
            .collect();
        let mut header = IDHeader::from_languages(
            LanguageCodes::new(lang_codes),
            SpeakerCode::new(id.speaker),
            ParticipantRole::new(id.role),
        );
        if !id.corpus.is_empty() {
            header = header.with_corpus(id.corpus);
        }
        if !id.age.is_empty() {
            header = header.with_age(id.age);
        }
        if !id.group.is_empty() {
            header = header.with_group(id.group);
        }
        if !id.ses.is_empty() {
            header = header.with_ses(id.ses);
        }
        if !id.education.is_empty() {
            header = header.with_education(id.education);
        }
        if !id.custom_field.is_empty() {
            header = header.with_custom_field(id.custom_field);
        }
        if !id.sex.is_empty() {
            header = header.with_sex(talkbank_model::model::Sex::from_text(id.sex));
        }
        header
    }
}

impl<'a> From<&ast::ParticipantEntryParsed<'a>> for ParticipantEntry {
    fn from(entry: &ast::ParticipantEntryParsed<'a>) -> Self {
        participant_words_to_entry(&entry.words)
    }
}

impl<'a> From<&ast::PhoTier<'a>> for talkbank_model::model::PhoTier {
    fn from(pho: &ast::PhoTier<'a>) -> Self {
        convert_pho_tier(
            pho,
            talkbank_model::model::dependent_tier::pho::PhoTierType::Pho,
        )
    }
}

impl<'a> From<&ast::PhoWordParsed<'a>> for talkbank_model::model::PhoWord {
    fn from(w: &ast::PhoWordParsed<'a>) -> Self {
        talkbank_model::model::PhoWord::new(w.segments.join("+"))
    }
}

// ═══════════════════════════════════════════════════════════════
// CA character → type mapping
// ═══════════════════════════════════════════════════════════════
