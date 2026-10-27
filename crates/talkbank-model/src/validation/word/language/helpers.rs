//! Small helper routines shared by word-language validators.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Switching>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Codes>

use crate::validation::context::{language_allows_numbers, language_allows_prefix_marker};

/// Return whether a composite language code contains any digit-allowing member.
///
/// Composite codes are split on `+` and `&`, then checked member-by-member.
pub(super) fn mixed_language_allows_numbers(lang_code: &str) -> bool {
    lang_code
        .split(&['+', '&'][..])
        .any(language_allows_numbers)
}

/// Return whether a composite language code contains any prefix-marker member.
///
/// Same permissive treatment of composite codes as
/// [`mixed_language_allows_numbers`]: a word marked `@s:heb+eng` is code-mixed
/// and either member may be the one the marker belongs to, so the word is
/// accepted rather than guessed at.
pub(crate) fn mixed_language_allows_prefix_marker(lang_code: &str) -> bool {
    lang_code
        .split(&['+', '&'][..])
        .any(language_allows_prefix_marker)
}
