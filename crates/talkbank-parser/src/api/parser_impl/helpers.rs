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

use crate::api::fragment::WrappedFragment;
use talkbank_model::ParseOutcome;
use talkbank_model::dependent_tier::DependentTier;
use talkbank_model::model::Line;
use talkbank_model::{
    ErrorCode, ErrorCollector, ErrorSink, ParseError, Severity, Span, TeeErrorSink,
};

use crate::parser::TreeSitterParser;
use crate::parser::chat_file_parser::{MINIMAL_CHAT_PREFIX, MINIMAL_CHAT_SUFFIX};

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
/// Parsed objects have spans relative to the wrapper. We adjust them:
/// 1. Subtract wrapper prefix length → input-relative spans (0-based)
/// 2. Add user offset → document-absolute spans
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
pub(crate) fn wrapper_parse_tier<T, F>(
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
    // Caller line endings are source evidence too. Supply a wrapper newline
    // only if the original input did not supply one; never shorten the owner's
    // admitted range merely to avoid an artificial blank line.
    let ParseOutcome::Parsed(source) = talkbank_model::FragmentSource::admit(input, offset, errors)
    else {
        return ParseOutcome::rejected();
    };
    let suffix = tier_suffix(source.input());

    // Build the minimal CHAT wrapper
    let fragment = WrappedFragment::new(
        &[MINIMAL_CHAT_PREFIX, MAIN_TIER_LINE, tier_header],
        source.input(),
        &suffix,
        offset,
    );
    let fragment = match fragment {
        Ok(fragment) => fragment,
        Err(errors_found) => {
            errors.report_vec(errors_found.into_error_vec());
            return ParseOutcome::rejected();
        }
    };

    match CompleteTierFragment::parse(parser, &fragment, errors) {
        ParseOutcome::Parsed(tier) => tier.project(extractor),
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
    let fragment = WrappedFragment::new(
        &[MINIMAL_CHAT_PREFIX, MAIN_TIER_LINE],
        source.input(),
        &suffix,
        offset,
    );
    let fragment = match fragment {
        Ok(fragment) => fragment,
        Err(errors_found) => {
            errors.report_vec(errors_found.into_error_vec());
            return ParseOutcome::rejected();
        }
    };

    match CompleteTierFragment::parse(parser, &fragment, errors) {
        ParseOutcome::Parsed(tier) => tier.project(Some),
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

/// The single tier lowered from this wrapper, covering all caller content.
/// Parsing constructs the source/model association; projection cannot replace it.
struct CompleteTierFragment<'fragment, 'input> {
    fragment: &'fragment WrappedFragment<'input>,
    tier: DependentTier,
}

impl<'fragment, 'input> CompleteTierFragment<'fragment, 'input> {
    fn parse(
        parser: &TreeSitterParser,
        fragment: &'fragment WrappedFragment<'input>,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Self> {
        let collected = ErrorCollector::new();
        let adjusting = fragment.error_sink(errors);
        let tee = TeeErrorSink::new(&adjusting, &collected);
        let file = parser.parse_chat_file_streaming(fragment.source(), &tee);
        if !collected.is_empty() {
            return ParseOutcome::Rejected;
        }
        let mut tiers = file
            .lines
            .into_iter()
            .filter_map(|line| match line {
                Line::Utterance(utterance) => Some(utterance.dependent_tiers),
                Line::Header { .. } => None,
            })
            .flatten();
        let Some(entry) = tiers.next() else {
            return Self::refuse(fragment, &adjusting);
        };
        let span = entry.span();
        if tiers.next().is_some()
            || fragment
                .require_complete_tier_input(span.start as usize..span.end as usize)
                .is_err()
        {
            return Self::refuse(fragment, &adjusting);
        }
        ParseOutcome::Parsed(Self {
            fragment,
            tier: entry.tier,
        })
    }

    fn refuse(fragment: &WrappedFragment<'_>, errors: &impl ErrorSink) -> ParseOutcome<Self> {
        errors.report(ParseError::at_span(
            ErrorCode::ParseFailed,
            Severity::Error,
            Span::from_usize(0, fragment.source().len()),
            "Expected exactly one dependent tier covering the complete fragment",
        ));
        ParseOutcome::Rejected
    }

    fn project<T>(self, extractor: impl FnOnce(DependentTier) -> Option<T>) -> ParseOutcome<T> {
        match extractor(self.fragment.rebase_tier(self.tier)) {
            Some(tier) => ParseOutcome::Parsed(tier),
            None => ParseOutcome::Rejected,
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
