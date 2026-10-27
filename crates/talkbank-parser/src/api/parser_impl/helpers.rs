//! Shared implementation helpers behind `TreeSitterParser` tier fragment methods.
//!
//! The key mechanism here is the "minimal CHAT wrapper" pattern: synthesize a
//! tiny valid file around one tier, parse it through the full parser pipeline,
//! and then project just the requested tier back out with corrected spans.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::api::fragment::{Synthetic, WrappedFragment};
use talkbank_model::ParseOutcome;
use talkbank_model::dependent_tier::DependentTier;
use talkbank_model::model::Line;
use talkbank_model::{
    ErrorCode, ErrorCollector, ErrorSink, FragmentSource, ParseError, Severity, SpanShift,
    TeeErrorSink,
};

use crate::parser::TreeSitterParser;
use crate::parser::chat_file_parser::{MINIMAL_CHAT_PREFIX, MINIMAL_CHAT_SUFFIX};

/// The one way a wrapper fails to assemble: the synthetic document would
/// exceed the 32-bit byte space the tree-sitter grammar addresses. Reported
/// through the caller's sink and refused; unreachable with any input a
/// transcript could hold, kept so the bound is a diagnostic rather than a
/// panic in the grammar. One owner for the three fragment paths that build
/// a wrapper.
pub(crate) fn admit_wrapper<'input>(
    wrapper: Result<WrappedFragment<'input>, talkbank_model::ParseErrors>,
    errors: &impl ErrorSink,
) -> ParseOutcome<WrappedFragment<'input>> {
    match wrapper {
        Ok(fragment) => ParseOutcome::Parsed(fragment),
        Err(errors_found) => {
            errors.report_vec(errors_found.into_error_vec());
            ParseOutcome::Rejected
        }
    }
}

// =========================================================================
// Wrapper Parsing Infrastructure
// =========================================================================

/// Main tier line used in minimal wrappers.
const MAIN_TIER_LINE: &str = "*CHI:\thello .\n";

/// Parse a dependent tier using the minimal wrapper pattern.
///
/// # The Minimal Wrapper Pattern
///
/// These adapters reuse whole-file lowering to parse dependent-tier content.
/// They wrap that content in a minimal valid CHAT structure:
///
/// ```text
/// @UTF8
/// @Begin
/// @Languages:  eng
/// *CHI:    hello .
/// %xxx:    <INPUT>
/// @End
/// ```
///
/// # Dual Error Handling
///
/// We use TWO error sinks:
/// 1. `WrappedFragment::error_sink`: Translates wrapper coordinates through the
///    owned input boundary to document coordinates for the user's ErrorSink.
/// 2. `ErrorCollector`: Collects errors to determine success/failure.
///
/// `TeeErrorSink` streams each error to both sinks simultaneously.
///
/// # Span Adjustment
///
/// Parsed objects have spans relative to the wrapper and are typed as such
/// (`Synthetic<_>`). Project the tier's wrapper-owned extent, select the
/// caller-owned payload, then consume it through `WrappedFragment::rebase`.
/// Only a completely admitted payload reaches the caller's document space.
///
/// # Type Parameters
///
/// * `T` - The tier type to extract after the owned dependent tier is projected
/// * `F` - Extractor function that pulls the tier from a `DependentTier`
///
/// # Arguments
///
/// * `parser` - The TreeSitterParser instance
/// * `tier_header` - The tier marker including tab (e.g., `"%mor:\t"`)
/// * `input` - The tier content to parse (without the header)
/// * `offset` - Byte offset in the original document
/// * `errors` - User's error sink for real-time reporting
/// * `extractor` - Function to extract the specific tier type from `DependentTier`
pub(crate) fn wrapper_parse_tier<T: SpanShift, F>(
    parser: &TreeSitterParser,
    tier_header: &str,
    input: &str,
    offset: usize,
    errors: &impl ErrorSink,
    extractor: F,
) -> ParseOutcome<T>
where
    F: FnOnce(DependentTier) -> Option<T>,
{
    let ParseOutcome::Parsed(source) = FragmentSource::admit(input, offset, errors) else {
        return ParseOutcome::rejected();
    };
    wrapper_parse_tier_with_suffix(parser, tier_header, source, "", errors, |tier| {
        extractor(tier).ok_or(TierProjectionError::WrongKind)
    })
}

/// [`wrapper_parse_tier`] over an already admitted source, with synthetic
/// text the wrapper appends to the caller's input on the same tier line
/// (`" ."` gives a lone `%mor` word its companion terminator). The appended
/// text is wrapper-owned: it lies outside the caller's input like the tier
/// label does, so the tier's clamped extent and every diagnostic exclude it.
pub(crate) fn wrapper_parse_tier_with_suffix<T: SpanShift, F>(
    parser: &TreeSitterParser,
    tier_header: &str,
    source: FragmentSource<'_>,
    input_suffix: &str,
    errors: &impl ErrorSink,
    extractor: F,
) -> ParseOutcome<T>
where
    F: FnOnce(DependentTier) -> Result<T, TierProjectionError>,
{
    // Caller line endings are source evidence too. Supply a wrapper newline
    // only if the original input did not supply one; never shorten the owner's
    // admitted range merely to avoid an artificial blank line. Appended tier
    // text always needs the newline after it.
    let suffix = if input_suffix.is_empty() {
        tier_suffix(source.input())
    } else {
        std::borrow::Cow::Owned(format!("{input_suffix}\n{MINIMAL_CHAT_SUFFIX}"))
    };

    // Build the minimal CHAT wrapper
    let ParseOutcome::Parsed(fragment) = admit_wrapper(
        WrappedFragment::new(
            &[MINIMAL_CHAT_PREFIX, MAIN_TIER_LINE, tier_header],
            source,
            &suffix,
        ),
        errors,
    ) else {
        return ParseOutcome::rejected();
    };

    match CompleteTierFragment::parse(parser, &fragment, errors) {
        ParseOutcome::Parsed(tier) => tier.project(extractor, errors),
        ParseOutcome::Rejected => ParseOutcome::Rejected,
    }
}

/// Parse a generic dependent tier (where input includes the header).
///
/// Unlike `wrapper_parse_tier`, this function expects the input to already
/// include the tier header (e.g., `"%mor:\tpro|I v|want ."`).
///
/// Used by `parse_dependent_tier()` for parsing unknown tier types.
pub(crate) fn wrapper_parse_generic_tier(
    parser: &TreeSitterParser,
    input: &str,
    offset: usize,
    errors: &impl ErrorSink,
) -> ParseOutcome<DependentTier> {
    // Preserve the complete original line, including its caller-owned ending.
    let ParseOutcome::Parsed(source) = talkbank_model::FragmentSource::admit(input, offset, errors)
    else {
        return ParseOutcome::rejected();
    };
    let suffix = tier_suffix(source.input());

    // Build wrapper (input already has tier header)
    let ParseOutcome::Parsed(fragment) = admit_wrapper(
        WrappedFragment::new(&[MINIMAL_CHAT_PREFIX, MAIN_TIER_LINE], source, &suffix),
        errors,
    ) else {
        return ParseOutcome::rejected();
    };

    match CompleteTierFragment::parse(parser, &fragment, errors) {
        ParseOutcome::Parsed(tier) => tier.project(Ok, errors),
        ParseOutcome::Rejected => ParseOutcome::Rejected,
    }
}

/// Do not synthesize a line terminator that the caller already supplied.
/// LF/CRLF bytes stay in the owned input; an actual extra blank line survives.
fn tier_suffix(input: &str) -> std::borrow::Cow<'static, str> {
    if input.ends_with('\n') {
        std::borrow::Cow::Borrowed(MINIMAL_CHAT_SUFFIX)
    } else {
        std::borrow::Cow::Owned(format!("\n{MINIMAL_CHAT_SUFFIX}"))
    }
}

/// Distinguish an internal wrapper mismatch from a refused item projection.
pub(crate) enum TierProjectionError {
    WrongKind,
    InvalidShape(&'static str),
}

/// The single tier lowered from this wrapper, covering all caller content.
/// Parsing constructs the source/model association; projection cannot replace it.
struct CompleteTierFragment<'fragment, 'input> {
    fragment: &'fragment WrappedFragment<'input>,
    tier: Synthetic<DependentTier>,
}

impl<'fragment, 'input> CompleteTierFragment<'fragment, 'input> {
    fn parse(
        parser: &TreeSitterParser,
        fragment: &'fragment WrappedFragment<'input>,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Self> {
        // `parse_file` translates every diagnostic into the caller's
        // coordinates before it reaches the tee, so both sinks see the same
        // placed diagnostics; the collector only decides success.
        let collected = ErrorCollector::new();
        let tee = TeeErrorSink::new(errors, &collected);
        let file = fragment.parse_file(parser, &tee);
        if !collected.is_empty() {
            return ParseOutcome::Rejected;
        }
        let entries = file.map(|file| {
            file.lines
                .into_iter()
                .filter_map(|line| match line {
                    Line::Utterance(utterance) => Some(utterance.dependent_tiers),
                    Line::Header { .. } => None,
                })
                .flatten()
                .collect::<Vec<_>>()
        });
        // Exactly one tier, located, covering the whole caller input.
        let Ok(entry) = entries.try_map(|entries| <[_; 1]>::try_from(entries).map(|[entry]| entry))
        else {
            return Self::refuse(fragment, errors);
        };
        let Some(span) = entry.project(|entry| entry.location().span()).transpose() else {
            return Self::refuse(fragment, errors);
        };
        if fragment.require_complete_labelled_input(span).is_err() {
            return Self::refuse(fragment, errors);
        }
        ParseOutcome::Parsed(Self {
            fragment,
            tier: entry.map(|entry| entry.tier),
        })
    }

    /// The refusal points at the caller's whole input, in the caller's
    /// coordinates, through the caller's own sink.
    fn refuse(fragment: &WrappedFragment<'_>, errors: &impl ErrorSink) -> ParseOutcome<Self> {
        errors.report(ParseError::at_span(
            ErrorCode::ParseFailed,
            Severity::Error,
            fragment.input_document_span(),
            "Expected exactly one dependent tier covering the complete fragment",
        ));
        ParseOutcome::Rejected
    }

    /// Select the caller-owned payload while it remains synthetic. A wrong
    /// tier kind is an internal wrapper failure; a refused item shape is an
    /// input finding. Only the selected payload undergoes checked rebasing.
    fn project<T: SpanShift>(
        self,
        extractor: impl FnOnce(DependentTier) -> Result<T, TierProjectionError>,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<T> {
        // Select the caller-owned payload before translating. In particular,
        // a lone MOR word never returns its wrapper's companion terminator.
        let selected = self.fragment.project_tier(self.tier).try_map(extractor);
        match selected {
            Ok(value) => match self.fragment.rebase(value) {
                Ok(value) => ParseOutcome::Parsed(value),
                Err(failure) => {
                    errors.report_vec(failure.into_error_vec());
                    ParseOutcome::Rejected
                }
            },
            Err(reason) => {
                let (code, message) = match reason {
                    TierProjectionError::WrongKind => (
                        ErrorCode::InternalError,
                        "The tier wrapper lowered a dependent tier of a different kind than its label selects",
                    ),
                    TierProjectionError::InvalidShape(message) => {
                        (ErrorCode::InvalidWordFormat, message)
                    }
                };
                errors.report(self.fragment.whole_input_error(
                    code,
                    Severity::Error,
                    self.fragment.input(),
                    message,
                ));
                ParseOutcome::Rejected
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::model::MorTier;

    /// Tests wrapper parse tier mor.
    #[test]
    fn test_wrapper_parse_tier_mor() -> Result<(), String> {
        let parser =
            TreeSitterParser::new().map_err(|err| format!("Failed to create parser: {err}"))?;
        let errors = ErrorCollector::new();

        let result =
            wrapper_parse_tier(
                &parser,
                "%mor:\t",
                "pro|I v|want .",
                0,
                &errors,
                |tier| match tier {
                    DependentTier::Mor(tier) => Some(tier),
                    _ => None,
                },
            );

        assert!(result.is_parsed());
        let mor: MorTier = result
            .into_option()
            .ok_or_else(|| "Expected MOR tier to parse".to_string())?;
        assert!(!mor.items().is_empty());
        Ok(())
    }

    /// Regression: a tier fragment whose `input` already ends in a line
    /// terminator must still parse. The wrapper appends its own terminator, so
    /// before `strip_one_trailing_newline` a trailing `\n` in `input` doubled
    /// into a blank line and the fragment was spuriously rejected with E747
    /// (BlankLineNotAllowed). This was the root cause of the re2c
    /// `mor_tier_equivalence` / `gra_tier_equivalence` oracle divergence: the
    /// re2c oracle parses the raw fragment and tolerates a trailing newline, so
    /// the ts fragment API must too.
    #[test]
    fn wrapper_tolerates_trailing_newline_in_input() -> Result<(), String> {
        let parser =
            TreeSitterParser::new().map_err(|err| format!("Failed to create parser: {err}"))?;
        let errors = ErrorCollector::new();

        let result = wrapper_parse_tier(
            &parser,
            "%mor:\t",
            "pro|I v|want n|cookie-PL .\n",
            0,
            &errors,
            |tier| match tier {
                DependentTier::Mor(tier) => Some(tier),
                _ => None,
            },
        );

        if !errors.is_empty() {
            return Err(format!(
                "expected no fragment errors for trailing-newline input, got: {:?}",
                errors.to_vec()
            ));
        }
        let mor: MorTier = result
            .into_option()
            .ok_or_else(|| "Expected MOR tier to parse".to_string())?;
        assert_eq!(mor.items().len(), 3);
        Ok(())
    }

    /// A deliberate extra blank line (`input` ending in `"\n\n"`) must still be
    /// reported: `strip_one_trailing_newline` removes only ONE terminator, so
    /// the second newline still doubles against the wrapper's terminator.
    #[test]
    fn wrapper_still_rejects_genuine_blank_line() -> Result<(), String> {
        let parser =
            TreeSitterParser::new().map_err(|err| format!("Failed to create parser: {err}"))?;
        let errors = ErrorCollector::new();

        let _ = wrapper_parse_tier(
            &parser,
            "%mor:\t",
            "pro|I v|want n|cookie-PL .\n\n",
            0,
            &errors,
            |tier| match tier {
                DependentTier::Mor(tier) => Some(tier),
                _ => None,
            },
        );

        assert!(
            !errors.is_empty(),
            "a genuine extra blank line must still be reported"
        );
        Ok(())
    }
}
