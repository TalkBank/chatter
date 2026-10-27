//! `WriteChat` trait for serializing data models to CHAT format
//!
//! This trait ensures all CHAT data model types can be serialized consistently,
//! similar to how serde's `Serialize` trait works for JSON/etc.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! # Design Principles
//!
//! 1. **Streaming**: Use `ChatWriter` with destination-specific errors
//! 2. **Self-contained**: Each type serializes itself completely (including prefixes)
//! 3. **Composable**: Types can call write_chat on their children
//! 4. **Error handling**: Propagate the destination error; owned strings use `Infallible`
//!
//! # Examples
//!
//! ```
//! use talkbank_model::model::{MorWord, PosCategory, WriteChat};
//!
//! let word = MorWord::new(PosCategory::new("pro"), "I");
//!
//! // Zero-allocation: write to any std::fmt::Write
//! let mut output = String::new();
//! word.write_chat(&mut output).expect("accepting destination");
//! assert_eq!(output, "pro|I");
//!
//! // Convenience: allocate a String
//! let output = word.to_chat_string();
//! assert_eq!(output, "pro|I");
//! ```

/// Trait for types that can be serialized to CHAT format
///
/// All CHAT data model types should implement this trait to ensure
/// they can be serialized consistently.
pub trait WriteChat {
    /// Write this object to CHAT format (zero-allocation)
    ///
    /// This method streams the CHAT representation directly to the writer,
    /// avoiding intermediate String allocations for better performance.
    ///
    /// # Arguments
    ///
    /// * `w` - A mutable reference to a `ChatWriter`; `std::fmt::Write` is adapted automatically
    ///
    /// # Returns
    ///
    /// * `Ok(())` if writing succeeded
    /// * `Err(W::Error)` if the destination refused a write
    ///
    /// # Examples
    ///
    /// ```
    /// use talkbank_model::model::{MorWord, PosCategory, WriteChat};
    ///
    /// let word = MorWord::new(PosCategory::new("n"), "dog");
    ///
    /// let mut output = String::new();
    /// word.write_chat(&mut output).expect("accepting destination");
    /// assert_eq!(output, "n|dog");
    /// ```
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error>;

    /// Convert to CHAT format string (convenience method that allocates)
    ///
    /// This is a convenience method that allocates a String. For better
    /// performance with large data structures, use `write_chat` directly.
    /// The owned destination has error type `Infallible`: this helper cannot
    /// discard a rendering error or return a successfully truncated prefix.
    ///
    /// # Examples
    ///
    /// ```
    /// use talkbank_model::model::{MorWord, PosCategory, WriteChat};
    ///
    /// let word = MorWord::new(PosCategory::new("v"), "run");
    /// assert_eq!(word.to_chat_string(), "v|run");
    /// ```
    fn to_chat_string(&self) -> String {
        crate::model::ChatString::render(|w| self.write_chat(w))
    }
}

impl<T: WriteChat + ?Sized> WriteChat for &T {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        T::write_chat(self, w)
    }
}

impl WriteChat for str {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        w.write_str(self)
    }
}

impl WriteChat for String {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        w.write_str(self)
    }
}

impl WriteChat for smol_str::SmolStr {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        w.write_str(self)
    }
}

impl WriteChat for crate::model::NonEmptyString {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        w.write_str(self.as_str())
    }
}

impl WriteChat for crate::model::RecoveryText {
    fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
        w.write_str(self.as_str())
    }
}

macro_rules! unsigned_chat {
    ($($ty:ty),+ $(,)?) => {$(
        impl WriteChat for $ty {
            fn write_chat<W: crate::model::ChatWriter>(&self, w: &mut W) -> Result<(), W::Error> {
                w.write_decimal(*self as u64)
            }
        }
    )+};
}

unsigned_chat!(u8, u16, u32, u64, usize);
