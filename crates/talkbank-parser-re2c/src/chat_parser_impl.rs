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
use talkbank_model::{
    ChatParser, ErrorSink, FragmentSemanticContext, FragmentSource, ParseOutcome, SingleUtterance,
    UtteranceCardinalityError,
};

/// Re2c-based CHAT parser implementing the shared `ChatParser` trait.
///
/// Unlike `TreeSitterParser`, this parser uses re2c for lexing and
/// source-owning tokens and chumsky parser combinators. It does not require
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

    /// Token entry points have no file dispatcher to inspect caller controls.
    /// Do that inspection before a legacy lexer sentinel can hide suffix bytes.
    fn token_fragment_source<'source>(
        input: &'source str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<FragmentSource<'source>> {
        let ParseOutcome::Parsed(source) = FragmentSource::admit(input, offset, errors) else {
            return ParseOutcome::rejected();
        };
        talkbank_model::validation::report_control_characters(input, &source.error_sink(errors));
        ParseOutcome::parsed(source)
    }

    /// Interpret a native, input-relative main tier under caller-supplied
    /// semantics before rebasing. Shape and traversal have one model owner.
    fn contextual_main(
        source: FragmentSource<'_>,
        mut main: ModelMainTier,
        context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelMainTier> {
        if context.ca_mode() {
            talkbank_model::model::content::word::ca::normalize_ca_omissions_in_main_tier(
                &mut main,
            );
        } else if let Some(location) = main.find_context_dependent_ca_omission() {
            // The shorthand's presence rejects the fragment; a snippet is
            // quoted only when the lexer recorded where the word is.
            let context = location.span().map(|span| {
                talkbank_model::ErrorContext::new(source.input(), span, "CA omission shorthand")
            });
            source.error_sink(errors).report(talkbank_model::ParseError::new(
                talkbank_model::ErrorCode::InvalidWordFormat,
                talkbank_model::Severity::Error,
                talkbank_model::DiagnosticLocation::from(location),
                context,
                "Main-tier fragment requires an explicit CA option to interpret omission shorthand",
            ));
            return ParseOutcome::rejected();
        }
        ParseOutcome::parsed(main)
    }

    /// Native rejection must reach the caller's sink, not only the return enum.
    /// The admitted source owns both the original bytes and coordinate origin.
    fn rejected_fragment<T>(
        source: talkbank_model::FragmentSource<'_>,
        errors: &impl ErrorSink,
        context: &str,
    ) -> ParseOutcome<T> {
        let input = source.input();
        source
            .error_sink(errors)
            .report(talkbank_model::ParseError::new(
                talkbank_model::ErrorCode::UnparsableContent,
                talkbank_model::Severity::Error,
                talkbank_model::DiagnosticLocation::from_offsets(0, input.len()),
                talkbank_model::ErrorContext::new(
                    input,
                    talkbank_model::Span::new(0, input.len()),
                    context,
                ),
                format!("Native re2c grammar rejected the {context} fragment"),
            ));
        ParseOutcome::rejected()
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
        fragment_source.rebase_reporting(file, errors)
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
            Some(fragment) => fragment_source.rebase_reporting(fragment.lower(), errors),
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
        // The shared trait accepts a complete header, unlike the low-level
        // body-only helper. Native header admission keeps the prefix out of
        // the language field and accounts for every original caller byte.
        let diagnostics = fragment_source.error_sink(errors);
        match crate::parser::HeaderFragment::parse(input, &diagnostics).map(|header| header.lower())
        {
            Some(Header::ID(id)) => fragment_source.rebase_reporting(id, errors),
            _ => Self::rejected_fragment(fragment_source, errors, "ID header"),
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
        let findings = talkbank_model::ErrorCollector::new();
        let parsed = crate::parser::parse_participants_header(input, &findings);
        let outcome = match parsed.as_slice() {
            [entry] if !findings.has_errors() && entry.role().is_some() => {
                fragment_source.rebase_reporting(entry.clone(), errors)
            }
            _ => Self::rejected_fragment(fragment_source, errors, "single participant entry"),
        };
        diagnostics.report_all(findings.into_vec());
        outcome
    }

    fn parse_utterance(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelUtterance> {
        self.parse_utterance_with_context(input, offset, &FragmentSemanticContext::new(), errors)
    }

    fn parse_utterance_with_context(
        &self,
        input: &str,
        offset: usize,
        context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelUtterance> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let diagnostics = fragment_source.error_sink(errors);
        let parsed = crate::parser::parse_chat_file_streaming(input, &diagnostics);
        let file = crate::convert::chat_file_to_model(&parsed, &diagnostics);
        let (mut model, mut document_context) = match SingleUtterance::admit(file) {
            Ok(single) => single.into_parts(),
            Err(UtteranceCardinalityError::Missing) => {
                return Self::rejected_fragment(fragment_source, errors, "utterance");
            }
            Err(UtteranceCardinalityError::Multiple) => {
                return Self::rejected_fragment(
                    fragment_source,
                    errors,
                    "single utterance (multiple utterances supplied)",
                );
            }
        };
        document_context
            .option_flags
            .extend(context.option_flags.iter().cloned());
        model.main =
            match Self::contextual_main(fragment_source, model.main, &document_context, errors) {
                ParseOutcome::Parsed(main) => main,
                ParseOutcome::Rejected => return ParseOutcome::rejected(),
            };
        fragment_source.rebase_reporting(model, errors)
    }

    fn parse_main_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelMainTier> {
        self.parse_main_tier_with_context(input, offset, &FragmentSemanticContext::new(), errors)
    }

    fn parse_main_tier_with_context(
        &self,
        input: &str,
        offset: usize,
        context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelMainTier> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let diagnostics = fragment_source.error_sink(errors);
        // Reuse the native file dispatcher on the original fragment, so
        // recovered orphan annotations and missing separators remain errors.
        let parsed = crate::parser::parse_chat_file_streaming(input, &diagnostics);
        let mut lines = parsed.into_lines().into_iter();
        match (lines.next(), lines.next()) {
            (Some(crate::ast::Line::Utterance(utterance)), None)
                if utterance.dependent_tiers().is_empty() =>
            {
                let model = crate::convert::main_tier_to_model(utterance.main_tier(), &diagnostics);
                match Self::contextual_main(fragment_source, model, context, errors) {
                    ParseOutcome::Parsed(main) => fragment_source.rebase_reporting(main, errors),
                    ParseOutcome::Rejected => ParseOutcome::rejected(),
                }
            }
            _ => Self::rejected_fragment(fragment_source, errors, "main tier"),
        }
    }

    fn parse_word(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Word> {
        self.parse_word_with_context(input, offset, &FragmentSemanticContext::new(), errors)
    }

    fn parse_word_with_context(
        &self,
        input: &str,
        offset: usize,
        context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Word> {
        let ParseOutcome::Parsed(fragment_source) =
            Self::token_fragment_source(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        match crate::parser::entry_points::parse_standalone_word(input) {
            Some(mut word) => {
                if context.ca_mode() {
                    talkbank_model::model::content::word::ca::normalize_ca_omission_word(&mut word);
                }
                fragment_source.rebase_reporting(word, errors)
            }
            None => Self::rejected_fragment(fragment_source, errors, "word"),
        }
    }

    fn parse_mor_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelMorTier> {
        let ParseOutcome::Parsed(fragment_source) =
            Self::token_fragment_source(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        crate::parser::parse_mor_tier_with_errors(input, &fragment_source.error_sink(errors))
            .and_then(|tier| fragment_source.rebase_reporting(tier, errors))
    }

    fn parse_mor_word(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MorWord> {
        let ParseOutcome::Parsed(fragment_source) =
            Self::token_fragment_source(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        crate::parser::parse_mor_word_with_errors(input, &fragment_source.error_sink(errors))
            .and_then(|word| fragment_source.rebase_reporting(word, errors))
    }

    fn parse_gra_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelGraTier> {
        let ParseOutcome::Parsed(fragment_source) =
            Self::token_fragment_source(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let diagnostics = fragment_source.error_sink(errors);
        crate::parser::parse_gra_tier_with_errors(input, &diagnostics)
            .and_then(|tier| fragment_source.rebase_reporting(tier, errors))
    }

    fn parse_gra_relation(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<GrammaticalRelation> {
        let ParseOutcome::Parsed(fragment_source) =
            Self::token_fragment_source(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let diagnostics = fragment_source.error_sink(errors);
        crate::parser::parse_gra_relation_with_errors(input, &diagnostics)
            .and_then(|relation| fragment_source.rebase_reporting(relation, errors))
    }

    fn parse_pho_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelPhoTier> {
        let ParseOutcome::Parsed(fragment_source) =
            Self::token_fragment_source(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        match crate::parser::parse_pho_tier(input) {
            ParseOutcome::Parsed(parsed) if !parsed.items.is_empty() => {
                fragment_source.rebase_reporting(parsed, errors)
            }
            ParseOutcome::Parsed(_) | ParseOutcome::Rejected => {
                Self::rejected_fragment(fragment_source, errors, "phonology tier")
            }
        }
    }

    fn parse_pho_word(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<PhoWord> {
        let ParseOutcome::Parsed(fragment_source) =
            Self::token_fragment_source(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        match crate::parser::parse_pho_tier(input) {
            ParseOutcome::Parsed(parsed) => match parsed.items.as_slice() {
                [talkbank_model::PhoItem::Word(word)] => {
                    fragment_source.rebase_reporting(word.clone(), errors)
                }
                _ => Self::rejected_fragment(fragment_source, errors, "phonology word"),
            },
            ParseOutcome::Rejected => {
                Self::rejected_fragment(fragment_source, errors, "phonology word")
            }
        }
    }

    fn parse_sin_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<talkbank_model::model::SinTier> {
        let ParseOutcome::Parsed(fragment_source) =
            Self::token_fragment_source(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        match crate::parser::parse_sin_tier(input) {
            ParseOutcome::Parsed(tier) if !tier.is_empty() => {
                fragment_source.rebase_reporting(tier, errors)
            }
            ParseOutcome::Parsed(_) | ParseOutcome::Rejected => {
                Self::rejected_fragment(fragment_source, errors, "nonempty SIN tier")
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
        let diagnostics = fragment_source.error_sink(errors);
        crate::parser::parse_text_tier_with_errors(input, &diagnostics).and_then(|content| {
            fragment_source.rebase_reporting(
                ActTier::new(
                    talkbank_model::Location::Located(
                        crate::source_text::SourceText::new(input).whole(),
                    ),
                    content,
                ),
                errors,
            )
        })
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
        let diagnostics = fragment_source.error_sink(errors);
        crate::parser::parse_text_tier_with_errors(input, &diagnostics).and_then(|content| {
            fragment_source.rebase_reporting(
                CodTier::new(
                    talkbank_model::Location::Located(
                        crate::source_text::SourceText::new(input).whole(),
                    ),
                    content,
                ),
                errors,
            )
        })
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
        let diagnostics = fragment_source.error_sink(errors);
        crate::parser::parse_com_tier_with_errors(input, &diagnostics).and_then(|content| {
            fragment_source.rebase_reporting(
                ComTier::new(
                    talkbank_model::Location::Located(
                        crate::source_text::SourceText::new(input).whole(),
                    ),
                    content,
                ),
                errors,
            )
        })
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
        let diagnostics = fragment_source.error_sink(errors);
        crate::parser::parse_text_tier_with_errors(input, &diagnostics).and_then(|content| {
            fragment_source.rebase_reporting(
                ExpTier::new(
                    talkbank_model::Location::Located(
                        crate::source_text::SourceText::new(input).whole(),
                    ),
                    content,
                ),
                errors,
            )
        })
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
        let diagnostics = fragment_source.error_sink(errors);
        crate::parser::parse_text_tier_with_errors(input, &diagnostics).and_then(|content| {
            fragment_source.rebase_reporting(
                AddTier::new(
                    talkbank_model::Location::Located(
                        crate::source_text::SourceText::new(input).whole(),
                    ),
                    content,
                ),
                errors,
            )
        })
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
        let diagnostics = fragment_source.error_sink(errors);
        crate::parser::parse_text_tier_with_errors(input, &diagnostics).and_then(|content| {
            fragment_source.rebase_reporting(
                GpxTier::new(
                    talkbank_model::Location::Located(
                        crate::source_text::SourceText::new(input).whole(),
                    ),
                    content,
                ),
                errors,
            )
        })
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
        let diagnostics = fragment_source.error_sink(errors);
        crate::parser::parse_text_tier_with_errors(input, &diagnostics).and_then(|content| {
            fragment_source.rebase_reporting(
                IntTier::new(
                    talkbank_model::Location::Located(
                        crate::source_text::SourceText::new(input).whole(),
                    ),
                    content,
                ),
                errors,
            )
        })
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
        let diagnostics = fragment_source.error_sink(errors);
        crate::parser::parse_text_tier_with_errors(input, &diagnostics).and_then(|content| {
            fragment_source.rebase_reporting(
                SpaTier::new(
                    talkbank_model::Location::Located(
                        crate::source_text::SourceText::new(input).whole(),
                    ),
                    content,
                ),
                errors,
            )
        })
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
        let diagnostics = fragment_source.error_sink(errors);
        crate::parser::parse_text_tier_with_errors(input, &diagnostics).and_then(|content| {
            fragment_source.rebase_reporting(
                SitTier::new(
                    talkbank_model::Location::Located(
                        crate::source_text::SourceText::new(input).whole(),
                    ),
                    content,
                ),
                errors,
            )
        })
    }

    fn parse_wor_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<WorTier> {
        let ParseOutcome::Parsed(fragment_source) =
            Self::token_fragment_source(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let diagnostics = fragment_source.error_sink(errors);
        match crate::convert::wor_tier_from_input(input, &diagnostics) {
            Some(wor) => fragment_source.rebase_reporting(wor, errors),
            // An unparsable %wor tier used to arrive here as an empty one, so a
            // malformed tier and a tier with no words were indistinguishable.
            None => Self::rejected_fragment(fragment_source, errors, "WOR tier"),
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
        // The rebasing sink, for the reason the two `%gra` entry points above
        // give: this path emitted no diagnostics at all before the `%gra`
        // lowering became fallible, so passing the raw sink introduced the
        // fragment-local span rather than inheriting it.
        let diagnostics = fragment_source.error_sink(errors);
        // Named rather than passed inline as a fresh parse-health accumulator,
        // because a fragment
        // has no utterance, so recovery has nowhere to be recorded and nothing
        // downstream that could read it. The rejection itself still reaches
        // `diagnostics`. A binding a reader can see beats a temporary.
        let mut no_utterance_health = talkbank_model::model::ParseHealth::untainted();
        if let Some(tier) = crate::parser::parse_dependent_fragment(input, &diagnostics) {
            let model_tier = crate::convert::dependent_tier_to_model(
                &tier,
                &diagnostics,
                &mut no_utterance_health,
            );
            return fragment_source.rebase_reporting(model_tier, errors);
        }
        Self::rejected_fragment(fragment_source, errors, "dependent tier")
    }
}
