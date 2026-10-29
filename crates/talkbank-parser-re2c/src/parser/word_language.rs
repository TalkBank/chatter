//! Admission of word-level language markers into the shared model.

use crate::token::WordLanguagePayload;

/// Admit a present lexer payload before it enters the native word carrier.
/// Shortcut identity is explicit; empty code text remains a rejection.
pub(super) fn admit_word_language(
    payload: WordLanguagePayload<'_>,
) -> Result<talkbank_model::model::WordLanguageMarker, talkbank_model::model::LanguageCodeError> {
    use talkbank_model::model::{LanguageCode, WordLanguageMarker};
    match payload {
        WordLanguagePayload::Shortcut => Ok(WordLanguageMarker::Shortcut),
        WordLanguagePayload::Codes(codes) if codes.contains('+') => codes
            .split('+')
            .map(LanguageCode::new)
            .collect::<Result<Vec<_>, _>>()
            .map(WordLanguageMarker::Multiple),
        WordLanguagePayload::Codes(codes) if codes.contains('&') => codes
            .split('&')
            .map(LanguageCode::new)
            .collect::<Result<Vec<_>, _>>()
            .map(WordLanguageMarker::Ambiguous),
        WordLanguagePayload::Codes(code) => {
            LanguageCode::new(code).map(WordLanguageMarker::Explicit)
        }
    }
}

#[cfg(test)]
mod language_admission_tests {
    use super::{WordLanguagePayload, admit_word_language};

    #[test]
    fn empty_language_segments_are_refused_before_lowering() {
        for payload in ["", "+eng", "eng+", "eng++fra", "&eng", "eng&", "eng&&fra"] {
            assert!(
                admit_word_language(WordLanguagePayload::Codes(payload)).is_err(),
                "{payload}"
            );
        }
        assert!(matches!(
            admit_word_language(WordLanguagePayload::Shortcut),
            Ok(talkbank_model::model::WordLanguageMarker::Shortcut)
        ));
    }
}
