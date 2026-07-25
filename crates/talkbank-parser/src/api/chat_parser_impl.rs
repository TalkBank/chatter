//! Implementation of the `ChatParser` trait for the tree-sitter parser.
//!
//! `ChatParser` (in `talkbank-model`) is the parser-agnostic CHAT parsing
//! API: `Re2cParser` has implemented it from the start, and this impl
//! makes the canonical `TreeSitterParser` usable through the SAME trait,
//! so downstream consumers select a backend with one generic bound
//! instead of a cfg-gated facade (the friction the first external crate
//! consumer had to work around before this existed).
//!
//! Every method is pure delegation to the corresponding inherent
//! `*_fragment` method on `TreeSitterParser` (same `&self` receiver,
//! same `(input, offset, errors)` shape, same `ParseOutcome` return),
//! so trait-path and inherent-path behavior cannot diverge. The two
//! `_with_context` trait hooks with real semantics on this backend
//! (main tier and utterance, where `FragmentSemanticContext` decides CA
//! omission handling) override the trait defaults to delegate to their
//! context-aware inherent counterparts; the remaining `_with_context`
//! defaults (which ignore the context) are correct as-is because the
//! corresponding inherent parsers take no semantic context either.

use talkbank_model::dependent_tier::DependentTier;
use talkbank_model::model::{
    ActTier, AddTier, ChatFile, CodTier, ComTier, ExpTier, GpxTier, GraTier, GrammaticalRelation,
    Header, IDHeader, IntTier, MainTier, MorTier, MorWord, ParticipantEntry, PhoTier, PhoWord,
    SinTier, SitTier, SpaTier, Utterance, WorTier, Word,
};
use talkbank_model::{ChatParser, ErrorSink, FragmentSemanticContext, ParseOutcome};

use crate::parser::TreeSitterParser;

impl ChatParser for TreeSitterParser {
    fn parser_name(&self) -> &'static str {
        "TreeSitterParser"
    }

    fn parse_chat_file(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ChatFile> {
        self.parse_chat_file_fragment(input, offset, errors)
    }

    fn parse_header(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Header> {
        self.parse_header_fragment(input, offset, errors)
    }

    fn parse_id_header(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<IDHeader> {
        self.parse_id_header_fragment(input, offset, errors)
    }

    fn parse_participant_entry(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ParticipantEntry> {
        self.parse_participant_entry_fragment(input, offset, errors)
    }

    fn parse_utterance(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Utterance> {
        self.parse_utterance_fragment(input, offset, errors)
    }

    fn parse_utterance_with_context(
        &self,
        input: &str,
        offset: usize,
        context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Utterance> {
        self.parse_utterance_fragment_with_context(input, offset, context, errors)
    }

    fn parse_main_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MainTier> {
        self.parse_main_tier_fragment(input, offset, errors)
    }

    fn parse_main_tier_with_context(
        &self,
        input: &str,
        offset: usize,
        context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MainTier> {
        self.parse_main_tier_fragment_with_context(input, offset, context, errors)
    }

    fn parse_word(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Word> {
        self.parse_word_fragment(input, offset, errors)
    }

    fn parse_mor_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MorTier> {
        self.parse_mor_tier_fragment(input, offset, errors)
    }

    fn parse_mor_word(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MorWord> {
        self.parse_mor_word_fragment(input, offset, errors)
    }

    fn parse_gra_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<GraTier> {
        self.parse_gra_tier_fragment(input, offset, errors)
    }

    fn parse_gra_relation(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<GrammaticalRelation> {
        self.parse_gra_relation_fragment(input, offset, errors)
    }

    fn parse_pho_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<PhoTier> {
        self.parse_pho_tier_fragment(input, offset, errors)
    }

    fn parse_pho_word(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<PhoWord> {
        self.parse_pho_word_fragment(input, offset, errors)
    }

    fn parse_sin_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<SinTier> {
        self.parse_sin_tier_fragment(input, offset, errors)
    }

    fn parse_act_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ActTier> {
        self.parse_act_tier_fragment(input, offset, errors)
    }

    fn parse_cod_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<CodTier> {
        self.parse_cod_tier_fragment(input, offset, errors)
    }

    fn parse_com_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ComTier> {
        self.parse_com_tier_fragment(input, offset, errors)
    }

    fn parse_exp_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ExpTier> {
        self.parse_exp_tier_fragment(input, offset, errors)
    }

    fn parse_add_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<AddTier> {
        self.parse_add_tier_fragment(input, offset, errors)
    }

    fn parse_gpx_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<GpxTier> {
        self.parse_gpx_tier_fragment(input, offset, errors)
    }

    fn parse_int_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<IntTier> {
        self.parse_int_tier_fragment(input, offset, errors)
    }

    fn parse_spa_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<SpaTier> {
        self.parse_spa_tier_fragment(input, offset, errors)
    }

    fn parse_sit_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<SitTier> {
        self.parse_sit_tier_fragment(input, offset, errors)
    }

    fn parse_wor_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<WorTier> {
        self.parse_wor_tier_fragment(input, offset, errors)
    }

    fn parse_dependent_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<DependentTier> {
        self.parse_dependent_tier_fragment(input, offset, errors)
    }
}
