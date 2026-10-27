//! Sink-specific failure for CHAT rendering, without fallible `Display` callbacks.

use std::convert::Infallible;

/// Destination for CHAT text. Only the destination can introduce a write error.
///
/// Renderers write text and characters directly; arbitrary `Display` formatting
/// is deliberately absent because it can fail independently of the destination.
/// Existing `std::fmt::Write` destinations are supported by the blanket adapter.
///
/// A generic renderer cannot introduce an independently fallible formatter:
///
/// ```compile_fail
/// use talkbank_model::model::ChatWriter;
/// fn render<W: ChatWriter>(sink: &mut W, value: &impl std::fmt::Display)
///     -> Result<(), W::Error>
/// {
///     write!(sink, "{value}")
/// }
/// ```
pub trait ChatWriter {
    /// Failure reported by this destination.
    type Error;

    /// Append a UTF-8 text fragment, or return the destination's error.
    fn write_str(&mut self, text: &str) -> Result<(), Self::Error>;

    /// Append one Unicode scalar value.
    fn write_char(&mut self, value: char) -> Result<(), Self::Error> {
        self.write_str(value.encode_utf8(&mut [0; 4]))
    }

    /// Write an unsigned decimal integer without allocation or `Display`.
    fn write_decimal(&mut self, mut value: u64) -> Result<(), Self::Error> {
        let mut digits = [0_u8; 20];
        let mut start = digits.len();
        loop {
            start -= 1;
            digits[start] = (value % 10) as u8;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        for digit in &digits[start..] {
            self.write_char(char::from(b'0' + digit))?;
        }
        Ok(())
    }
}

impl<W: std::fmt::Write + ?Sized> ChatWriter for W {
    type Error = std::fmt::Error;

    fn write_str(&mut self, text: &str) -> Result<(), Self::Error> {
        std::fmt::Write::write_str(self, text)
    }

    fn write_char(&mut self, value: char) -> Result<(), Self::Error> {
        std::fmt::Write::write_char(self, value)
    }
}

/// An owned-string destination whose write error is uninhabited.
///
/// This deliberately does not implement `std::fmt::Write`: using that trait
/// would erase the evidence that these writes cannot return a formatting error.
pub struct ChatString(String);

impl ChatString {
    /// Render a complete owned string. There is no discarded error or partial
    /// success path: the closure's error type has no values.
    pub fn render(render: impl FnOnce(&mut Self) -> Result<(), Infallible>) -> String {
        let mut sink = Self(String::new());
        match render(&mut sink) {
            Ok(()) => sink.0,
            Err(impossible) => match impossible {},
        }
    }
}

impl ChatWriter for ChatString {
    type Error = Infallible;

    fn write_str(&mut self, text: &str) -> Result<(), Self::Error> {
        self.0.push_str(text);
        Ok(())
    }

    fn write_char(&mut self, value: char) -> Result<(), Self::Error> {
        self.0.push(value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ChatString, ChatWriter};
    use crate::model::{MorWord, PosCategory, WriteChat};

    #[test]
    fn owned_destination_preserves_unicode_and_decimal_extremes() {
        let values = [0, 9, 10, 99, 100, u32::MAX as u64, u64::MAX];
        for value in values {
            let rendered = ChatString::render(|sink| {
                sink.write_str("ɪ↔ɛ")?;
                sink.write_char('🦜')?;
                sink.write_decimal(value)
            });
            assert_eq!(rendered, format!("ɪ↔ɛ🦜{value}"));
        }
    }

    #[derive(Debug, PartialEq)]
    struct Refused;

    struct RefusingSink;

    impl ChatWriter for RefusingSink {
        type Error = Refused;

        fn write_str(&mut self, _: &str) -> Result<(), Self::Error> {
            Err(Refused)
        }
    }

    #[test]
    fn model_preserves_the_destinations_error_type() {
        let word = MorWord::new(PosCategory::new("n"), "dog");
        assert_eq!(word.write_chat(&mut RefusingSink), Err(Refused));
        assert_eq!(word.to_chat_string(), "n|dog");
    }
}
