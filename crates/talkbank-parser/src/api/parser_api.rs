//! Fragment-aware parsing methods on TreeSitterParser.
//!
//! These methods accept an `offset` parameter for span adjustment and an
//! `ErrorSink` for streaming error reporting. They wrap the raw parsing
//! methods and handle offset correction for embedded CHAT fragments.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! # Architecture
//!
//! Most tier parsing follows the "minimal wrapper" pattern: the caller's
//! admitted `FragmentSource` is wrapped in a minimal valid CHAT document, the
//! wrapper is parsed, and the tier is lowered from the parse as a
//! `Synthetic<_>` value that only the wrapper can move into the caller's
//! coordinates (`api::fragment`). The pattern is encapsulated in
//! `parser_impl::wrapper_parse_tier()`.

use talkbank_model::dependent_tier::DependentTier;
use talkbank_model::model::{
    ActTier, AddTier, ChatFile, CodTier, ComTier, ExpTier, GpxTier, GraTier, GrammaticalRelation,
    Header, IDHeader, IntTier, MainTier, MorTier, MorWord, ParticipantEntry, PhoTier, PhoWord,
    SinTier, SitTier, SpaTier, Utterance, WorTier, Word,
};
use talkbank_model::{
    DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, ParseErrors, Severity,
};
use talkbank_model::{FragmentSemanticContext, ParseOutcome};

use super::parser_impl::{
    TierProjectionError, wrapper_parse_generic_tier, wrapper_parse_tier,
    wrapper_parse_tier_with_suffix,
};
use crate::parser::TreeSitterParser;

/// Report parse errors through an ErrorSink.
fn report_parse_errors(errors: ParseErrors, sink: &impl ErrorSink) {
    sink.report_vec(errors.into_error_vec());
}

const PARTICIPANTS_HEADER_PREFIX: &str = "@Participants:\t";

/// Cardinality admission shared by all single-item fragment projections.
struct SingleItem<T>(T);

enum ItemCardinalityError {
    Empty,
    Multiple,
}

impl<T> SingleItem<T> {
    fn admit(items: impl IntoIterator<Item = T>) -> Result<Self, ItemCardinalityError> {
        let mut items = items.into_iter();
        let Some(item) = items.next() else {
            return Err(ItemCardinalityError::Empty);
        };
        match items.next() {
            None => Ok(Self(item)),
            Some(_) => Err(ItemCardinalityError::Multiple),
        }
    }
}

fn reject_item_shape<T>(
    input: &str,
    errors: &impl ErrorSink,
    message: &'static str,
) -> ParseOutcome<T> {
    errors.report(ParseError::new(
        ErrorCode::InvalidWordFormat,
        Severity::Error,
        DiagnosticLocation::from_offsets(0, input.len()),
        ErrorContext::new(input, talkbank_model::Span::new(0, input.len()), input),
        message,
    ));
    ParseOutcome::rejected()
}

/// An admitted projection that loses neither tier items nor post-clitics.
struct SingleMorWord(MorWord);

enum MorWordShapeError {
    ItemCount,
    PostClitics,
}

impl TryFrom<MorTier> for SingleMorWord {
    type Error = MorWordShapeError;

    fn try_from(tier: MorTier) -> Result<Self, Self::Error> {
        let SingleItem(item) =
            SingleItem::admit(tier.into_items()).map_err(|_| MorWordShapeError::ItemCount)?;
        if !item.post_clitics.is_empty() {
            return Err(MorWordShapeError::PostClitics);
        }
        Ok(Self(item.main))
    }
}

/// Reject a main tier whose CA omission shorthand needs file context the
/// caller did not supply.
///
/// Runs over the INPUT-relative tier, before the fragment source rebases it,
/// so the word's location is already a snippet coordinate and the sink does
/// the one move into the caller's document. The shorthand's presence rejects
/// the fragment; the snippet is quoted only for a located word (every
/// tree-sitter word is).
fn report_context_dependent_main_tier_error(
    errors: &impl ErrorSink,
    input: &str,
    location: talkbank_model::Location,
) {
    let context = location.span().and_then(|span| {
        input
            .get(span.to_range())
            .map(|found| ErrorContext::new(input, span, found))
    });

    errors.report(
        ParseError::new(
            ErrorCode::InvalidWordFormat,
            Severity::Error,
            DiagnosticLocation::from(location),
            context,
            "Main-tier fragment requires file context to interpret CA omission shorthand",
        )
        .with_suggestion(
            "Parse a full CHAT file with @Options: CA (or CA-Unicode), or rewrite the fragment using context-independent CHAT notation",
        ),
    );
}

/// Fragment-aware parsing methods.
///
/// These take an `offset` parameter for span adjustment and an `ErrorSink`
/// for streaming error reporting. Use these when parsing embedded CHAT
/// fragments where spans need to map back to positions in a larger document.
///
/// For simple full-file parsing, use `parse_chat_file()` or
/// `parse_chat_file_streaming()` directly, no offset needed.
impl TreeSitterParser {
    // =========================================================================
    // Word-Level Fragment Parsing
    // =========================================================================

    /// Parse a word with the enclosing document's semantic options.
    ///
    /// Only admitted words are normalized; refusal and source coordinates are
    /// retained by the ordinary fragment boundary. CA interpretation uses the
    /// same typed operation as whole-document parsing, without reading raw text.
    pub fn parse_word_fragment_with_context(
        &self,
        input: &str,
        offset: usize,
        context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Word> {
        self.parse_word_fragment(input, offset, errors)
            .map(|mut word| {
                if context.ca_mode() {
                    talkbank_model::model::content::word::ca::normalize_ca_omission_word(&mut word);
                }
                word
            })
    }

    /// Parse an individual word with offset adjustment and streaming errors.
    pub fn parse_word_fragment(
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
        let adjusting_sink = fragment_source.error_sink(errors);
        match self.parse_word(input) {
            Ok(word) => fragment_source.rebase_reporting(word, errors),
            Err(errs) => {
                report_parse_errors(errs, &adjusting_sink);
                ParseOutcome::rejected()
            }
        }
    }

    // =========================================================================
    // Main Tier Fragment Parsing
    // =========================================================================

    /// Parse a main tier line with offset adjustment and streaming errors.
    pub fn parse_main_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MainTier> {
        self.parse_main_tier_fragment_with_context(
            input,
            offset,
            &FragmentSemanticContext::default(),
            errors,
        )
    }

    /// Parse a main tier line with explicit semantic context.
    pub fn parse_main_tier_fragment_with_context(
        &self,
        input: &str,
        offset: usize,
        context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MainTier> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let adjusting_sink = fragment_source.error_sink(errors);
        match self.parse_main_tier(input) {
            Ok(mut main) => {
                if context.ca_mode() {
                    crate::parser::chat_file_parser::chat_file::normalize::normalize_ca_omissions_main_tier(&mut main);
                } else if let Some(location) = main.find_context_dependent_ca_omission() {
                    report_context_dependent_main_tier_error(&adjusting_sink, input, location);
                    return ParseOutcome::rejected();
                }
                fragment_source.rebase_reporting(main, errors)
            }
            Err(errs) => {
                report_parse_errors(errs, &adjusting_sink);
                ParseOutcome::rejected()
            }
        }
    }

    /// Parse a single utterance with offset adjustment and streaming errors.
    pub fn parse_utterance_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Utterance> {
        self.parse_utterance_fragment_with_context(
            input,
            offset,
            &FragmentSemanticContext::default(),
            errors,
        )
    }

    /// Parse a single utterance with explicit semantic context.
    pub fn parse_utterance_fragment_with_context(
        &self,
        input: &str,
        offset: usize,
        context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Utterance> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let adjusting_sink = fragment_source.error_sink(errors);
        match self.parse_single_utterance(input) {
            Ok(single) => {
                let (mut utterance, document_context) = single.into_parts();
                if context.ca_mode() || document_context.ca_mode() {
                    crate::parser::chat_file_parser::chat_file::normalize::normalize_ca_omissions_main_tier(&mut utterance.main);
                } else if let Some(location) = utterance.main.find_context_dependent_ca_omission() {
                    report_context_dependent_main_tier_error(&adjusting_sink, input, location);
                    return ParseOutcome::rejected();
                }
                fragment_source.rebase_reporting(utterance, errors)
            }
            Err(errs) => {
                report_parse_errors(errs, &adjusting_sink);
                ParseOutcome::rejected()
            }
        }
    }

    /// Parse a complete CHAT file with offset adjustment and streaming errors.
    pub fn parse_chat_file_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ChatFile> {
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let adjusting_sink = fragment_source.error_sink(errors);
        let chat = self.parse_chat_file_streaming(input, &adjusting_sink);
        fragment_source.rebase_reporting(chat, errors)
    }

    // =========================================================================
    // Headers
    // =========================================================================

    /// Parse any header line with offset adjustment and streaming errors.
    pub fn parse_header_fragment(
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
        let adjusting_sink = fragment_source.error_sink(errors);
        match self.parse_header(input) {
            Ok(header) => fragment_source.rebase_reporting(header, errors),
            Err(parse_errors) => {
                for error in parse_errors.errors {
                    adjusting_sink.report(error);
                }
                ParseOutcome::rejected()
            }
        }
    }

    /// Parse an @ID header with offset adjustment and streaming errors.
    pub fn parse_id_header_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<IDHeader> {
        match self.parse_header_fragment(input, offset, errors) {
            ParseOutcome::Parsed(Header::ID(id)) => ParseOutcome::parsed(id),
            _ => ParseOutcome::rejected(),
        }
    }

    /// Parse a participant entry with offset adjustment and streaming errors.
    ///
    /// One wrapper: the caller's entry sits on a wrapper-supplied
    /// `@Participants:` line inside the post-preamble document, is lowered
    /// there and rebased once. (It used to hand its synthetic source to the
    /// public header API as if it were a document, which wrapped it again.)
    pub fn parse_participant_entry_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ParticipantEntry> {
        use crate::parser::chat_file_parser::header_dispatch::{
            HeaderCoverage, POST_BEGIN_HEADER_INDEX, POST_BEGIN_PREFIX, POST_BEGIN_SUFFIX,
        };
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let ParseOutcome::Parsed(wrapper) = super::parser_impl::admit_wrapper(
            super::fragment::WrappedFragment::new(
                &[POST_BEGIN_PREFIX, PARTICIPANTS_HEADER_PREFIX],
                fragment_source,
                POST_BEGIN_SUFFIX,
            ),
            errors,
        ) else {
            return ParseOutcome::rejected();
        };
        let header = match self.lower_wrapped_header(
            &wrapper,
            POST_BEGIN_HEADER_INDEX,
            HeaderCoverage::LabelSupplied,
        ) {
            Ok(header) => header,
            Err(found) => {
                errors.report_vec(found.into_error_vec());
                return ParseOutcome::rejected();
            }
        };
        let entry = header.try_map(|header| match header {
            Header::Participants { entries } => SingleItem::admit(entries)
                .map(|SingleItem(entry)| entry)
                .map_err(|_| "Expected exactly one participant entry"),
            _ => Err("Expected a participant entry"),
        });
        match entry {
            Ok(entry) => match wrapper.rebase(entry) {
                Ok(entry) => ParseOutcome::parsed(entry),
                Err(failure) => {
                    errors.report_vec(failure.into_error_vec());
                    ParseOutcome::rejected()
                }
            },
            Err(message) => reject_item_shape(input, &fragment_source.error_sink(errors), message),
        }
    }

    // =========================================================================
    // Morphology Tiers
    // =========================================================================

    /// Parse a %mor tier line with offset adjustment and streaming errors.
    pub fn parse_mor_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MorTier> {
        wrapper_parse_tier(self, "%mor:\t", input, offset, errors, |tier| match tier {
            DependentTier::Mor(tier) => Some(tier),
            _ => None,
        })
    }

    /// Parse a single MOR word with offset adjustment and streaming errors.
    pub fn parse_mor_word_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MorWord> {
        // The companion terminator is wrapper-owned text on the tier line; the
        // one wrapper excludes it from the tier's extent and from every
        // diagnostic. Select the word before translating the returned payload.
        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        wrapper_parse_tier_with_suffix(self, "%mor:\t", fragment_source, " .", errors, |tier| {
            match tier {
                DependentTier::Mor(tier) => SingleMorWord::try_from(tier)
                    .map(|SingleMorWord(word)| word)
                    .map_err(|reason| {
                        TierProjectionError::InvalidShape(match reason {
                            MorWordShapeError::ItemCount => "Expected exactly one MOR word",
                            MorWordShapeError::PostClitics => {
                                "A MOR word fragment cannot include post-clitics"
                            }
                        })
                    }),
                _ => Err(TierProjectionError::WrongKind),
            }
        })
    }

    // =========================================================================
    // Grammar Tiers
    // =========================================================================

    /// Parse a %gra tier line with offset adjustment and streaming errors.
    pub fn parse_gra_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<GraTier> {
        wrapper_parse_tier(self, "%gra:\t", input, offset, errors, |tier| match tier {
            DependentTier::Gra(tier) => Some(tier),
            _ => None,
        })
    }

    /// Parse a single grammatical relation with offset adjustment and streaming errors.
    pub fn parse_gra_relation_fragment(
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
        let document_sink = fragment_source.error_sink(errors);
        let Some(tier) = self
            .parse_gra_tier_fragment(input, offset, errors)
            .into_option()
        else {
            return ParseOutcome::rejected();
        };
        match SingleItem::admit(tier.into_relations()) {
            Ok(SingleItem(relation)) => ParseOutcome::parsed(relation),
            Err(_) => reject_item_shape(
                input,
                &document_sink,
                "Expected exactly one grammatical relation",
            ),
        }
    }

    // =========================================================================
    // Phonology Tiers
    // =========================================================================

    /// Parse a %pho tier line with offset adjustment and streaming errors.
    pub fn parse_pho_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<PhoTier> {
        wrapper_parse_tier(self, "%pho:\t", input, offset, errors, |tier| match tier {
            DependentTier::Pho(tier) => Some(tier),
            _ => None,
        })
    }

    /// Parse a single phonological word with offset adjustment and streaming errors.
    pub fn parse_pho_word_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<PhoWord> {
        use talkbank_model::model::PhoItem;

        let ParseOutcome::Parsed(fragment_source) =
            talkbank_model::FragmentSource::admit(input, offset, errors)
        else {
            return ParseOutcome::rejected();
        };
        let document_sink = fragment_source.error_sink(errors);
        let Some(tier) = self
            .parse_pho_tier_fragment(input, offset, errors)
            .into_option()
        else {
            return ParseOutcome::rejected();
        };
        let Ok(SingleItem(item)) = SingleItem::admit(tier.items.into_vec()) else {
            return reject_item_shape(
                input,
                &document_sink,
                "Expected exactly one phonological word",
            );
        };
        match item {
            PhoItem::Word(word) => ParseOutcome::parsed(word),
            PhoItem::Group(_) => reject_item_shape(
                input,
                &document_sink,
                "A phonological word fragment cannot include a group",
            ),
        }
    }

    // =========================================================================
    // Other Tiers
    // =========================================================================

    /// Parse a %sin tier with offset adjustment and streaming errors.
    pub fn parse_sin_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<SinTier> {
        wrapper_parse_tier(self, "%sin:\t", input, offset, errors, |tier| match tier {
            DependentTier::Sin(tier) => Some(tier),
            _ => None,
        })
    }

    /// Parse a %act tier with offset adjustment and streaming errors.
    pub fn parse_act_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ActTier> {
        wrapper_parse_tier(self, "%act:\t", input, offset, errors, |tier| match tier {
            DependentTier::Act(t) => Some(t),
            _ => None,
        })
    }

    /// Parse a %cod tier with offset adjustment and streaming errors.
    pub fn parse_cod_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<CodTier> {
        wrapper_parse_tier(self, "%cod:\t", input, offset, errors, |tier| match tier {
            DependentTier::Cod(t) => Some(t),
            _ => None,
        })
    }

    /// Parse a %com tier with offset adjustment and streaming errors.
    pub fn parse_com_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ComTier> {
        wrapper_parse_tier(self, "%com:\t", input, offset, errors, |tier| match tier {
            DependentTier::Com(t) => Some(t),
            _ => None,
        })
    }

    /// Parse a %exp tier with offset adjustment and streaming errors.
    pub fn parse_exp_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ExpTier> {
        wrapper_parse_tier(self, "%exp:\t", input, offset, errors, |tier| match tier {
            DependentTier::Exp(t) => Some(t),
            _ => None,
        })
    }

    /// Parse a %add tier with offset adjustment and streaming errors.
    pub fn parse_add_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<AddTier> {
        wrapper_parse_tier(self, "%add:\t", input, offset, errors, |tier| match tier {
            DependentTier::Add(t) => Some(t),
            _ => None,
        })
    }

    /// Parse a %gpx tier with offset adjustment and streaming errors.
    pub fn parse_gpx_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<GpxTier> {
        wrapper_parse_tier(self, "%gpx:\t", input, offset, errors, |tier| match tier {
            DependentTier::Gpx(t) => Some(t),
            _ => None,
        })
    }

    /// Parse a %int tier with offset adjustment and streaming errors.
    pub fn parse_int_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<IntTier> {
        wrapper_parse_tier(self, "%int:\t", input, offset, errors, |tier| match tier {
            DependentTier::Int(t) => Some(t),
            _ => None,
        })
    }

    /// Parse a %spa tier with offset adjustment and streaming errors.
    pub fn parse_spa_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<SpaTier> {
        wrapper_parse_tier(self, "%spa:\t", input, offset, errors, |tier| match tier {
            DependentTier::Spa(t) => Some(t),
            _ => None,
        })
    }

    /// Parse a %sit tier with offset adjustment and streaming errors.
    pub fn parse_sit_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<SitTier> {
        wrapper_parse_tier(self, "%sit:\t", input, offset, errors, |tier| match tier {
            DependentTier::Sit(t) => Some(t),
            _ => None,
        })
    }

    /// Parse a %wor tier with offset adjustment and streaming errors.
    pub fn parse_wor_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<WorTier> {
        wrapper_parse_tier(self, "%wor:\t", input, offset, errors, |tier| match tier {
            DependentTier::Wor(t) => Some(t),
            _ => None,
        })
    }

    /// Parse any dependent tier line (including prefix) with offset adjustment and streaming errors.
    pub fn parse_dependent_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<DependentTier> {
        wrapper_parse_generic_tier(self, input, offset, errors)
    }
}
