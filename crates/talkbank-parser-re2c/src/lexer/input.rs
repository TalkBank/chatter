//! The DFA's lookahead distinguishes a source byte from the input boundary.

/// Immutable caller input, including any forbidden control characters.
/// No terminator is appended and no source byte defines its extent.
#[derive(Clone, Copy)]
pub(super) struct LexerInput<'a>(&'a str);

/// End of input is not a NUL byte. The generated EOF rule uses both the
/// encoded lookahead and this distinction before choosing an accepting edge.
#[derive(Clone, Copy)]
pub(super) enum InputSymbol {
    Byte(u8),
    End,
}

impl<'a> LexerInput<'a> {
    pub(super) fn new(source: &'a str) -> Self {
        Self(source)
    }

    pub(super) fn text(self) -> &'a str {
        self.0
    }

    pub(super) fn symbol(self, cursor: usize) -> InputSymbol {
        match self.0.as_bytes().get(cursor) {
            Some(&byte) => InputSymbol::Byte(byte),
            None => InputSymbol::End,
        }
    }
}

impl InputSymbol {
    /// Encoding required by re2c:eof = 0. This is only DFA lookahead, never
    /// a token payload or a completion decision; the EOF bound check follows.
    pub(super) fn lookahead(self) -> u8 {
        match self {
            Self::Byte(byte) => byte,
            Self::End => 0,
        }
    }

    pub(super) fn is_end(self) -> bool {
        matches!(self, Self::End)
    }
}
