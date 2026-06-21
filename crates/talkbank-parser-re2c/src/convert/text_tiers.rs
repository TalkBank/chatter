//! Part of the AST→model conversion (see `mod.rs`); split out for file size.
#![allow(clippy::unreachable, clippy::unwrap_used, clippy::expect_used)]

use crate::ast;
use crate::token::Token;
use talkbank_model::Span;
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
        LanguageCodes::new(langs.codes.iter().map(|c| LanguageCode::new(*c)).collect())
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
                    let s: u64 = start_time.parse().unwrap_or(0);
                    let e: u64 = end_time.parse().unwrap_or(0);
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

/// Parse %wor tier content and convert to model WorTier.
pub fn wor_tier_from_input(input: &str) -> WorTier {
    use chumsky::Parser as _;
    use talkbank_model::model::dependent_tier::wor::WorItem;

    // %wor uses same word rules as main tier. Parse words via chumsky.
    let tokens = crate::parser::lex_to_tokens(input, crate::lexer::COND_MAIN_CONTENT);
    let contents = crate::parser::main_tier::contents_parser()
        .parse(tokens)
        .into_result()
        .unwrap_or_default();

    let mut items = Vec::new();
    for item in &contents {
        match item {
            ast::ContentItem::Word(w) => {
                let word = word_from_parsed(w);
                items.push(WorItem::Word(Box::new(word)));
            }
            ast::ContentItem::Separator(tok) => {
                items.push(WorItem::Separator {
                    text: tok.text().to_string(),
                    span: Span::DUMMY,
                });
            }
            _ => {}
        }
    }
    WorTier::new(items)
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
        let lang_codes: Vec<LanguageCode> = id
            .language
            .split(',')
            .map(|s| LanguageCode::new(s.trim()))
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
// CA omission normalization (post-parse, context-dependent)
// ═══════════════════════════════════════════════════════════════

/// Normalize CA omission markers when @Options: CA is active.
/// A standalone (word), a word whose only content is a single Shortening
/// and has no category, is reclassified as CAOmission with Text content.
pub(crate) fn normalize_ca_omissions_in_lines(lines: &mut [talkbank_model::model::Line]) {
    for line in lines {
        if let talkbank_model::model::Line::Utterance(utterance) = line {
            for content in &mut utterance.main.content.content {
                normalize_ca_omission(content);
            }
        }
    }
}

pub(crate) fn normalize_ca_omission(content: &mut UtteranceContent) {
    match content {
        UtteranceContent::Word(word) => normalize_ca_omission_word(word),
        UtteranceContent::AnnotatedWord(annotated) => {
            normalize_ca_omission_word(&mut annotated.inner);
        }
        UtteranceContent::ReplacedWord(replaced) => {
            normalize_ca_omission_word(&mut replaced.word);
        }
        UtteranceContent::Group(group) => {
            for item in &mut group.content.content {
                normalize_ca_omission_bracketed_item(item);
            }
        }
        UtteranceContent::AnnotatedGroup(annotated) => {
            for item in &mut annotated.inner.content.content {
                normalize_ca_omission_bracketed_item(item);
            }
        }
        UtteranceContent::Retrace(retrace) => {
            for item in &mut retrace.content.content {
                normalize_ca_omission_bracketed_item(item);
            }
        }
        UtteranceContent::Quotation(quote) => {
            for item in &mut quote.content.content {
                normalize_ca_omission_bracketed_item(item);
            }
        }
        _ => {}
    }
}

pub(crate) fn normalize_ca_omission_bracketed_item(item: &mut BracketedItem) {
    match item {
        BracketedItem::Word(word) => normalize_ca_omission_word(word),
        BracketedItem::AnnotatedWord(annotated) => {
            normalize_ca_omission_word(&mut annotated.inner);
        }
        BracketedItem::ReplacedWord(replaced) => {
            normalize_ca_omission_word(&mut replaced.word);
        }
        _ => {}
    }
}

pub(crate) fn normalize_ca_omission_word(word: &mut Word) {
    // Only reclassify if no existing category and content is a single Shortening
    if word.category.is_some() {
        return;
    }
    if word.content.len() == 1
        && let WordContent::Shortening(shortening) = &word.content[0]
    {
        // Reclassify: Shortening → Text, category → CAOmission
        let text = shortening.as_ref().to_string();
        word.category = Some(WordCategory::CAOmission);
        word.content = WordContents::new(smallvec::smallvec![WordContent::Text(
            WordText::new_unchecked(&text)
        )]);
    }
}

// ═══════════════════════════════════════════════════════════════
// CA character → type mapping
// ═══════════════════════════════════════════════════════════════
