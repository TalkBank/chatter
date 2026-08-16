//! Validation for a dependent tier that declares no content (E756).
//!
//! One rule lives here: a tier line whose payload is absent or whitespace-only
//! asserts an annotation that is not there.
//!
//! History: born as W601 firing at Error severity (the warning-prefixed code
//! was the bug; renumbered 2026-07-16, rejection unchanged), and wired ONLY to
//! user-defined `%x*` tiers because `TextTier::content` could not represent an
//! empty standard tier. When it could (2026-08-15), an empty `%eng:` became
//! representable and unjudged: re2c reported such a file VALID where
//! tree-sitter rejected it through E330, an `_auto` stub with no description.
//! The maintainer ruling widened E756 rather than write E330's spec or add a
//! third code, on the grounds that E756's rule ("a tier whose content is empty
//! declares nothing") was never `%x`-specific and only its NAME was.
//!
//! A sibling W602 check (deprecated `%xLABEL` where LABEL was a standard tier)
//! was DELETED 2026-07-16 as dead code: the Phon `%x`-tier fold routes every
//! known label to typed tier parsers, so labels like `xpho` never reach this
//! path.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

/// Report a dependent tier that declares no content.
///
/// `label` is the tier label as parsed, without `%` or colon: a `%xtst:` tier
/// arrives as `xtst` and a `%eng:` tier as `eng`. Callers get it from
/// [`crate::model::DependentTier::kind`], and decide WHETHER to call by asking
/// [`crate::model::DependentTier::empty_content_span`], which owns the question
/// of whether a given tier kind can even be empty.
///
/// `pub(crate)` since 2026-08-16, and that is the point rather than tidiness.
/// It was public because the tree-sitter `%x` lowering called it from the
/// PARSE path, which is how an empty `%xtst:` came to be reported without ever
/// being added to the model: the parser both judged the tier and dropped it.
/// With the decision back where it belongs (the validator asks
/// `empty_content_span`), no caller outside this crate remains, and closing the
/// door is what stops the next one reopening it. Leaving a better primitive
/// public only moves the duplication to the next caller.
pub(crate) fn check_dependent_tier_content(
    label: &str,
    span: crate::Span,
    errors: &impl ErrorSink,
) {
    let mut err = ParseError::new(
        ErrorCode::EmptyDependentTier,
        Severity::Error,
        SourceLocation::at_offset(span.start as usize),
        ErrorContext::new("", 0..0, ""),
        format!("Dependent tier %{label} has no content"),
    )
    .with_suggestion(
        "A dependent tier must carry the annotation it declares; remove the empty tier line otherwise",
    );
    err.location.span = span;
    errors.report(err);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCollector;

    /// The report is well formed for the tier it names.
    ///
    /// The emptiness DECISION no longer lives here: it moved to
    /// `DependentTier::empty_content_span` when E756 was widened past `%x*` on
    /// 2026-08-15, because only the tier type knows whether it can be empty at
    /// all. What survives in this module is the reporting, so what survives
    /// here are the assertions about the diagnostic. The tests that fed this
    /// function `None` / `""` / `" \t"` moved with the decision.
    ///
    /// The `%xfoo` / `%xxfoo` pair records a real past bug: the label already
    /// carries its `x`, and an older format string double-prefixed it.
    #[test]
    fn the_report_names_the_tier_with_one_percent_prefix() {
        let errors = ErrorCollector::new();
        check_dependent_tier_content("xfoo", crate::Span::DUMMY, &errors);
        let reported = errors.into_vec();
        assert_eq!(reported.len(), 1);
        assert_eq!(reported[0].code, ErrorCode::EmptyDependentTier);
        assert_eq!(reported[0].severity, Severity::Error);
        assert!(reported[0].message.contains("%xfoo"));
        assert!(!reported[0].message.contains("%xxfoo"));
    }

    /// A standard tier reports under the same code and reads naturally.
    ///
    /// POLICY, not an invariant a type could absorb: "a dependent tier must
    /// carry content" is a CHAT rule with a real alternative (accept it, as
    /// CLAN does), so the validator states it rather than the model forbidding
    /// it. The model deliberately CAN represent an empty tier; before it
    /// could, each parser invented its own way round and the two disagreed.
    #[test]
    fn a_standard_tier_reports_under_the_same_code() {
        let errors = ErrorCollector::new();
        check_dependent_tier_content("eng", crate::Span::DUMMY, &errors);
        let reported = errors.into_vec();
        assert_eq!(reported.len(), 1);
        assert_eq!(reported[0].code, ErrorCode::EmptyDependentTier);
        assert!(reported[0].message.contains("%eng"));
    }
}
