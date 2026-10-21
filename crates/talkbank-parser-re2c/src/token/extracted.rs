//! A tag-extracted payload that remembers the lexeme it was cut from.

use serde::Serialize;

/// A rich token's payload (what the lexer's tags extracted, trimmed) together
/// with the WHOLE lexeme the rule matched, so the item can be placed at itself
/// rather than at its payload. `[- eng]` has payload `eng` and lexeme
/// `[- eng]`; `&=laughs` has payload `laughs` and lexeme `&=laughs`.
///
/// Only the lexer constructs one (`emit_t1!`, `emit_t1t2!`), so both slices
/// are borrowed from the lexer input and `SourceText::span_of` can place the
/// lexeme. Consumers that want the item's content read `text()`; the
/// converter places the item with `lexeme()`. The AST serializes the payload
/// alone, as it did when the token was the payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extracted<'a> {
    payload: &'a str,
    lexeme: &'a str,
}

impl<'a> Extracted<'a> {
    /// Called only at lexical admission, with the trimmed tag capture and the
    /// rule's full match.
    pub(crate) fn from_lexed(payload: &'a str, lexeme: &'a str) -> Self {
        Self { payload, lexeme }
    }

    /// The tag-extracted content, as every consumer read the token before it
    /// carried its lexeme.
    pub fn text(self) -> &'a str {
        self.payload
    }

    /// The item as written, for placement.
    pub fn lexeme(self) -> &'a str {
        self.lexeme
    }
}

impl Serialize for Extracted<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.payload)
    }
}
