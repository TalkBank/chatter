//! Shared resolution of raw CLI strings into [`ErrorCode`]s.
//!
//! `chatter fix --code` and `chatter validate --suppress` both parse the
//! same shape of input (a case-insensitive [`ErrorCode`] string), and each
//! used to carry its own copy of that parse (`to_uppercase()` plus
//! [`ErrorCode::parse_exact`]). Two copies is how a 2026-07-31 review
//! caught `chatter fix --code` silently falling back to "every code" on a
//! typo: the drift was never in the shared parse itself, but a shared parse
//! is what makes that fact checkable in one place instead of two.
//!
//! This module resolves; it does not decide what an unrecognized value
//! means. `chatter fix` fails closed on one (see `fix::resolve_requested_codes`);
//! `chatter validate --suppress` also fails closed today, but via its own
//! private `SuppressionSelector` wrapper
//! that additionally recognizes named groups (`xphon`). Each caller keeps
//! its own policy; only the per-value parse is shared.

use std::collections::HashSet;

use talkbank_model::ErrorCode;

/// Resolve one raw `--code`/`--suppress` value against the real
/// [`ErrorCode`] set.
///
/// Matching is case-insensitive (`e241`, `E241`); existence is not: a
/// value that names no real code returns `None` rather than being
/// guessed at or silently matched against nothing.
pub(crate) fn resolve_error_code(raw: &str) -> Option<ErrorCode> {
    ErrorCode::parse_exact(&raw.to_uppercase())
}

/// The result of resolving a batch of raw values via
/// [`resolve_error_codes`]: what resolved, and what did not.
///
/// Carries both halves rather than picking a policy for an unrecognized
/// value itself (fail closed, warn-and-drop, silently ignore): that
/// decision belongs to each caller, which is the whole reason this type
/// has two fields instead of returning `Result<HashSet<ErrorCode>, _>`
/// and discarding the values that failed to parse.
#[derive(Debug, Clone, Default)]
pub(crate) struct ResolvedErrorCodes {
    /// Every raw value that named a real error code, deduplicated.
    pub codes: HashSet<ErrorCode>,
    /// Every raw value, verbatim as typed (before case normalization),
    /// that named no real error code.
    pub unrecognized: Vec<String>,
}

/// Resolve every value in `raw` via [`resolve_error_code`].
pub(crate) fn resolve_error_codes(raw: &[String]) -> ResolvedErrorCodes {
    let mut resolved = ResolvedErrorCodes::default();
    for value in raw {
        match resolve_error_code(value) {
            Some(code) => {
                resolved.codes.insert(code);
            }
            None => resolved.unrecognized.push(value.clone()),
        }
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_a_known_code_case_insensitively() {
        assert_eq!(
            resolve_error_code("e241"),
            Some(ErrorCode::IllegalUntranscribed)
        );
        assert_eq!(
            resolve_error_code("E241"),
            Some(ErrorCode::IllegalUntranscribed)
        );
    }

    #[test]
    fn rejects_an_unrecognized_value() {
        assert_eq!(resolve_error_code("E9999"), None);
        assert_eq!(resolve_error_code("bogus"), None);
    }

    #[test]
    fn batch_resolution_splits_known_from_unrecognized() {
        let resolved = resolve_error_codes(&["E241".to_owned(), "bogus".to_owned()]);
        assert!(resolved.codes.contains(&ErrorCode::IllegalUntranscribed));
        assert_eq!(resolved.unrecognized, vec!["bogus".to_owned()]);
    }
}
