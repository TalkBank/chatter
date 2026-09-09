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
use crate::source_text::SourceText;
use crate::token::Token;
use talkbank_model::model::*;
use talkbank_model::{ErrorCode, ErrorSink, ParseError, ParseOutcome, Severity};

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

/// Lower one `%gra` relation, reporting what the model cannot hold.
///
/// # What this replaced, and why it is not a `From`
///
/// It was `From<&GraRelationParsed>`, infallible, writing
/// `r.index.parse().unwrap_or(0)` and `r.head.parse().unwrap_or(0)`. The lexer
/// admits any digit run for either field, so the only way `parse` fails is a
/// value too large for `usize` , and the answer to that was the number ZERO.
/// For the head, zero is the ROOT attachment: `2|99999999999999999999|PUNCT`
/// became `2|0|PUNCT`, so the oracle backend did not merely miss a diagnostic
/// on invalid input, it FABRICATED a well formed dependency tree from it, and
/// the parity harness could see only the missing diagnostic. Found 2026-09-07
/// by writing E710's first working example.
///
/// The three rejections mirror the canonical parser's, code for code, because
/// an oracle that rejects the same inputs for different reasons is not one:
/// E709 for a zero index, E708 for an index the model cannot hold, E710 for a
/// head it cannot hold. A rejected relation is DROPPED, never defaulted, which
/// is also what the canonical parser does.
///
/// Spans come from the `SourceText` the slices were cut from, so a diagnostic
/// points at the field rather than at a fabricated position.
pub fn gra_relation_to_model(
    r: &ast::GraRelationParsed<'_>,
    source: SourceText<'_>,
    errors: &(impl ErrorSink + ?Sized),
) -> ParseOutcome<GrammaticalRelation> {
    let index = match r.index.parse::<usize>() {
        Ok(0) => {
            report_gra(
                errors,
                source,
                r.index,
                ErrorCode::InvalidGrammarIndex,
                "Index cannot be 0 (indices are 1-indexed)",
            );
            return ParseOutcome::rejected();
        }
        Ok(index) => index,
        Err(_) => {
            report_gra(
                errors,
                source,
                r.index,
                ErrorCode::MalformedGrammarRelation,
                &format!("Invalid index '{}': must be a positive integer", r.index),
            );
            return ParseOutcome::rejected();
        }
    };
    let head = match r.head.parse::<usize>() {
        Ok(head) => head,
        Err(_) => {
            report_gra(
                errors,
                source,
                r.head,
                ErrorCode::UnexpectedGrammarNode,
                &format!("Invalid head '{}': must be a non-negative integer", r.head),
            );
            return ParseOutcome::rejected();
        }
    };
    ParseOutcome::parsed(GrammaticalRelation {
        index,
        head,
        relation: GrammaticalRelationType::new(r.relation),
    })
}

/// One diagnostic, located at the field that carries the fault.
///
/// A field whose span this source cannot resolve is reported at the tier's own
/// fallback rather than at `Span::DUMMY`: the sentinel is also the legal
/// zero-width position at byte 0, so it would place the diagnostic at the top
/// of the file and look like a fact.
fn report_gra(
    errors: &(impl ErrorSink + ?Sized),
    source: SourceText<'_>,
    field: &str,
    code: ErrorCode,
    message: &str,
) {
    // The slice came from this source by construction, so the fallback is
    // unreachable today. It is written out rather than unwrapped so that a
    // future caller pairing a relation with the wrong source loses the
    // POSITION and not the diagnostic, and it widens to the whole file rather
    // than collapsing to `Span::DUMMY`, which is also the legal zero-width
    // position at byte 0 and would put the finding at the top of the file as
    // though that were where it is.
    let at = match source.span_of(field) {
        Some(span) => span,
        None => source.whole(),
    };
    errors.report(ParseError::at_span(
        code,
        Severity::Error,
        at,
        message.to_owned(),
    ));
}

/// A lowered `%gra` tier, and what lowering it cost.
///
/// Returned rather than a bare [`GraTier`] because a tier that lost a relation
/// to a rejection is SHORTER than the one the author wrote, and nothing about
/// the resulting value says so. Every cross-tier check then blames the
/// transcript for our own recovery: `%mor` has three chunks and `%gra` has two,
/// so E720; the surviving relations are not 1..N, so E721; the only root may
/// be the one that was dropped, so E722.
///
/// Two things read the fact, and they are not the same question. The
/// [`GraCompleteness`] this carries onto the tier answers "did THIS tier lose
/// a relation", which is what `talkbank-model`'s whole-tier rules need. The
/// taint answers "is cross-tier alignment involving `%gra` trustworthy", which
/// the canonical backend also sets for faults in other tiers entirely.
///
/// Before this, the two backends disagreed about the consequence on
/// `spec/errors/E710.md` example 2: tree-sitter tainted and reported E600 and
/// a spurious E722, re2c recorded nothing and reported E720 against a
/// transcript whose two tiers agree.
#[must_use]
pub struct LoweredGra {
    tier: GraTier,
    recovered: Recovered,
}

/// Whether lowering had to drop something the author wrote.
#[derive(Clone, Copy)]
enum Recovered {
    /// Every relation on the line is in the tier.
    Nothing,
    /// At least one relation was rejected and is not in the tier.
    ADroppedRelation,
}

impl LoweredGra {
    /// The tier, recording any recovery on the utterance's parse health.
    ///
    /// The one route for a caller that HAS an utterance, which is every caller
    /// that lowers a whole file.
    pub fn into_tier(self, health: &mut ParseHealthState) -> GraTier {
        match self.recovered {
            Recovered::Nothing => {}
            Recovered::ADroppedRelation => health.taint(ParseHealthTier::Gra),
        }
        self.tier
    }

    /// The tier alone, for a caller with no utterance to record it on.
    ///
    /// The fragment entry points parse one tier out of context: there is no
    /// `ParseHealth` to taint and nothing downstream that could read it. Named
    /// so that dropping the fact is a decision visible at the call site rather
    /// than a field nobody looked at.
    pub fn tier_without_health(self) -> GraTier {
        self.tier
    }
}

/// Lower a `%gra` tier, dropping every relation the model cannot hold.
pub fn gra_tier_to_model(
    tier: &ast::GraTier<'_>,
    source: SourceText<'_>,
    errors: &(impl ErrorSink + ?Sized),
) -> LoweredGra {
    let mut relations = Vec::with_capacity(tier.relations.len());
    for parsed in &tier.relations {
        if let ParseOutcome::Parsed(relation) = gra_relation_to_model(parsed, source, errors) {
            relations.push(relation);
        }
    }
    // Compared against what the LINE held, not against a count carried
    // alongside: the two cannot disagree.
    let (recovered, completeness) = match relations.len() == tier.relations.len() {
        true => (Recovered::Nothing, GraCompleteness::Whole),
        false => (Recovered::ADroppedRelation, GraCompleteness::Truncated),
    };
    LoweredGra {
        // The fact is recorded TWICE, on purpose, because two different
        // questions read it. `GraCompleteness` travels on the tier and answers
        // "did this tier lose a relation", which is what the structural rules
        // need. The taint below travels on the utterance and answers "is
        // cross-tier alignment involving `%gra` trustworthy", which is coarser
        // and is set for reasons that have nothing to do with this tier.
        tier: GraTier::lowered_from(relations, completeness),
        recovered,
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
pub(crate) fn convert_sin_tier(sin: &ast::SinTierParsed) -> talkbank_model::model::SinTier {
    use talkbank_model::model::dependent_tier::sin::{SinGroupGestures, SinItem};
    let items: Vec<SinItem> = sin
        .items
        .iter()
        .map(|item| match item {
            ast::SinItemParsed::Token(s) => SinItem::Token(s.clone()),
            ast::SinItemParsed::Group(words) => {
                SinItem::SinGroup(SinGroupGestures::new(words.clone()))
            }
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

/// Parse a `%sin` fragment through the same grammar used for whole files.
/// Malformed groups are rejected rather than silently discarded.
pub fn sin_tier_from_text(
    input: &str,
) -> talkbank_model::ParseOutcome<talkbank_model::model::SinTier> {
    use chumsky::Parser as _;
    let tokens = crate::parser::lex_to_tokens(input, crate::lexer::COND_SIN_CONTENT);
    match crate::parser::dependent_tiers::sin_tier_parser()
        .parse(tokens.as_slice())
        .into_result()
    {
        Ok(parsed) => talkbank_model::ParseOutcome::parsed(convert_sin_tier(&parsed)),
        Err(_) => talkbank_model::ParseOutcome::rejected(),
    }
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

    let (tokens, source) =
        crate::parser::lex_to_tokens_and_source(input, crate::lexer::COND_MAIN_CONTENT);
    let source = crate::source_text::SourceText::new(source);
    crate::parser::dependent_tiers::wor_tier_parser()
        .parse(tokens.as_slice())
        .into_result()
        .ok()
        .map(|parsed| crate::convert::tiers::wor_tier_to_model(&parsed, source))
}

// `From<&ast::MainTier>` and `From<&ast::Utterance>` USED to live here, under
// a comment saying "no source needed". That stopped being true when separators
// began carrying their position: placing a borrowed slice needs the text it
// was borrowed from, and a `From` has nowhere to receive it. Keeping them
// would have meant fabricating `Span::DUMMY` inside an infallible conversion,
// which is the defect this change exists to remove, so they are gone and the
// two callers pass a `SourceText` explicitly.

// `From<&ast::WordWithAnnotations>` went the same way and for the same reason:
// a word's span comes from the text its `raw_text` borrows, and a `From` has
// nowhere to receive it.

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

#[cfg(test)]
mod gra_lowering_tests {
    use super::*;
    use talkbank_model::errors::ErrorCollector;

    /// Lower a `%gra` body through the real lexer and parser, as the whole-file
    /// path does.
    fn lower(body: &str) -> (GraTier, Vec<ErrorCode>) {
        let parsed = match crate::parser::parse_gra_tier(body) {
            Some(parsed) => parsed,
            None => panic!("the fixture bodies in this module all parse: {body}"),
        };
        let errors = ErrorCollector::new();
        let tier = gra_tier_to_model(&parsed, SourceText::new(body), &errors).tier_without_health();
        let codes = errors.to_vec().iter().map(|error| error.code).collect();
        (tier, codes)
    }

    /// SURVIVES: behaviour a signature cannot state. That an unrepresentable
    /// field is REPORTED and DROPPED, rather than lowered to a number, is a
    /// property of running the lowering.
    ///
    /// This is the case that made the oracle backend fabricate: the lexer
    /// admits any digit run for a head, so the only `parse` failure is a value
    /// too large for `usize`, and the answer used to be ZERO, which is the ROOT
    /// attachment. `2|<overflow>|PUNCT` became `2|0|PUNCT`: a well formed
    /// dependency tree invented out of invalid input.
    #[test]
    fn a_head_the_model_cannot_hold_is_reported_and_dropped() {
        let (tier, codes) = lower("1|0|ROOT 2|99999999999999999999|PUNCT\n");
        assert_eq!(codes, vec![ErrorCode::UnexpectedGrammarNode]);
        assert_eq!(
            tier.relations().len(),
            1,
            "the rejected relation must be dropped, never defaulted"
        );
        assert!(
            tier.relations().iter().all(|relation| relation.index == 1),
            "the surviving relation must be the one that was representable"
        );
    }

    /// SURVIVES: behaviour. An index of zero is a different rule with a
    /// different code, and the canonical parser reports E709 for it.
    #[test]
    fn a_zero_index_is_reported_as_its_own_rule() {
        let (tier, codes) = lower("0|0|ROOT\n");
        assert_eq!(codes, vec![ErrorCode::InvalidGrammarIndex]);
        assert!(tier.relations().is_empty());
    }

    /// SURVIVES: behaviour. An index too large to hold is E708, not E710:
    /// the two fields fail for the same reason and are reported differently
    /// because the canonical parser reports them differently, and an oracle
    /// that rejects the same input under another code is not one.
    #[test]
    fn an_index_the_model_cannot_hold_is_a_different_code_from_a_head() {
        let (_, codes) = lower("99999999999999999999|0|ROOT\n");
        assert_eq!(codes, vec![ErrorCode::MalformedGrammarRelation]);
    }

    /// SURVIVES: behaviour. The ordinary tier still lowers untouched.
    #[test]
    fn a_well_formed_tier_reports_nothing() {
        let (tier, codes) = lower("1|2|SUBJ 2|0|ROOT 3|2|OBJ\n");
        assert!(codes.is_empty());
        assert_eq!(tier.relations().len(), 3);
    }
}
