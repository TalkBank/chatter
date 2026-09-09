//! The two language-metadata claims that are about a STATE, not a parse.
//!
//! # Why fifteen tests left
//!
//! They built each `Utterance` by hand and wrote the CHAT they meant in a
//! comment beside it: `// *CHI:\tni3 hello@s .` above
//! `Word::new_unchecked("hello@s", "hello")` with `lang = Some(Shortcut)`. The
//! comment and the struct were two statements of one fact held together by
//! nothing. All fifteen are now
//! `talkbank-parser-tests/tests/integration/utterance_metadata_from_source.rs`,
//! where the comment IS the input and the utterance is what the parser builds;
//! every assertion travelled (resolved language and its tag, per-word language
//! AND provenance, record count and order, code-switch detection, per-language
//! counts, the alignment error codes and severities). A `#[cfg(test)] mod`
//! here cannot parse (second instantiation of the crate); an integration test
//! under `tests/` with a dev-dependency cycle could, and that crate is the
//! cheaper home.
//!
//! Two things the move had to spell differently, because the originals built
//! shapes no parse produces as written: a bare `Group` (every parsed group
//! carries an annotation, so `<...> [!]` stands in, and the walker descends
//! the same way) and `UtteranceContent::Action(Action::new())` (the parser
//! builds it from the `0` zero marker).
//!
//! # What stays
//!
//! Two tests of `UtteranceLanguage::Uncomputed` and
//! `UtteranceLanguageMetadata::default()`: a state that means "analysis
//! pending", reached by construction rather than by any parse, whose
//! validation tag must be a warning and not an error. Nothing here builds an
//! utterance.

use crate::model::{UtteranceLanguage, UtteranceLanguageMetadata};

/// `UtteranceLanguage::Uncomputed` maps to warning-level validation state.
///
/// This keeps "not yet computed" distinct from true parse/semantic errors.
#[test]
fn test_utterance_language_uncomputed_is_warning_tag() {
    let state = UtteranceLanguage::Uncomputed;
    assert_eq!(
        crate::model::ValidationTagged::validation_tag(&state),
        crate::model::ValidationTag::Warning
    );
    assert!(crate::model::ValidationTagged::is_validation_warning(
        &state
    ));
}

/// Default language metadata state starts as uncomputed warning.
///
/// The default communicates "analysis pending" rather than invalid transcript content.
#[test]
fn test_language_metadata_state_defaults_to_uncomputed_warning() {
    let state = UtteranceLanguageMetadata::default();
    assert!(matches!(state, UtteranceLanguageMetadata::Uncomputed));
    assert_eq!(
        crate::model::ValidationTagged::validation_tag(&state),
        crate::model::ValidationTag::Warning
    );
}
