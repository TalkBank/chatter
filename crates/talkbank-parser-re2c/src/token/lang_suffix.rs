//! The `@s` language suffix of a word, with its lexeme.

use serde::Serialize;

/// A word's `@s` suffix: bare (`@s`, the shortcut that resolves like the
/// utterance's declared language) or explicit (`@s:eng+zho`). The codes are
/// `None` for the bare form, which is its own resolution rule rather than a
/// missing value; the lexeme is the whole suffix as written, for placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LangSuffix<'a> {
    codes: Option<&'a str>,
    lexeme: &'a str,
}

impl<'a> LangSuffix<'a> {
    /// Called only at lexical admission.
    pub(crate) fn from_lexed(codes: Option<&'a str>, lexeme: &'a str) -> Self {
        Self { codes, lexeme }
    }

    /// The language code(s) after `@s:`, or `None` for the bare shortcut.
    pub fn codes(self) -> Option<&'a str> {
        self.codes
    }

    /// The suffix as written (`@s`, `@s:eng+zho`).
    pub fn lexeme(self) -> &'a str {
        self.lexeme
    }
}

/// Serialized as the codes alone, as the token was when it carried only them.
impl Serialize for LangSuffix<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.codes.serialize(serializer)
    }
}
