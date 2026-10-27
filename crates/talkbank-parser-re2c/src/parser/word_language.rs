//! Admission of word-level language markers into the shared model.

/// Admit the lexer payload before it enters the native word carrier. An absent
/// payload denotes the lexer's explicit bare @s token, not missing model data.
pub(super) fn admit_word_language(
    codes: Option<&str>,
) -> Result<talkbank_model::model::WordLanguageMarker, talkbank_model::model::LanguageCodeError> {
    use talkbank_model::model::{LanguageCode, WordLanguageMarker};
    match codes {
        None => Ok(WordLanguageMarker::Shortcut),
        Some(codes) if codes.contains('+') => codes
            .split('+')
            .map(LanguageCode::new)
            .collect::<Result<Vec<_>, _>>()
            .map(WordLanguageMarker::Multiple),
        Some(codes) if codes.contains('&') => codes
            .split('&')
            .map(LanguageCode::new)
            .collect::<Result<Vec<_>, _>>()
            .map(WordLanguageMarker::Ambiguous),
        Some(code) => LanguageCode::new(code).map(WordLanguageMarker::Explicit),
    }
}

#[cfg(test)]
mod language_admission_tests {
    use super::admit_word_language;

    #[test]
    fn empty_language_segments_are_refused_before_lowering() {
        for payload in ["", "+eng", "eng+", "eng++fra", "&eng", "eng&", "eng&&fra"] {
            assert!(admit_word_language(Some(payload)).is_err(), "{payload}");
        }
        assert!(matches!(
            admit_word_language(None),
            Ok(talkbank_model::model::WordLanguageMarker::Shortcut)
        ));
    }
}
