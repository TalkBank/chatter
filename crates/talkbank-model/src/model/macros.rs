//! Shared macros for model-layer newtypes and interned string wrappers.
//!
//! These macros generate reusable wrapper types used across header codes,
//! main-tier content tokens, and dependent-tier tokens.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
/// The READ and RENDER surface shared by every `SmolStr`-backed string
/// newtype: `as_str`, `WriteChat`, `Display`, `Deref<Target = str>`, `AsRef<str>`.
///
/// Separated from [`string_newtype!`] so a newtype WITH an invariant can share
/// it. Such a type must not have the infallible `new` / `From<String>` /
/// `From<&str>` that `string_newtype!` also emits, since an invariant with an
/// infallible constructor beside it is a suggestion rather than an invariant;
/// but nothing about borrowing or rendering differs, and hand-copying these
/// five impls per checked type is how they drift. `MediaFilename` is the first
/// caller (see `header_strings.rs`).
#[macro_export]
macro_rules! string_newtype_read_impls {
    ($name:ident) => {
        impl $name {
            /// Borrows the wrapped value as `&str`.
            ///
            /// This is the preferred accessor for formatting and validation code.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl $crate::model::WriteChat for $name {
            /// Writes the wrapped string content directly as CHAT text.
            fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
                w.write_str(&self.0)
            }
        }
        impl std::fmt::Display for $name {
            /// Displays the wrapped string without additional formatting.
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;

            /// Borrows the wrapped value as `str`.
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            /// Returns a borrowed `str` view of the wrapped value.
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

/// Macro to generate simple string newtype wrappers with common trait implementations.
///
/// Uses `SmolStr` for inline storage of short strings (≤23 bytes) and O(1) clone.
///
/// For newtypes with NO invariant: every value of the inner type is a legal
/// value of the newtype, which is why the constructors here are infallible. A
/// type that can reject its input must not use this macro; it invokes
/// [`string_newtype_read_impls!`] for the shared surface and writes its own
/// checked constructor.
///
/// This macro generates:
/// - Basic newtype struct with Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq, Hash
/// - Everything in [`string_newtype_read_impls!`]: `as_str`, WriteChat, Display, Deref, `AsRef<str>`
/// - An infallible `new(impl Into<SmolStr>)`
/// - `From<String>` and `From<&str>` implementations
#[macro_export]
macro_rules! string_newtype {
    ($(#[$meta:meta])* $vis:vis struct $name:ident;) => {
        #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema, Debug, Clone, PartialEq, Eq, Hash, talkbank_derive::SemanticEq, talkbank_derive::SpanShift)]
        $(#[$meta])*
        $vis struct $name(smol_str::SmolStr);

        $crate::string_newtype_read_impls!($name);

        impl $name {
            /// Constructs a new wrapper value from owned or borrowed text.
            ///
            /// This constructor performs no normalization so lexical content is
            /// preserved exactly as provided by callers.
            pub fn new(value: impl Into<smol_str::SmolStr>) -> Self {
                Self(value.into())
            }
        }

        impl From<String> for $name {
            /// Converts an owned string into the newtype.
            fn from(value: String) -> Self {
                Self(smol_str::SmolStr::from(value))
            }
        }

        impl From<&str> for $name {
            /// Converts a borrowed string slice into the newtype.
            fn from(value: &str) -> Self {
                Self(smol_str::SmolStr::from(value))
            }
        }
    };
}

/// Macro to generate interned string newtype wrappers using global interners.
///
/// This macro generates:
/// - Newtype struct with `Arc<str>` for memory-efficient deduplication
/// - `new(impl AsRef<str>)` that goes through the provided interner
/// - `as_str()` method
/// - All standard trait implementations (WriteChat, Display, Deref, AsRef, From)
/// - Serialize, Deserialize for transparent JSON serialization
///
/// # Example
///
/// ```ignore
/// interned_newtype! {
///     /// Documentation for the type
///     pub struct MyCode,
///     interner: my_interner()
/// }
/// ```
#[macro_export]
macro_rules! interned_newtype {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident,
        interner: $interner_fn:expr
    ) => {
        #[derive(
            serde::Serialize,
            serde::Deserialize,
            schemars::JsonSchema,
            Debug,
            Clone,
            PartialEq,
            Eq,
            Hash,
            talkbank_derive::SemanticEq,
            talkbank_derive::SpanShift
        )]
        #[serde(transparent)]
        $(#[$meta])*
        $vis struct $name(pub std::sync::Arc<str>);

        impl $name {
            /// Create a new interned value.
            ///
            /// The value is interned using the global interner, meaning repeated
            /// calls with the same value will return Arc pointers to the same
            /// allocation. This provides both memory efficiency and O(1) cloning.
            pub fn new(value: impl AsRef<str>) -> Self {
                let s = value.as_ref();
                Self($interner_fn.intern(s))
            }

            /// Get the value as a string slice.
            ///
            /// Callers should prefer this over touching the inner `Arc<str>`
            /// directly so representation details stay encapsulated.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl $crate::model::WriteChat for $name {
            /// Writes the interned string content directly as CHAT text.
            fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
                w.write_str(&self.0)
            }
        }

        impl std::fmt::Display for $name {
            /// Displays the interned string without additional formatting.
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;

            /// Borrows the interned value as `str`.
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            /// Returns a borrowed `str` view of the interned value.
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl std::borrow::Borrow<str> for $name {
            /// Returns a borrowed `str` view for map/set lookup APIs.
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            /// Converts an owned string into the interned newtype.
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl From<&str> for $name {
            /// Converts a borrowed string slice into the interned newtype.
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }
    };
}
