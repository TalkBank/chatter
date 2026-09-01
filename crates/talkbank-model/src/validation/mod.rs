//! Validation module for CHAT data model
//!
//! Validation is performed via **methods on model types**:
//!
//! ```ignore
//! use talkbank_model::{ChatFile, ErrorCollector};
//!
//! let errors = ErrorCollector::new();
//! chat_file.validate(&errors);
//! let error_vec = errors.into_vec();
//! ```
//!
//! ## Public API
//!
//! - **`ChatFile::validate()`** - Validate entire file with streaming errors
//! - **`ChatFile::validate_with_alignment()`** - Validate with tier alignment
//! - **`Validate` trait** - Implemented by all model types for uniform validation
//! - **`ValidationContext`** - File-level context passed down validation hierarchy
//!
//! ## Location Tracking Limitation
//!
//! Currently, validation errors use placeholder source locations `(1, 1)` because
//! the domain model (Word, MainTier, etc.) does not carry source location information.
//! This is a deliberate design choice for the current phase:
//! - Domain model remains simple and focused on semantics
//! - Location tracking will be added in Phase 4 (Validation Engine) when we design
//!   a comprehensive approach that integrates with parsing and editor integration
//!
//! For now, validation errors still provide useful context through ErrorContext
//! (the actual text, column ranges, and expectations).
//!
//! ## Design Principles
//!
//! - Validation is separate from parsing (parse, do not validate)
//! - Add new errors using TDD with focused tests
//! - Stream diagnostics via `ErrorSink` without early returns
//!
//! ## Validation Diagnostics Rules
//!
//! - Do not rely on fabricated values from parser recovery when validating semantics
//! - If source context is unknown, represent that explicitly instead of fake sentinel spans/content
//! - Keep validation errors structured and miette-friendly for consistent source-located rendering
//! - Alignment-related validation must honor parse-taint and skip mismatches for tainted domains
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

// Module declarations
#[cfg(feature = "async")]
pub mod async_runtime;
mod bullet;
mod chat_file;
mod config;
mod context;
#[doc(hidden)]
pub mod cross_utterance;
pub(crate) mod header;
#[doc(hidden)]
pub mod main_tier;
pub(crate) mod retrace;
mod speaker;
mod state;
pub(crate) mod temporal;
mod r#trait;
mod unparsed_tier;
pub(crate) mod utterance;
pub(crate) mod word;

// Re-export public API
pub use config::RuleSelection;
pub use context::{SharedValidationData, ValidationContext, language_allows_numbers};
pub use state::{NotValidated, Validated, ValidationState};
pub use r#trait::Validate;

// Re-export async helpers when feature is enabled
#[cfg(feature = "async")]
pub use crate::AsyncChannelErrorSink;
#[cfg(feature = "async")]
pub use async_runtime::{AsyncValidationError, validate_async, validate_with_rules_async};
pub use word::language::LanguageResolution;
pub use word::{GoverningMark, GoverningMarkKind, LanguageResolutionOutcome};

// Public bullet validation function
pub(crate) use bullet::check_bullet;
pub use bullet::check_bullet_monotonicity;
pub(crate) use speaker::check_speaker_id;
pub(crate) use unparsed_tier::check_dependent_tier_content;

// Re-export tests if they exist
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCollector;
    use crate::model::{Annotated, ContentAnnotation, Word};
    use crate::validation::Validate;

    /// An unrecognised annotation on a word is reported, EXACTLY ONCE.
    ///
    /// Drives a real `MainTier`, not a bare `Annotated<Word>`, because the
    /// rule moved there; see `main_tier::report_unknown_annotations`.
    ///
    /// The COUNT is the assertion, in both directions. A second emitter left
    /// standing shows up here as two, and that is not hypothetical: deleting
    /// the first one missed `ReplacedWordAnnotations`, which doubled E207 on
    /// a replaced word until an integration test counted it.
    #[test]
    fn an_unknown_annotation_on_a_word_is_reported_exactly_once() {
        use crate::ErrorCode;
        use crate::model::{MainTier, Terminator, UtteranceContent};

        let word = Annotated::with_one(
            Word::new_unchecked("hello [::: stuff]", "hello"),
            ContentAnnotation::Unknown(crate::model::ScopedUnknown {
                marker: ":::".into(),
                text: "stuff".into(),
            }),
        );
        // No terminator: the missing-terminator diagnostic is filtered out
        // below, and building one here would test the terminator model rather
        // than this rule.
        let main = MainTier::new(
            "CHI",
            vec![UtteranceContent::AnnotatedWord(Box::new(word))],
            Option::<Terminator>::None,
        );

        let errors = ErrorCollector::new();
        let context = ValidationContext::new()
            .with_participant_ids(std::iter::once(crate::model::SpeakerCode::new("CHI")).collect());
        main.validate(&context, &errors);

        let reported: Vec<_> = errors
            .into_vec()
            .into_iter()
            .filter(|e| e.code == ErrorCode::UnknownAnnotation)
            .collect();
        assert_eq!(
            reported.len(),
            1,
            "exactly one E207, not zero (unreported) and not two (reported by \
             both the old impl and the new traversal); got {reported:?}"
        );
        assert!(
            reported[0].message.contains(":::"),
            "the message must name the marker as written. Got: {}",
            reported[0].message
        );
    }

    /// Verifies word validation no errors.
    #[test]
    fn test_word_validation_no_errors() {
        // Build valid word programmatically
        // Note: Don't wrap in Annotated unless there are actual annotations,
        let word = Word::new_unchecked("hello", "hello");

        let errors = ErrorCollector::new();
        let context = ValidationContext::new();
        word.validate(&context, &errors);
        let error_vec = errors.into_vec();

        // Should have no errors
        assert_eq!(error_vec.len(), 0);
    }
}
