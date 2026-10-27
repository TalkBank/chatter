//! Source-bound physical lines shared by diagnostic coordinate consumers.

/// A line start and its content, excluding its LF, CRLF, or CR terminator.
/// Only `PhysicalLines` constructs these values.
pub(super) struct PhysicalLine<'source> {
    start: usize,
    text: &'source str,
}

impl<'source> PhysicalLine<'source> {
    pub(super) fn start(&self) -> usize {
        self.start
    }

    pub(super) fn text(&self) -> &'source str {
        self.text
    }
}

/// Includes the empty terminal line after a terminator, and one line for empty
/// input. A CRLF pair is indivisible: both bytes belong to the preceding line.
pub(super) struct PhysicalLines<'source> {
    remaining: Option<&'source str>,
    offset: usize,
}

impl<'source> PhysicalLines<'source> {
    pub(super) fn new(source: &'source str) -> Self {
        Self {
            remaining: Some(source),
            offset: 0,
        }
    }
}

impl<'source> Iterator for PhysicalLines<'source> {
    type Item = PhysicalLine<'source>;

    fn next(&mut self) -> Option<Self::Item> {
        let remaining = self.remaining.take()?;
        let start = self.offset;
        let content_end = remaining
            .bytes()
            .position(|byte| matches!(byte, b'\r' | b'\n'));
        let text = match content_end {
            Some(end) => {
                let terminator_len = if remaining[end..].starts_with("\r\n") {
                    2
                } else {
                    1
                };
                let next = end + terminator_len;
                self.remaining = Some(&remaining[next..]);
                self.offset += next;
                &remaining[..end]
            }
            None => remaining,
        };
        Some(PhysicalLine { start, text })
    }
}
