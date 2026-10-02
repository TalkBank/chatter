//! `--code` and `--suppress` values, parsed by clap into typed values.
//!
//! Both flags name error codes case-insensitively (`e241`, `E241`), and
//! `--suppress` also takes named groups (`xphon`). Each parse is a clap
//! `value_parser`, so a value that names nothing real is a usage error
//! (exit 2) naming that value, before the command runs: a typo never
//! reaches a command as a string it has to remember to check. `chatter
//! fix --code` in particular must never let a typo widen a batch run to
//! every code in the catalog.

use talkbank_model::ErrorCode;

/// A `--code` or `--suppress` value that names no known error code.
#[derive(Clone, Debug, thiserror::Error)]
#[error("{value:?} is not a known error code; see `chatter validate --list-checks`")]
pub(crate) struct UnknownErrorCode {
    /// The value as typed.
    value: String,
}

/// Parse one `--code` value: a known error code, matched
/// case-insensitively.
pub(crate) fn parse_error_code(raw: &str) -> Result<ErrorCode, UnknownErrorCode> {
    ErrorCode::parse_exact(&raw.to_uppercase()).ok_or_else(|| UnknownErrorCode {
        value: raw.to_owned(),
    })
}

/// A named `--suppress` group: a user-friendly shorthand for a fixed set of
/// error codes. Closed on purpose (a `match` on this type must be
/// exhaustive), so adding a group is a compile-time-visible decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SuppressionGroup {
    /// The whole Phon `%x` dependent-tier validation surface (`%xmodsyl`,
    /// `%xphosyl`, `%xphoaln`, `%xphoint`). Opt-out only; this validation
    /// runs by default.
    Xphon,
}

impl SuppressionGroup {
    /// The error codes this group expands to. For [`Self::Xphon`] this is
    /// `talkbank_model::XPHON_ERROR_CODES` (beside the `ErrorCode`
    /// definitions), so the group cannot drift from what the validator
    /// emits.
    pub(crate) fn codes(self) -> &'static [ErrorCode] {
        match self {
            Self::Xphon => talkbank_model::XPHON_ERROR_CODES,
        }
    }
}

/// One `--suppress` value: a named group or a single error code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SuppressionSelector {
    /// A named group, e.g. `xphon`.
    Group(SuppressionGroup),
    /// A single error code, e.g. `E736`.
    Code(ErrorCode),
}

/// A `--suppress` value that names neither a known group nor a known error
/// code, as typed.
#[derive(Clone, Debug, thiserror::Error)]
#[error(
    "{value:?} is not a known suppression group (xphon) or a known error code; \
     see `chatter validate --list-checks` for valid codes"
)]
pub(crate) struct UnknownSuppressionValue {
    value: String,
}

/// Parse one `--suppress` value. Group names and error codes are both
/// matched case-insensitively; a value that is neither is refused, never
/// guessed at.
pub(crate) fn parse_suppression(raw: &str) -> Result<SuppressionSelector, UnknownSuppressionValue> {
    match raw.to_lowercase().as_str() {
        "xphon" => Ok(SuppressionSelector::Group(SuppressionGroup::Xphon)),
        _ => parse_error_code(raw)
            .map(SuppressionSelector::Code)
            .map_err(|_| UnknownSuppressionValue {
                value: raw.to_owned(),
            }),
    }
}

/// The error codes `--suppress` hides: groups expanded to their member
/// codes, sorted by code and without repeats. Built only by [`Self::of`], so
/// "sorted, without repeats" is how the value was made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SuppressedCodes(Vec<ErrorCode>);

impl SuppressedCodes {
    /// The codes the `--suppress` selectors name.
    pub(crate) fn of(selectors: &[SuppressionSelector]) -> Self {
        let mut codes = Vec::new();
        for selector in selectors {
            match selector {
                SuppressionSelector::Group(group) => codes.extend_from_slice(group.codes()),
                SuppressionSelector::Code(code) => codes.push(*code),
            }
        }
        codes.sort_unstable_by_key(|code| code.as_str());
        codes.dedup();
        Self(codes)
    }

    /// The codes, sorted, each once.
    pub(crate) fn codes(&self) -> &[ErrorCode] {
        &self.0
    }

    /// Whether nothing is suppressed.
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::XPHON_ERROR_CODES;

    #[test]
    fn a_known_code_parses_case_insensitively_and_an_unknown_one_is_refused() {
        assert_eq!(
            parse_error_code("e241").expect("known"),
            ErrorCode::IllegalUntranscribed
        );
        assert_eq!(
            parse_error_code("E241").expect("known"),
            ErrorCode::IllegalUntranscribed
        );
        for bogus in ["E9999", "bogus"] {
            let refused = parse_error_code(bogus).expect_err("unknown");
            assert!(refused.to_string().contains(bogus));
        }
    }

    #[test]
    fn suppression_values_are_groups_or_codes_and_nothing_else() {
        assert_eq!(
            parse_suppression("XPHON").expect("group"),
            SuppressionSelector::Group(SuppressionGroup::Xphon)
        );
        assert_eq!(
            parse_suppression("e316").expect("code"),
            SuppressionSelector::Code(ErrorCode::UnparsableContent)
        );
        for bogus in ["notagroup", "E9999"] {
            let refused = parse_suppression(bogus).expect_err("unknown");
            assert!(refused.to_string().contains(bogus));
        }
    }

    #[test]
    fn groups_expand_and_codes_pass_through_sorted_once() {
        assert!(
            SuppressedCodes::of(&[]).is_empty(),
            "nothing suppressed by default"
        );
        let mut xphon_sorted = XPHON_ERROR_CODES.to_vec();
        xphon_sorted.sort_unstable_by_key(|code| code.as_str());
        let xphon = SuppressedCodes::of(&[SuppressionSelector::Group(SuppressionGroup::Xphon)]);
        assert_eq!(xphon.codes(), xphon_sorted);
        let mixed = SuppressedCodes::of(&[
            SuppressionSelector::Group(SuppressionGroup::Xphon),
            SuppressionSelector::Code(ErrorCode::UnparsableContent),
            SuppressionSelector::Group(SuppressionGroup::Xphon),
        ]);
        assert_eq!(mixed.codes().len(), XPHON_ERROR_CODES.len() + 1);
        assert!(mixed.codes().contains(&ErrorCode::UnparsableContent));
        assert!(
            mixed
                .codes()
                .windows(2)
                .all(|pair| pair[0].as_str() < pair[1].as_str()),
            "sorted, each once"
        );
    }
}
