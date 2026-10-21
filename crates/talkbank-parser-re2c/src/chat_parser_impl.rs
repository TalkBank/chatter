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
    ChatParser, ErrorSink, FragmentSemanticContext, FragmentSource, ParseOutcome,
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
        // The shared trait accepts a complete header, unlike the low-level
        // body-only helper. Native header admission keeps the prefix out of
        // the language field and accounts for every original caller byte.
        let diagnostics = fragment_source.error_sink(errors);
        match crate::parser::HeaderFragment::parse(input, &diagnostics).map(|header| header.lower())
        {
            Some(Header::ID(id)) => ParseOutcome::parsed(fragment_source.rebase(id)),
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
        let parsed = crate::parser::parse_participants_header(input, &diagnostics);
        match parsed.entries.as_slice() {
            [entry] if entry.words.len() >= 2 => {
                ParseOutcome::parsed(fragment_source.rebase(ParticipantEntry::from(entry)))
            }
            _ => Self::rejected_fragment(fragment_source, errors, "single participant entry"),
        }
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
        let source = crate::source_text::SourceText::new(parsed.source);
        for line in &parsed.lines {
            if let crate::ast::Line::Utterance(u) = line {
                let mut model =
                    crate::convert::utterance_to_model(u.as_ref(), source, &diagnostics);
                model.main =
                    match Self::contextual_main(fragment_source, model.main, context, errors) {
                        ParseOutcome::Parsed(main) => main,
                        ParseOutcome::Rejected => return ParseOutcome::rejected(),
                    };
                return ParseOutcome::parsed(fragment_source.rebase(model));
            }
        }
        Self::rejected_fragment(fragment_source, errors, "utterance")
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
        let mut lines = parsed.lines.into_iter();
        match (lines.next(), lines.next()) {
            (Some(crate::ast::Line::Utterance(utterance)), None)
                if utterance.dependent_tiers.is_empty() =>
            {
                let model = crate::convert::main_tier_to_model(
                    &utterance.main_tier,
                    crate::source_text::SourceText::new(input),
                    &diagnostics,
                );
                match Self::contextual_main(fragment_source, model, context, errors) {
                    ParseOutcome::Parsed(main) => {
                        ParseOutcome::parsed(fragment_source.rebase(main))
                    }
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
        match crate::parser::parse_word(input) {
            Some(parsed) => {
                let mut word = crate::convert::word_from_parsed(
                    &parsed,
                    crate::source_text::SourceText::new(input),
                );
                if context.ca_mode() {
                    talkbank_model::model::content::word::ca::normalize_ca_omission_word(&mut word);
                }
                ParseOutcome::parsed(fragment_source.rebase(word))
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
        let parsed = crate::parser::parse_mor_tier(input);
        // A fragment's tier is the whole input.
        let source = crate::source_text::SourceText::new(input);
        match crate::convert::mor_tier_to_model(
            &parsed,
            talkbank_model::Location::Located(source.whole()),
            source,
        ) {
            Ok(tier) => ParseOutcome::parsed(fragment_source.rebase(tier)),
            // AST-to-model conversion failure (missing or unrecognized
            // terminator). Caller pattern-matches on Rejected.
            Err(_) => Self::rejected_fragment(fragment_source, errors, "MOR tier"),
        }
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
        match crate::parser::parse_mor_word(input) {
            Some(parsed) => ParseOutcome::parsed(fragment_source.rebase(MorWord::from(&parsed))),
            None => Self::rejected_fragment(fragment_source, errors, "MOR word"),
        }
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
        let Some(parsed) = crate::parser::parse_gra_tier(input) else {
            // Declined, not lowered to an empty tier. This used to hand the
            // caller a `%gra` tier of no relations and call it a parse.
            return Self::rejected_fragment(fragment_source, errors, "GRA tier");
        };
        if parsed.relations.is_empty() {
            return Self::rejected_fragment(fragment_source, errors, "nonempty GRA tier");
        }
        let source = crate::source_text::SourceText::new(input);
        // The REBASING sink, not the caller's raw one. `rebase` moves the
        // model; a diagnostic already handed to a sink is past moving, so a
        // span computed against the fragment stays fragment-local and points
        // at the wrong bytes of the file. Every other entry point in this file
        // wraps first, and so does the canonical backend.
        let diagnostics = fragment_source.error_sink(errors);
        ParseOutcome::parsed(
            fragment_source.rebase(
                crate::convert::gra_tier_to_model(&parsed, source, &diagnostics)
                    .tier_without_health(talkbank_model::Location::Located(source.whole())),
            ),
        )
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
        let source = crate::source_text::SourceText::new(input);
        let diagnostics = fragment_source.error_sink(errors);
        match crate::parser::parse_gra_relation(input) {
            // A relation the model cannot hold is REJECTED here too, with the
            // diagnostic already reported, rather than lowered with a
            // fabricated head. The fragment API and the whole-file path answer
            // alike in code, in drop behaviour AND in POSITION: the sink is the
            // rebasing one, so the span is the caller's offset rather than the
            // fragment's own.
            Some(parsed) => {
                match crate::convert::gra_relation_to_model(&parsed, source, &diagnostics) {
                    ParseOutcome::Parsed(relation) => {
                        ParseOutcome::parsed(fragment_source.rebase(relation))
                    }
                    ParseOutcome::Rejected => ParseOutcome::rejected(),
                }
            }
            None => Self::rejected_fragment(fragment_source, errors, "GRA relation"),
        }
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
        match crate::parser::parse_pho_tier_checked(input) {
            Some(parsed) if !parsed.items.is_empty() => {
                ParseOutcome::parsed(fragment_source.rebase(crate::convert::convert_pho_tier(
                    &parsed,
                    talkbank_model::model::dependent_tier::pho::PhoTierType::Pho,
                    talkbank_model::Location::Located(
                        crate::source_text::SourceText::new(input).whole(),
                    ),
                )))
            }
            Some(_) | None => Self::rejected_fragment(fragment_source, errors, "phonology tier"),
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
        match crate::parser::parse_pho_tier_checked(input) {
            Some(parsed) => match parsed.items.as_slice() {
                [crate::ast::PhoItemParsed::Word(word)] => {
                    ParseOutcome::parsed(fragment_source.rebase(PhoWord::from(word)))
                }
                _ => Self::rejected_fragment(fragment_source, errors, "phonology word"),
            },
            None => Self::rejected_fragment(fragment_source, errors, "phonology word"),
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
        match crate::convert::sin_tier_from_text(input) {
            ParseOutcome::Parsed(tier) if !tier.is_empty() => {
                ParseOutcome::parsed(fragment_source.rebase(tier))
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
        let parsed = crate::parser::parse_text_tier_with_errors(input, &diagnostics);
        ParseOutcome::parsed(fragment_source.rebase(crate::convert::to_act_tier(
            &parsed,
            talkbank_model::Location::Located(crate::source_text::SourceText::new(input).whole()),
        )))
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
        let parsed = crate::parser::parse_text_tier_with_errors(input, &diagnostics);
        ParseOutcome::parsed(fragment_source.rebase(crate::convert::to_cod_tier(
            &parsed,
            talkbank_model::Location::Located(crate::source_text::SourceText::new(input).whole()),
        )))
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
        let parsed = crate::parser::parse_com_tier_with_errors(input, &diagnostics);
        ParseOutcome::parsed(fragment_source.rebase(crate::convert::to_com_tier(
            &parsed,
            talkbank_model::Location::Located(crate::source_text::SourceText::new(input).whole()),
        )))
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
        let parsed = crate::parser::parse_text_tier_with_errors(input, &diagnostics);
        ParseOutcome::parsed(fragment_source.rebase(crate::convert::to_exp_tier(
            &parsed,
            talkbank_model::Location::Located(crate::source_text::SourceText::new(input).whole()),
        )))
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
        let parsed = crate::parser::parse_text_tier_with_errors(input, &diagnostics);
        ParseOutcome::parsed(fragment_source.rebase(crate::convert::to_add_tier(
            &parsed,
            talkbank_model::Location::Located(crate::source_text::SourceText::new(input).whole()),
        )))
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
        let parsed = crate::parser::parse_text_tier_with_errors(input, &diagnostics);
        ParseOutcome::parsed(fragment_source.rebase(crate::convert::to_gpx_tier(
            &parsed,
            talkbank_model::Location::Located(crate::source_text::SourceText::new(input).whole()),
        )))
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
        let parsed = crate::parser::parse_text_tier_with_errors(input, &diagnostics);
        ParseOutcome::parsed(fragment_source.rebase(crate::convert::to_int_tier(
            &parsed,
            talkbank_model::Location::Located(crate::source_text::SourceText::new(input).whole()),
        )))
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
        let parsed = crate::parser::parse_text_tier_with_errors(input, &diagnostics);
        ParseOutcome::parsed(fragment_source.rebase(crate::convert::to_spa_tier(
            &parsed,
            talkbank_model::Location::Located(crate::source_text::SourceText::new(input).whole()),
        )))
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
        let parsed = crate::parser::parse_text_tier_with_errors(input, &diagnostics);
        ParseOutcome::parsed(fragment_source.rebase(crate::convert::to_sit_tier(
            &parsed,
            talkbank_model::Location::Located(crate::source_text::SourceText::new(input).whole()),
        )))
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
            Some(wor) => ParseOutcome::parsed(fragment_source.rebase(wor)),
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
        // for the reason `LoweredGra::tier_without_health` gives: a fragment
        // has no utterance, so recovery has nowhere to be recorded and nothing
        // downstream that could read it. The rejection itself still reaches
        // `diagnostics`. A binding a reader can see beats a temporary.
        let mut no_utterance_health = talkbank_model::model::ParseHealth::untainted();
        if let Some(tier) = crate::parser::parse_dependent_fragment(input, &diagnostics)
            && let Some(model_tier) = crate::convert::dependent_tier_to_model(
                &tier.tier,
                talkbank_model::Location::Located(tier.extent),
                crate::source_text::SourceText::new(input),
                &diagnostics,
                &mut no_utterance_health,
            )
        {
            return ParseOutcome::parsed(fragment_source.rebase(model_tier));
        }
        Self::rejected_fragment(fragment_source, errors, "dependent tier")
    }
}
