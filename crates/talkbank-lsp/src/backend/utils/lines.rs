//! Protocol lines retain their original byte origin and exclude their terminator.
//!
//! LSP recognizes LF, CRLF and CR. A trailing terminator starts a final empty
//! line; CRLF is one terminator. This is editor geometry, not CHAT parsing.

#[derive(Clone, Copy)]
pub(crate) struct ProtocolLine<'a> {
    text: &'a str,
    start: usize,
}

impl<'a> ProtocolLine<'a> {
    pub(crate) fn text(self) -> &'a str {
        self.text
    }

    pub(crate) fn start(self) -> usize {
        self.start
    }

    pub(super) fn end(self) -> usize {
        self.start + self.text.len()
    }
}

/// Only splitting the retained source can issue a line and its next cursor.
struct Remaining<'a> {
    text: &'a str,
    start: usize,
}

impl<'a> Remaining<'a> {
    fn split(self) -> (ProtocolLine<'a>, Option<Self>) {
        match self.text.find(['\r', '\n']) {
            Some(end) => {
                let after = end
                    + if self.text[end..].starts_with("\r\n") {
                        2
                    } else {
                        1
                    };
                (
                    ProtocolLine {
                        text: &self.text[..end],
                        start: self.start,
                    },
                    Some(Self {
                        text: &self.text[after..],
                        start: self.start + after,
                    }),
                )
            }
            None => (
                ProtocolLine {
                    text: self.text,
                    start: self.start,
                },
                None,
            ),
        }
    }
}

struct RemainingLines<'a>(Option<Remaining<'a>>);

impl<'a> Iterator for RemainingLines<'a> {
    type Item = ProtocolLine<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let (line, remaining) = self.0.take()?.split();
        self.0 = remaining;
        Some(line)
    }
}

/// Every document has a first line, including the empty document.
pub(super) fn split_lines(
    text: &str,
) -> (ProtocolLine<'_>, impl Iterator<Item = ProtocolLine<'_>>) {
    let (first, remaining) = Remaining { text, start: 0 }.split();
    (first, RemainingLines(remaining))
}

pub(crate) fn protocol_lines(text: &str) -> impl Iterator<Item = ProtocolLine<'_>> {
    let (first, remaining) = split_lines(text);
    std::iter::once(first).chain(remaining)
}
