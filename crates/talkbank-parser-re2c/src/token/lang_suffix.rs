//! The `@s` language suffix of a word, with its lexeme.

use serde::Serialize;

/// A present rich-word language suffix, before shared-model admission.
///
/// Absence belongs to the owning token's `Option`. Explicit code text is
/// checked during parser admission; an empty `Codes` payload is rejected,
/// never reinterpreted as a shortcut.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "codes", rename_all = "snake_case")]
pub enum WordLanguagePayload<'a> {
    /// The observed bare `@s` marker.
    Shortcut,
    /// Source text following `@s:`, including any language separators.
    Codes(&'a str),
}

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
