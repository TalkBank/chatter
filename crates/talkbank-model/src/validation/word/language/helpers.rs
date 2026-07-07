//! Small helper routines shared by word-language validators.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Codes>

use crate::model::LanguageCode;
use crate::validation::context::language_allows_numbers;

/// Resolve the alternate language for `@s` shortcut handling.
///
/// This helper only supports primary/secondary toggling from `@Languages`; it
/// intentionally returns `None` for tertiary or undeclared contexts.
pub(super) fn get_other_language(
    current_lang: &LanguageCode,
    declared_languages: &[LanguageCode],
) -> Option<LanguageCode> {
    if declared_languages.is_empty() {
        return None;
    }

    let primary = &declared_languages[0];
    let secondary = declared_languages.get(1);

    if current_lang.as_str() == primary.as_str() {
        secondary.cloned()
    } else if let Some(secondary_lang) = secondary {
        if current_lang.as_str() == secondary_lang.as_str() {
            Some(primary.clone())
        } else {
            None
        }
    } else {
        None
    }
}

/// Return whether `lang` is tertiary (index >= 2) in declared languages.
///
/// Tertiary languages require explicit markers instead of `@s` shortcut usage.
pub(super) fn is_tertiary_language(
    lang: &LanguageCode,
    declared_languages: &[LanguageCode],
) -> bool {
    match declared_languages
        .iter()
        .position(|l| l.as_str() == lang.as_str())
    {
        Some(pos) => pos >= 2,
        None => false,
    }
}

/// Return whether a composite language code contains any digit-allowing member.
///
/// Composite codes are split on `+` and `&`, then checked member-by-member.
pub(super) fn mixed_language_allows_numbers(lang_code: &str) -> bool {
    lang_code
        .split(&['+', '&'][..])
        .any(language_allows_numbers)
}
