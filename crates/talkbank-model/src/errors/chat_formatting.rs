//! CHAT text normalization for diagnostic rendering.
//!
//! This module converts raw CHAT lines into display-friendly text while keeping
//! offset mappings so diagnostics can still point to the right source region.
//!
//! Normalization rules:
//! - Tabs expand to spaces at 8-column tab stops.
//! - Media bullet delimiters (`\u{0015}`) render as `•`.
//! - Underline control-marker pairs are removed from plain output.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Special_Markers>
///
/// Stateful processor that yields display events from CHAT control text.
///
/// This iterator handles:
/// - Tab expansion to 8-column boundaries
/// - Bullet delimiter rendering (\u0015 -> '•')
/// - Underline marker tracking (\u0002\u0001 begin, \u0002\u0002 end)
///
/// Consumers can use this to build styled output (TUI) or plain text (miette).
pub struct ChatTextProcessor<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    char_pos: usize,     // Byte offset in original text
    display_pos: usize,  // Byte offset in display output
    is_underlined: bool, // Current underline state
}

/// A display event produced by [`ChatTextProcessor`] when processing CHAT text.
#[derive(Debug, Clone, PartialEq)]
pub enum DisplayEvent {
    /// Regular character to display
    Char(char),
    /// Spaces from tab expansion (count)
    TabSpaces(usize),
    /// Bullet character
    Bullet,
    /// Start underlined region
    UnderlineBegin,
    /// End underlined region
    UnderlineEnd,
}

impl<'a> ChatTextProcessor<'a> {
    /// Create a new processor for the given CHAT text.
    pub fn new(text: &'a str) -> Self {
        Self {
            chars: text.chars().peekable(),
            char_pos: 0,
            display_pos: 0,
            is_underlined: false,
        }
    }

    /// Current byte offset in original text.
    pub fn char_pos(&self) -> usize {
        self.char_pos
    }

    /// Current byte offset in display output.
    pub fn display_pos(&self) -> usize {
        self.display_pos
    }

    /// Whether we're currently in an underlined region
    pub fn is_underlined(&self) -> bool {
        self.is_underlined
    }

    /// Process the next CHAT character and return one normalized display event.
    ///
    /// Offsets exposed by [`Self::char_pos`] and [`Self::display_pos`] are
    /// advanced in UTF-8 bytes.
    pub fn next_event(&mut self) -> Option<DisplayEvent> {
        let ch = self.chars.next()?;
        let ch_len = ch.len_utf8();

        // Handle underline markers
        if ch == '\u{0002}'
            && let Some(&next_ch) = self.chars.peek()
        {
            if next_ch == '\u{0001}' {
                // UNDERLINE_BEGIN
                self.chars.next(); // consume \u{0001}
                self.char_pos += ch_len + next_ch.len_utf8();
                self.is_underlined = true;
                return Some(DisplayEvent::UnderlineBegin);
            } else if next_ch == '\u{0002}' {
                // UNDERLINE_END
                self.chars.next(); // consume second \u{0002}
                self.char_pos += ch_len + next_ch.len_utf8();
                self.is_underlined = false;
                return Some(DisplayEvent::UnderlineEnd);
            }
        }

        // Handle special characters
        let event = match ch {
            '\t' => {
                let spaces_to_add = 8 - (self.display_pos % 8);
                self.display_pos += spaces_to_add;
                DisplayEvent::TabSpaces(spaces_to_add)
            }
            '\u{0015}' => {
                self.display_pos += '•'.len_utf8();
                DisplayEvent::Bullet
            }
            _ => {
                self.display_pos += ch_len;
                DisplayEvent::Char(ch)
            }
        };

        self.char_pos += ch_len;
        Some(event)
    }
}

/// Result of processing CHAT text for plain display.
///
/// Contains normalized display text and a mapping from original UTF-8 byte
/// offsets to display UTF-8 byte offsets.
pub struct PlainDisplayResult {
    /// Formatted text with tabs expanded, bullets rendered, markers removed
    pub text: String,
    /// Sorted list of `(original_byte_offset, display_byte_offset)` breakpoints.
    /// Use [`Self::map_offset`] to look up a display position.
    offset_map: Vec<(usize, usize)>,
}

impl PlainDisplayResult {
    /// Map one original byte offset to the corresponding display byte offset.
    ///
    /// Uses binary search on the breakpoint table built during processing.
    pub fn map_offset(&self, original: usize) -> usize {
        match self
            .offset_map
            .binary_search_by_key(&original, |&(orig, _)| orig)
        {
            Ok(i) => self.offset_map[i].1,
            Err(0) => 0,
            Err(i) => {
                // Interpolate linearly between breakpoints.
                let (prev_orig, prev_disp) = self.offset_map[i - 1];
                prev_disp + (original - prev_orig)
            }
        }
    }

    /// Map an original (start, end) span to display coordinates, ensuring minimum span width of 1.
    pub fn map_span(&self, start: usize, end: usize) -> (usize, usize) {
        let ds = self.map_offset(start);
        let de = self.map_offset(end);
        (ds, de.max(ds + 1))
    }
}

/// Process CHAT text into normalized display text and a reusable byte-offset map.
///
/// Single pass: builds output text and records offset breakpoints.
pub fn process_for_plain_display_mapped(text: &str) -> PlainDisplayResult {
    let mut processor = ChatTextProcessor::new(text);
    let mut display = String::with_capacity(text.len() * 2);
    let mut offset_map: Vec<(usize, usize)> = Vec::new();

    // Record initial position
    offset_map.push((0, 0));

    while let Some(event) = processor.next_event() {
        let char_pos = processor.char_pos();
        let display_pos = processor.display_pos();

        match event {
            DisplayEvent::Char(ch) => display.push(ch),
            DisplayEvent::TabSpaces(n) => {
                for _ in 0..n {
                    display.push(' ');
                }
            }
            DisplayEvent::Bullet => display.push('•'),
            DisplayEvent::UnderlineBegin | DisplayEvent::UnderlineEnd => {
                // Don't add anything to plain text, but record the position
                // shift (original bytes consumed, display position unchanged)
            }
        }

        // Record breakpoint whenever char_pos and display_pos diverge from
        // a simple 1:1 mapping (tabs, markers, bullets change the ratio)
        offset_map.push((char_pos, display_pos));
    }

    // Deduplicate consecutive entries with same original offset (keep last)
    offset_map.dedup_by_key(|entry| entry.0);

    PlainDisplayResult {
        text: display,
        offset_map,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests processor tabs.
    #[test]
    fn test_processor_tabs() {
        let mut proc = ChatTextProcessor::new("a\tb");

        assert_eq!(proc.next_event(), Some(DisplayEvent::Char('a')));
        assert_eq!(proc.display_pos(), 1);

        assert_eq!(proc.next_event(), Some(DisplayEvent::TabSpaces(7))); // 8 - 1 = 7
        assert_eq!(proc.display_pos(), 8);

        assert_eq!(proc.next_event(), Some(DisplayEvent::Char('b')));
        assert_eq!(proc.display_pos(), 9);
    }

    /// Tests processor bullet.
    #[test]
    fn test_processor_bullet() {
        let mut proc = ChatTextProcessor::new("a\u{0015}b");

        assert_eq!(proc.next_event(), Some(DisplayEvent::Char('a')));
        assert_eq!(proc.next_event(), Some(DisplayEvent::Bullet));
        assert_eq!(proc.next_event(), Some(DisplayEvent::Char('b')));
    }

    /// Tests processor underline.
    #[test]
    fn test_processor_underline() {
        let mut proc = ChatTextProcessor::new("a\u{0002}\u{0001}b\u{0002}\u{0002}c");

        assert_eq!(proc.next_event(), Some(DisplayEvent::Char('a')));
        assert!(!proc.is_underlined());

        assert_eq!(proc.next_event(), Some(DisplayEvent::UnderlineBegin));
        assert!(proc.is_underlined());

        assert_eq!(proc.next_event(), Some(DisplayEvent::Char('b')));
        assert!(proc.is_underlined());

        assert_eq!(proc.next_event(), Some(DisplayEvent::UnderlineEnd));
        assert!(!proc.is_underlined());

        assert_eq!(proc.next_event(), Some(DisplayEvent::Char('c')));
        assert!(!proc.is_underlined());
    }
}
