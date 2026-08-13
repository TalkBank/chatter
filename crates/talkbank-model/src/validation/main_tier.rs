//! Structural validation rules for main tiers.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Delimiters>

// Design rule 3, enforced by the compiler rather than by prose: a `_` arm over
// a content enum means a future variant compiles clean and answers wrong.
// Added per file as each is cleaned; `audit_content_catch_alls` lists the rest.
#![deny(clippy::wildcard_enum_match_arm)]
mod word_recursion;

pub(crate) use word_recursion::validate_words_at_every_depth;

use crate::model::{BracketedItem, ContentStructure, GroupRef, MainTier, UtteranceContent};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

/// Reject pauses that occur inside phonological groups (`‹...›`).
///
/// Pauses like `(.)` or `(1.5)` should not appear inside phonological groups.
/// This is a structural constraint of CHAT format.
///
/// Example violations:
/// - `‹hɛ (.) loʊ›` - ERROR: pause inside phonological group
///
/// Valid:
/// - `‹hɛloʊ›` - OK: no pauses
/// - `‹gʊd baɪ›` - OK: no pauses
pub(crate) fn check_no_pauses_in_pho_groups(main_tier: &MainTier, errors: &impl ErrorSink) {
    use crate::model::BracketedItem;

    // Recursively check all phonological groups in the main tier
    for content_item in main_tier.content.content.iter() {
        if let UtteranceContent::PhoGroup(pho_group) = content_item {
            // Check if any item in the pho group is a pause
            for item in &pho_group.content.content {
                if matches!(item, BracketedItem::Pause(_)) {
                    errors.report(
                        ParseError::new(
                            ErrorCode::PauseInPhoGroup,
                            Severity::Error,
                            SourceLocation::new(main_tier.span),
                            ErrorContext::new("", main_tier.span, ""),
                            "Pause cannot appear inside phonological group ‹...›",
                        )
                        .with_suggestion(
                            "Move the pause outside the phonological group, or remove it",
                        ),
                    );
                    // Only report once per phonological group
                    break;
                }
            }
        }
    }
}

/// Reject nested quotations inside a quotation span.
///
/// Quotations (`"..."`) should not contain other quotations. This checks both
/// main tier content and recursively through bracketed content.
///
/// Example violations:
/// - `"I said "hello" there"` - ERROR: nested quotation
///
/// Valid:
/// - `"I said hello there"` - OK: no nesting
/// - `he said "hello" and "goodbye"` - OK: separate quotations, not nested
pub(crate) fn check_no_nested_quotations(main_tier: &MainTier, errors: &impl ErrorSink) {
    // Check all main tier content for quotations
    for content_item in main_tier.content.content.iter() {
        if let UtteranceContent::Quotation(quotation) = content_item {
            // Check if this quotation contains any nested quotations
            if has_nested_quotation(&quotation.content.content) {
                errors.report(
                    ParseError::new(
                        ErrorCode::NestedQuotation,
                        Severity::Error,
                        SourceLocation::new(main_tier.span),
                        ErrorContext::new("", main_tier.span, ""),
                        "Quotations cannot be nested inside other quotations",
                    )
                    .with_suggestion("Use separate quotations or reformulate without nesting"),
                );
            }
        }
    }
}

/// Recursively detect whether any nested item is a quotation, at ANY depth.
///
/// # Why this is not a hand-written match any more
///
/// It was, and it recursed into `AnnotatedGroup` and nothing else, sending
/// every other container to `_ => {}`. So a quotation inside a retrace, a
/// phonological group, a sign group, or another quotation was invisible:
///
/// ```text
/// *CHI:	“I said “hello” there” .      reported E372
/// *CHI:	“a <“b”> [/] c” .             reported NOTHING
/// ```
///
/// The model for the second is quotation -> retrace -> quotation, which is the
/// nesting this rule exists to reject. Classifying through [`ContentStructure`]
/// means the predicate descends wherever the rest of the crate descends, and
/// `GroupRef` is what lets it still tell a QUOTATION from any other container,
/// which a bare `&BracketedContent` could not.
fn has_nested_quotation(items: &[BracketedItem]) -> bool {
    items.iter().any(|item| {
        // Bound once. Calling `structure()` again inside the arm re-derives
        // what the match just destructured, and leaves two calls that a reader
        // must keep in agreement.
        let structure = item.structure();
        match structure {
            ContentStructure::Group(GroupRef::Quotation(_)) => true,
            ContentStructure::Retrace(_) | ContentStructure::Group(_) => structure
                .enclosed()
                .is_some_and(|content| has_nested_quotation(&content.content)),
            ContentStructure::Word(_) | ContentStructure::Leaf(_) => false,
        }
    })
}

/// Regression tests for main-tier structural checks in this module.
#[cfg(test)]
mod tests {
    use crate::ErrorCollector;
    use crate::model::{MainTier, SpeakerCode, Terminator, UtteranceContent, Word};
    use crate::validation::{Validate, ValidationContext};
    use std::collections::HashSet;

    fn participants(ids: &[&'static str]) -> HashSet<SpeakerCode> {
        ids.iter().map(|id| SpeakerCode::new(*id)).collect()
    }

    #[test]
    fn missing_terminator_errors_outside_ca_mode() {
        let content = vec![UtteranceContent::Word(Box::new(Word::new_unchecked(
            "hi", "hi",
        )))];
        let main = MainTier::new("CHI", content, Option::<Terminator>::None);
        let ctx = ValidationContext::new()
            .with_participant_ids(participants(&["CHI"]))
            .with_ca_mode(false);
        let errors = ErrorCollector::new();
        main.validate(&ctx, &errors);
        let error_vec = errors.into_vec();
        // E305 (`MissingTerminator`), not E304, which is `MissingSpeaker`
        // and is reserved for tree-sitter recovery cases where the `*` or
        // the speaker token itself is absent.
        assert!(
            error_vec.iter().any(|e| e.code.as_str() == "E305"),
            "Expected E305 when terminator missing outside CA mode, got: {error_vec:?}"
        );
        assert!(
            error_vec.iter().all(|e| e.code.as_str() != "E304"),
            "E304 must not be emitted for missing terminator"
        );
    }

    #[test]
    fn missing_terminator_allowed_in_ca_mode() {
        let content = vec![UtteranceContent::Word(Box::new(Word::new_unchecked(
            "hi", "hi",
        )))];
        let main = MainTier::new("CHI", content, Option::<Terminator>::None);
        let ctx = ValidationContext::new()
            .with_participant_ids(participants(&["CHI"]))
            .with_ca_mode(true);
        let errors = ErrorCollector::new();
        main.validate(&ctx, &errors);
        let error_vec = errors.into_vec();
        assert!(
            error_vec.iter().all(|e| e.code.as_str() != "E305"),
            "CA mode should not emit E305 for missing terminator"
        );
    }
}
