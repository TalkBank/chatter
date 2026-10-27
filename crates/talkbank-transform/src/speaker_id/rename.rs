//! Admission for identity-preserving speaker renames, without role remapping.
use crate::speaker_token::{SpeakerToken, SpeakerTokenError};
use talkbank_model::{ParseError, SpeakerCode};

/// A rename admitted against the caller's complete speaker inventory.
///
/// The caller must establish source completeness and bind edits to that source.
/// Unlike `apply_mapping`, this operation neither drops turns nor changes roles.
#[derive(Debug)]
pub struct SpeakerRename {
    original: SpeakerCode,
    replacement: SpeakerToken,
}

/// An explicit no-op or an admitted change of spelling.
#[derive(Debug)]
pub enum SpeakerRenameDecision {
    /// The requested identity is already spelled this way.
    Unchanged,
    /// The replacement is valid and does not merge identities.
    Rename(SpeakerRename),
}

/// Why an identity-preserving rename cannot be admitted.
#[derive(Debug, thiserror::Error)]
pub enum SpeakerRenameError {
    /// No speaker token can spell this replacement at every reference site.
    #[error(
        "replacement must be a nonempty speaker token: ASCII letters, digits, _, +, - or apostrophe"
    )]
    UnspellableReplacement,
    /// Shared model validation rejected the replacement; retain its findings.
    #[error("replacement violates speaker-code constraints: {0:?}")]
    InvalidReplacement(Vec<ParseError>),
    /// The inventory does not contain the identity being renamed.
    #[error("original speaker does not occur in the supplied inventory")]
    MissingOriginal,
    /// Another identity already has the requested spelling.
    #[error("replacement is already used by another speaker")]
    Collision,
}

impl SpeakerRename {
    /// Admit a rename against all observed speaker codes, including references
    /// without declarations. Matching is exact and case-sensitive.
    pub fn admit<'a>(
        original: &str,
        replacement: &str,
        observed: impl IntoIterator<Item = &'a str>,
    ) -> Result<SpeakerRenameDecision, SpeakerRenameError> {
        let codes: std::collections::HashSet<_> = observed.into_iter().collect();
        if original.is_empty() || !codes.contains(original) {
            return Err(SpeakerRenameError::MissingOriginal);
        }
        if original == replacement {
            return Ok(SpeakerRenameDecision::Unchanged);
        }
        let admitted =
            SpeakerToken::admit(SpeakerCode::new(replacement)).map_err(|error| match error {
                SpeakerTokenError::Unspellable => SpeakerRenameError::UnspellableReplacement,
                SpeakerTokenError::Invalid(errors) => {
                    SpeakerRenameError::InvalidReplacement(errors)
                }
            })?;
        if codes.contains(replacement) {
            return Err(SpeakerRenameError::Collision);
        }
        Ok(SpeakerRenameDecision::Rename(Self {
            original: SpeakerCode::new(original),
            replacement: admitted,
        }))
    }

    /// Identity whose source occurrences may be replaced.
    pub fn original(&self) -> &str {
        self.original.as_str()
    }

    /// Valid, unused replacement spelling.
    pub fn replacement(&self) -> &str {
        self.replacement.as_code().as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_admission_preserves_identity_and_token_boundaries() {
        for target in [
            "", "A B", "CH:I", "A|B", "\n@End", "\u{15}", "CHILDREN", "🦀", "A.B", "A\"B",
        ] {
            assert!(
                SpeakerRename::admit("CHI", target, ["CHI", "MOT"]).is_err(),
                "{target:?}"
            );
        }
        for target in ["KID", "A_B", "A+B", "A-B", "A'B", "kid", "123", "ABC1234"] {
            let SpeakerRenameDecision::Rename(rename) =
                SpeakerRename::admit("CHI", target, ["CHI", "MOT"]).unwrap()
            else {
                panic!("expected rename")
            };
            assert_eq!(rename.original(), "CHI");
            assert_eq!(rename.replacement(), target);
        }
        assert!(matches!(
            SpeakerRename::admit("CHI", "MOT", ["CHI", "MOT"]),
            Err(SpeakerRenameError::Collision)
        ));
        assert!(matches!(
            SpeakerRename::admit("CHI", "CHI", ["CHI"]),
            Ok(SpeakerRenameDecision::Unchanged)
        ));
        assert!(matches!(
            SpeakerRename::admit("CHI", "CHI", ["MOT"]),
            Err(SpeakerRenameError::MissingOriginal)
        ));
        let Err(SpeakerRenameError::InvalidReplacement(errors)) =
            SpeakerRename::admit("CHI", "CHILDREN", ["CHI"])
        else {
            panic!("retain model diagnostics")
        };
        assert!(!errors.is_empty());
    }
}
