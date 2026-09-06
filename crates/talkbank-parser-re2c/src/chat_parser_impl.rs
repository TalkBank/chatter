//! Implementation of the `ChatParser` trait for the re2c-based parser.
//!
//! This makes our parser a drop-in replacement for `TreeSitterParser`
//! via the shared `ChatParser` trait. Shared tests verify both parsers
//! produce semantically equivalent output.

use talkbank_model::model::{
    ActTier, AddTier, ChatFile as ModelChatFile, CodTier, ComTier,
    DependentTier as ModelDependentTier, ExpTier, GpxTier, GraTier as ModelGraTier,
    GrammaticalRelation, Header, IDHeader, IntTier, MainTier as ModelMainTier,
    MorTier as ModelMorTier, MorWord, ParticipantEntry, PhoTier as ModelPhoTier, PhoWord, SitTier,
    SpaTier, Utterance as ModelUtterance, WorTier, Word,
};
use talkbank_model::{ChatParser, ErrorSink, ParseOutcome};

/// Re2c-based CHAT parser implementing the shared `ChatParser` trait.
///
/// Unlike `TreeSitterParser`, this parser uses re2c for lexing and
/// a handwritten recursive-descent parser. It does not require
/// tree-sitter to be installed or configured.
pub struct Re2cParser;

impl Re2cParser {
    /// Create a new re2c parser instance.
    ///
    /// Unlike `TreeSitterParser`, this is zero-cost, no grammar loading,
    /// no internal buffers. The parser is stateless and `Send + Sync`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for Re2cParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatParser for Re2cParser {
    fn parser_name(&self) -> &'static str {
        "Re2cParser"
    }

    fn parse_chat_file(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelChatFile> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let diagnostics = fragment_source.error_sink(errors);
        let file = crate::parser::parse_chat_file_to_model(input, &diagnostics);
        ParseOutcome::parsed(fragment_source.rebase(file))
    }

    fn parse_header(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Header> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let diagnostics = fragment_source.error_sink(errors);
        match crate::parser::HeaderFragment::parse(input, &diagnostics) {
            Some(fragment) => ParseOutcome::parsed(fragment_source.rebase(fragment.lower())),
            None => ParseOutcome::rejected(),
        }
    }

    fn parse_id_header(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<IDHeader> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        match crate::parser::parse_id_header(input) {
            Some(parsed) => ParseOutcome::parsed(fragment_source.rebase(IDHeader::from(&parsed))),
            None => ParseOutcome::rejected(),
        }
    }

    fn parse_participant_entry(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ParticipantEntry> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let diagnostics = fragment_source.error_sink(errors);
        let parsed = crate::parser::parse_participants_header(input, &diagnostics);
        match parsed.entries.first() {
            Some(entry) => {
                ParseOutcome::parsed(fragment_source.rebase(ParticipantEntry::from(entry)))
            }
            None => ParseOutcome::rejected(),
        }
    }

    fn parse_utterance(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelUtterance> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let diagnostics = fragment_source.error_sink(errors);
        let parsed = crate::parser::parse_chat_file_streaming(input, &diagnostics);
        let source = crate::source_text::SourceText::new(parsed.source);
        for line in &parsed.lines {
            if let crate::ast::Line::Utterance(u) = line {
                let model = crate::convert::utterance_to_model(u.as_ref(), source, &diagnostics);
                return ParseOutcome::parsed(fragment_source.rebase(model));
            }
        }
        ParseOutcome::rejected()
    }

    fn parse_main_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelMainTier> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let diagnostics = fragment_source.error_sink(errors);
        match crate::parser::parse_main_tier_with_source(input) {
            Some((parsed, source)) => {
                let model = crate::convert::main_tier_to_model(
                    &parsed,
                    crate::source_text::SourceText::new(source),
                    &diagnostics,
                );
                ParseOutcome::parsed(fragment_source.rebase(model))
            }
            None => ParseOutcome::rejected(),
        }
    }

    fn parse_word(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Word> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        match crate::parser::parse_word(input) {
            Some(parsed) => {
                let word = crate::convert::word_from_parsed(
                    &parsed,
                    crate::source_text::SourceText::new(input),
                );
                ParseOutcome::parsed(fragment_source.rebase(word))
            }
            None => ParseOutcome::rejected(),
        }
    }

    fn parse_mor_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelMorTier> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let parsed = crate::parser::parse_mor_tier(input);
        match ModelMorTier::try_from(&parsed) {
            Ok(tier) => ParseOutcome::parsed(fragment_source.rebase(tier)),
            // AST-to-model conversion failure (missing or unrecognized
            // terminator). Caller pattern-matches on Rejected.
            Err(_) => ParseOutcome::rejected(),
        }
    }

    fn parse_mor_word(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MorWord> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        match crate::parser::parse_mor_word(input) {
            Some(parsed) => ParseOutcome::parsed(fragment_source.rebase(MorWord::from(&parsed))),
            None => ParseOutcome::rejected(),
        }
    }

    fn parse_gra_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelGraTier> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let parsed = crate::parser::parse_gra_tier(input);
        ParseOutcome::parsed(fragment_source.rebase(ModelGraTier::from(&parsed)))
    }

    fn parse_gra_relation(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<GrammaticalRelation> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        match crate::parser::parse_gra_relation(input) {
            Some(parsed) => {
                ParseOutcome::parsed(fragment_source.rebase(GrammaticalRelation::from(&parsed)))
            }
            None => ParseOutcome::rejected(),
        }
    }

    fn parse_pho_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelPhoTier> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let parsed = crate::parser::parse_pho_tier(input);
        ParseOutcome::parsed(fragment_source.rebase(ModelPhoTier::from(&parsed)))
    }

    fn parse_pho_word(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<PhoWord> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let parsed = crate::parser::parse_pho_tier(input);
        let first_word = parsed.items.iter().find_map(|item| match item {
            crate::ast::PhoItemParsed::Word(w) => Some(w),
            _ => None,
        });
        match first_word {
            Some(w) => ParseOutcome::parsed(fragment_source.rebase(PhoWord::from(w))),
            None => ParseOutcome::rejected(),
        }
    }

    fn parse_sin_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<talkbank_model::model::SinTier> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        match crate::convert::sin_tier_from_text(input) {
            ParseOutcome::Parsed(tier) => ParseOutcome::parsed(fragment_source.rebase(tier)),
            ParseOutcome::Rejected => {
                errors.report(talkbank_model::ParseError::new(
                    talkbank_model::ErrorCode::UnparsableContent,
                    talkbank_model::Severity::Error,
                    talkbank_model::SourceLocation::from_offsets(
                        offset,
                        offset.saturating_add(input.len()),
                    ),
                    talkbank_model::ErrorContext::new(input, 0..input.len(), input),
                    "Failed to parse %sin tier content",
                ));
                ParseOutcome::rejected()
            }
        }
    }

    fn parse_act_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ActTier> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let parsed = crate::parser::parse_text_tier(input);
        ParseOutcome::parsed(fragment_source.rebase(crate::convert::to_act_tier(&parsed)))
    }

    fn parse_cod_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<CodTier> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let parsed = crate::parser::parse_text_tier(input);
        ParseOutcome::parsed(fragment_source.rebase(crate::convert::to_cod_tier(&parsed)))
    }

    fn parse_com_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ComTier> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let parsed = crate::parser::parse_text_tier(input);
        ParseOutcome::parsed(fragment_source.rebase(crate::convert::to_com_tier(&parsed)))
    }

    fn parse_exp_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ExpTier> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let parsed = crate::parser::parse_text_tier(input);
        ParseOutcome::parsed(fragment_source.rebase(crate::convert::to_exp_tier(&parsed)))
    }

    fn parse_add_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<AddTier> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let parsed = crate::parser::parse_text_tier(input);
        ParseOutcome::parsed(fragment_source.rebase(crate::convert::to_add_tier(&parsed)))
    }

    fn parse_gpx_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<GpxTier> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let parsed = crate::parser::parse_text_tier(input);
        ParseOutcome::parsed(fragment_source.rebase(crate::convert::to_gpx_tier(&parsed)))
    }

    fn parse_int_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<IntTier> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let parsed = crate::parser::parse_text_tier(input);
        ParseOutcome::parsed(fragment_source.rebase(crate::convert::to_int_tier(&parsed)))
    }

    fn parse_spa_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<SpaTier> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let parsed = crate::parser::parse_text_tier(input);
        ParseOutcome::parsed(fragment_source.rebase(crate::convert::to_spa_tier(&parsed)))
    }

    fn parse_sit_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<SitTier> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let parsed = crate::parser::parse_text_tier(input);
        ParseOutcome::parsed(fragment_source.rebase(crate::convert::to_sit_tier(&parsed)))
    }

    fn parse_wor_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<WorTier> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        match crate::convert::wor_tier_from_input(input) {
            Some(wor) => ParseOutcome::parsed(fragment_source.rebase(wor)),
            // An unparsable %wor tier used to arrive here as an empty one, so a
            // malformed tier and a tier with no words were indistinguishable.
            None => ParseOutcome::rejected(),
        }
    }

    fn parse_dependent_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelDependentTier> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let parsed = crate::parser::parse_chat_file(input);
        let source = crate::source_text::SourceText::new(parsed.source);
        for line in &parsed.lines {
            if let crate::ast::Line::Utterance(u) = line
                && let Some(tier) = u.dependent_tiers.first()
                && let Some(model_tier) =
                    crate::convert::dependent_tier_to_model(&tier.tier, source)
            {
                return ParseOutcome::parsed(fragment_source.rebase(model_tier));
            }
        }
        ParseOutcome::rejected()
    }
}
