//! Validation rules for user-defined dependent tiers (`%x*`).
//!
//! One rule lives here: a user-defined tier must carry content (E756).
//! History: the check was born as W601 firing at Error severity (the
//! warning-prefixed code was the bug; renumbered 2026-07-16, rejection
//! unchanged). A sibling W602 check (deprecated `%xLABEL` where LABEL
//! was a standard tier) was DELETED the same day as dead code: the Phon
//! `%x`-tier fold routes every known label to typed tier parsers, so
//! labels like `xpho` never reach this user-defined path (and the old
//! branch compared against bare names like `pho`, which production
//! labels, `x`-prefixed, could never equal).
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};

/// Validate one user-defined `%x*` tier payload.
///
/// `label` is the tier label as parsed, INCLUDING the `x` prefix (a
/// `%xtst:` tier arrives as `xtst`).
pub fn check_user_defined_tier_content(
    label: &str,
    content: &str,
    span: Span,
    errors: &impl ErrorSink,
) {
    // E756: a tier declaring no content asserts an annotation that is
    // not there and fails to make sense.
    if content.chars().all(|ch| ch.is_whitespace()) {
        let mut err = ParseError::new(
            ErrorCode::EmptyUserDefinedTier,
            Severity::Error,
            SourceLocation::at_offset(span.start as usize),
            ErrorContext::new(content, 0..content.len(), content),
            format!("User-defined tier %{label} has no content"),
        )
        .with_suggestion(
            "User-defined tiers should contain custom analysis/annotation data; remove the empty tier line otherwise",
        );
        err.location.span = span;
        errors.report(err);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCollector;

    #[test]
    fn test_e756_empty_tier() {
        let errors = ErrorCollector::new();
        check_user_defined_tier_content("xfoo", "", Span::DUMMY, &errors);
        let error_vec = errors.into_vec();
        assert_eq!(error_vec.len(), 1);
        assert_eq!(error_vec[0].code, ErrorCode::EmptyUserDefinedTier);
        assert_eq!(error_vec[0].severity, Severity::Error);
        // The message names the tier with a single % prefix (the label
        // already carries the x; the old format double-prefixed to %xx...).
        assert!(error_vec[0].message.contains("%xfoo"));
        assert!(!error_vec[0].message.contains("%xxfoo"));
    }

    #[test]
    fn test_e756_whitespace_only_tier() {
        let errors = ErrorCollector::new();
        check_user_defined_tier_content("xtst", " \t", Span::DUMMY, &errors);
        let error_vec = errors.into_vec();
        assert_eq!(error_vec.len(), 1);
        assert_eq!(error_vec[0].code, ErrorCode::EmptyUserDefinedTier);
    }

    #[test]
    fn test_valid_user_tier_no_errors() {
        // Any user-defined tier with content is valid; the deprecated
        // %xLABEL check (W602) was deleted as dead code, so known-standard
        // labels draw nothing here (their typed parsers own them upstream).
        for label in ["xfoo", "xpho", "xmor", "xcustom"] {
            let errors = ErrorCollector::new();
            check_user_defined_tier_content(label, "test content", Span::DUMMY, &errors);
            assert!(
                errors.into_vec().is_empty(),
                "user-defined tier {label} with content must be valid"
            );
        }
    }
}
