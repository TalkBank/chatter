//! CHAT logical line iterator.
//!
//! In CHAT, a "logical line" starts with `@`, `*`, or `%` and includes
//! all subsequent continuation lines (physical lines starting with `\t`).
//!
//! ```text
//! @Comment:\tThis is a long comment    ← logical line start
//! \tthat continues here                ← continuation (part of same logical line)
//! \tand here                           ← continuation
//! *CHI:\thello .                       ← next logical line
//! ```
//!
//! This module provides [`ChatLines`], an iterator that yields complete
//! logical lines with their byte offsets. It handles `\n`, `\r\n`
//! and lone `\r` line endings. This helper belongs to fixture investigations;
//! the production parser uses its lexer-owned line regions.
//!
/// A single CHAT logical line with its position in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ChatLine<'a> {
    /// The full text of the logical line, including continuations
    /// and the trailing newline(s).
    text: &'a str,

    /// Byte offset of the start of this line in the source.
    offset: usize,
}

impl<'a> ChatLine<'a> {
    pub(super) fn text(&self) -> &'a str {
        self.text
    }

    pub(super) fn offset(&self) -> usize {
        self.offset
    }

    /// Presentation classification, derived from the retained original text.
    pub(super) fn kind(&self) -> ChatLineKind {
        match self.text.as_bytes().first() {
            Some(b'@') => ChatLineKind::Header,
            Some(b'*') => ChatLineKind::MainTier,
            Some(b'%') => ChatLineKind::DependentTier,
            Some(b'\r' | b'\n') => ChatLineKind::Blank,
            _ => ChatLineKind::Other,
        }
    }
}

/// What kind of CHAT line this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChatLineKind {
    /// Header line starting with `@`
    Header,
    /// Main tier line starting with `*`
    MainTier,
    /// Dependent tier line starting with `%`
    DependentTier,
    /// Line starting with a line ending (stray whitespace is Other)
    Blank,
    /// Unrecognized line, including a stray whitespace-prefixed continuation.
    Other,
}

/// Iterator over CHAT logical lines in a source string.
///
/// Each yielded [`ChatLine`] includes all continuation lines
/// (physical lines starting with `\t`) that follow the initial line.
pub(super) struct ChatLines<'a> {
    source: &'a str,
    pos: usize,
}

impl<'a> ChatLines<'a> {
    /// Create a new iterator over CHAT logical lines.
    pub fn new(source: &'a str) -> Self {
        Self { source, pos: 0 }
    }
}

impl<'a> Iterator for ChatLines<'a> {
    type Item = ChatLine<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.source.len() {
            return None;
        }

        let start = self.pos;
        let bytes = self.source.as_bytes();

        // Advance past the first physical line
        self.advance_past_newline();

        // Consume continuation lines (physical lines starting with \t)
        while self.pos < self.source.len() && bytes[self.pos] == b'\t' {
            self.advance_past_newline();
        }

        let text = &self.source[start..self.pos];
        Some(ChatLine {
            text,
            offset: start,
        })
    }
}

impl ChatLines<'_> {
    /// Advance past the current physical line (to after the newline).
    fn advance_past_newline(&mut self) {
        let bytes = self.source.as_bytes();
        while self.pos < bytes.len() {
            if bytes[self.pos] == b'\n' {
                self.pos += 1;
                return;
            }
            if bytes[self.pos] == b'\r' {
                self.pos += 1;
                if self.pos < bytes.len() && bytes[self.pos] == b'\n' {
                    self.pos += 1;
                }
                return;
            }
            self.pos += 1;
        }
        // Reached end of input without newline (last line without trailing newline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_lines_partition_original_source_in_each_transport() {
        let reference =
            include_str!("../../../../corpus/reference/core/multiline-continuation.cha");
        for ending in ["\n", "\r\n", "\r"] {
            let source = reference.replace('\n', ending);
            let lines: Vec<_> = ChatLines::new(&source).collect();
            assert!(
                lines
                    .iter()
                    .any(|line| line.text().contains(&format!("{ending}\t")))
            );
            let mut consumed = 0;
            for line in &lines {
                assert_eq!(line.offset(), consumed);
                assert!(!line.text().is_empty());
                let end = consumed + line.text().len();
                assert_eq!(&source[consumed..end], line.text());
                consumed = end;
            }
            assert_eq!(consumed, source.len());
            assert_eq!(lines.iter().map(ChatLine::text).collect::<String>(), source);
        }
    }

    #[test]
    fn simple_lines() {
        let input = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
        let lines: Vec<_> = ChatLines::new(input).collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0].kind(), ChatLineKind::Header);
        assert_eq!(lines[0].text(), "@UTF8\n");
        assert_eq!(lines[2].kind(), ChatLineKind::MainTier);
        assert_eq!(lines[2].text(), "*CHI:\thello .\n");
    }

    #[test]
    fn continuation_lines() {
        let input = "@Comment:\tlong\n\tcontinued\n\tmore\n*CHI:\thello .\n";
        let lines: Vec<_> = ChatLines::new(input).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text(), "@Comment:\tlong\n\tcontinued\n\tmore\n");
        assert_eq!(lines[0].kind(), ChatLineKind::Header);
        assert_eq!(lines[1].text(), "*CHI:\thello .\n");
    }

    #[test]
    fn dependent_tiers() {
        let input = "*CHI:\thello .\n%mor:\tpro|I .\n%gra:\t1|0|ROOT .\n";
        let lines: Vec<_> = ChatLines::new(input).collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1].kind(), ChatLineKind::DependentTier);
        assert_eq!(lines[2].kind(), ChatLineKind::DependentTier);
    }

    #[test]
    fn blank_lines() {
        let input = "@UTF8\n\n*CHI:\thello .\n";
        let lines: Vec<_> = ChatLines::new(input).collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1].kind(), ChatLineKind::Blank);
    }

    #[test]
    fn offsets_are_correct() {
        let input = "@UTF8\n*CHI:\thello .\n";
        let lines: Vec<_> = ChatLines::new(input).collect();
        assert_eq!(lines[0].offset(), 0);
        assert_eq!(lines[1].offset(), 6); // "@UTF8\n" is 6 bytes
    }

    #[test]
    fn crlf_handling() {
        let input = "@UTF8\r\n*CHI:\thello .\r\n";
        let lines: Vec<_> = ChatLines::new(input).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].kind(), ChatLineKind::Header);
    }

    #[test]
    fn no_trailing_newline() {
        let input = "@UTF8\n@End";
        let lines: Vec<_> = ChatLines::new(input).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].text(), "@End");
    }

    #[test]
    fn erroneous_line_absorbs_continuations() {
        let input = "GARBAGE\n\tcontinued garbage\n*CHI:\thello .\n";
        let lines: Vec<_> = ChatLines::new(input).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].kind(), ChatLineKind::Other);
        assert_eq!(lines[0].text(), "GARBAGE\n\tcontinued garbage\n");
        assert_eq!(lines[1].kind(), ChatLineKind::MainTier);
    }

    #[test]
    fn mor_with_continuation() {
        let input = "%mor:\tpro|I\n\tv|want .\n*MOT:\thi .\n";
        let lines: Vec<_> = ChatLines::new(input).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text(), "%mor:\tpro|I\n\tv|want .\n");
        assert_eq!(lines[0].kind(), ChatLineKind::DependentTier);
    }
}
