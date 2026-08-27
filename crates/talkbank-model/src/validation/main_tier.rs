//! Structural validation rules for main tiers.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Delimiters>

// A CHAT speaker prefix is followed by a literal TAB, so the tabs in the
// examples below are the format being described, not indentation. Corrupting
// them to spaces would make the doc show invalid CHAT.
#![allow(clippy::tabs_in_doc_comments)]
// Design rule 3, enforced by the compiler rather than by prose: a `_` arm over
// a content enum means a future variant compiles clean and answers wrong.
// Added per file as each is cleaned; `audit_content_catch_alls` lists the rest.
#![deny(clippy::wildcard_enum_match_arm)]
mod word_recursion;

pub(crate) use word_recursion::validate_words_at_every_depth;

use crate::model::{
    AnnotatedContentAnnotations, ContentStructure, Descend, GroupRef, MainTier, UtteranceContent,
};
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

/// Report every UNRECOGNISED scoped annotation, on any construct, at any depth.
///
/// # Why a traversal and not a trait impl
///
/// It was `impl<T: Validate> Validate for Annotated<T>`, which reached a
/// construct's annotations only when its PAYLOAD implemented `Validate`.
/// `Word` does; `Group`, `Event`, `Action` and `Quotation` do not. So whether
/// an unrecognised annotation was reported depended on a property of something
/// else entirely, which is a policy nobody chose and nothing stated.
///
/// It went unseen because the default backend refuses those inputs at parse
/// time and never builds the node. Under `--parser=re2c`, which does build it,
/// 0.16.0's unrecognised-annotation fix reached the word and turned the other
/// five hosts SILENT: `<hello world> [qq] .`, `&=laughs [qq] .`, `0 [qq] .`,
/// `“hello” [qq] .` and `hello (.) [qq] .` all validated clean where v0.15.0
/// reported E321. A backend that answers "valid" on input the authority
/// refuses is useless as the specification oracle it exists to be.
///
/// [`ContentStructure::scoped_annotations`] answers for every variant already,
/// so asking the owner removes the coupling rather than adding four `Validate`
/// impls with nothing else to say.
pub(crate) fn report_unknown_annotations(main_tier: &MainTier, errors: &impl ErrorSink) {
    for content_item in main_tier.content.content.iter() {
        content_item.structure().walk(&mut |item| {
            let annotations = item.scoped_annotations();
            if !annotations.is_empty() {
                // A leaf records no span of its own, so its annotations are
                // reported against the tier. Saying so beats inventing a
                // position for them.
                let span = item.span().unwrap_or(main_tier.span);
                AnnotatedContentAnnotations::report_unknown_markers(annotations, span, errors);
            }
            Descend::Into
        });
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
    for content_item in main_tier.content.content.iter() {
        report_nested_quotations(content_item.structure(), main_tier, errors);
    }
}

/// Report every quotation, at ANY depth, that encloses a further quotation.
///
/// # Why this descends instead of matching one variant
///
/// It was `if let UtteranceContent::Quotation(q) = content_item`, at the top
/// level only. That is the same defect [`has_nested_quotation`] documents one
/// level down, in the OTHER half of the same relation, and it outlived that
/// fix.
///
/// A quotation carrying a retrace marker does not lower to
/// `UtteranceContent::Quotation`; it lowers to a RETRACE wrapping the
/// quotation. So the `if let` never matched and the rule silently stopped
/// applying, on the default backend only:
///
/// ```text
/// *CHI:	“a “b” c” .              reported E372
/// *CHI:	“a “b” c” [//] hello .    reported NOTHING, and re2c reported E372
/// ```
///
/// The rule holds between two quotations, so it has to descend on BOTH sides:
/// to the one that encloses and to the one enclosed. Both halves classify
/// through [`ContentStructure`] now, so neither can be hidden by a wrapper
/// again. Spec examples 4 and 5 of `E372.md` are the two directions.
fn report_nested_quotations(
    structure: ContentStructure<'_>,
    main_tier: &MainTier,
    errors: &impl ErrorSink,
) {
    structure.walk(&mut |item| match item {
        ContentStructure::Group(GroupRef::Quotation(_)) => {
            if encloses_a_quotation(item) {
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
            // SKIP, not `Into`, and this is the whole reason `walk` carries a
            // three-state answer: the predicate above already covers every
            // depth beneath this quotation, so descending as well would report
            // the same nesting once per level of it.
            Descend::Skip
        }
        ContentStructure::Word(_)
        | ContentStructure::Retrace(_)
        | ContentStructure::Group(_)
        | ContentStructure::Leaf(_) => Descend::Into,
    });
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
fn encloses_a_quotation(container: ContentStructure<'_>) -> bool {
    let Some(content) = container.enclosed() else {
        return false;
    };
    let mut found = false;
    for item in content.content.iter() {
        item.structure().walk(&mut |item| match item {
            ContentStructure::Group(GroupRef::Quotation(_)) => {
                found = true;
                Descend::Stop
            }
            ContentStructure::Word(_)
            | ContentStructure::Retrace(_)
            | ContentStructure::Group(_)
            | ContentStructure::Leaf(_) => Descend::Into,
        });
        if found {
            break;
        }
    }
    found
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
