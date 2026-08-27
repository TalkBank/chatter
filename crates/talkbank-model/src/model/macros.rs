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
/// Separated from [`string_newtype!`](crate::string_newtype) so a newtype
/// WITH an invariant can share
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

/// The accessor set every `Vec`-backed collection newtype in the model owes a
/// consumer: `as_slice`, `as_mut_slice`, `into_vec`, `take`.
///
/// `is_empty`/`len`/`iter` are deliberately NOT here: `Deref<Target = Vec<T>>`
/// already supplies them, and emitting them would collide with the
/// hand-written ones on unrelated types sharing these files.
///
/// # Why this exists
///
/// Seventeen types wrap a `Vec` behind a private field, and before this macro
/// each hand-wrote its own subset. They had already drifted: `into_vec` was on
/// 6 of 17, `as_slice` on 11, `as_mut_slice` on 6, so what a consumer could do
/// depended on which type it happened to hold. v0.9.0 then closed the fields
/// without the consuming half, and a downstream pipeline could not move data
/// out of the ones that lacked `into_vec` at all.
///
/// One owner ends that: a type either invokes this and has the whole set, or
/// does not and has none of it. Per-type extras (`push`, `insert`, `remove`,
/// `append`, `pop`, `extend`) stay hand-written, because they genuinely differ.
///
/// # What closing the field does and does not buy, stated honestly
///
/// It prevents literal construction and destructuring. It does NOT currently
/// reserve room for a future invariant, because every one of these types also
/// has `impl From<Vec<T>>` and `impl Deref<Target = Vec<T>>`, so a caller can
/// still build any list at all in one call and read all of it. Any claim that
/// reconstruction "goes through `new`, where an invariant would be checked" is
/// false while that `From` exists. Deciding whether these types should carry
/// invariants (and therefore whether `From` should become `TryFrom`) is an open
/// design question. NOTE that this macro is not yet where that answer would be
/// applied: `Deref`, `From<Vec<T>>`, `new` and `is_empty` are still hand-written
/// on all seventeen types, so changing `From` today is a seventeen-site edit.
/// Moving those four into this macro is the prerequisite, and is worth doing
/// whichever way the invariant question goes.
#[macro_export]
macro_rules! collection_newtype_ops {
    ($name:ident, $item:ty) => {
        impl $name {
            /// Borrows the items.
            pub fn as_slice(&self) -> &[$item] {
                &self.0
            }

            /// Borrows the items for ELEMENT mutation.
            ///
            /// A slice, not a `&mut Vec`: elements may be rewritten in place
            /// but the collection cannot be resized through it. Resizing goes
            /// through [`take`](Self::take) and reconstruction, so the length
            /// only ever changes at a point the type can see.
            pub fn as_mut_slice(&mut self) -> &mut [$item] {
                &mut self.0
            }

            /// Consumes the wrapper and returns the owned items.
            ///
            /// A consuming accessor cannot violate an invariant, because the
            /// value it came from no longer exists.
            pub fn into_vec(self) -> Vec<$item> {
                self.0
            }

            /// Moves the items out, leaving this collection empty.
            ///
            /// The `&mut` counterpart of [`into_vec`](Self::into_vec), for the
            /// common "rebuild this list in place" shape. Without it every
            /// caller writes `std::mem::replace(x, T::new(Vec::new())).into_vec()`
            /// by hand; that incantation appeared sixteen times in one
            /// downstream migration before this existed.
            pub fn take(&mut self) -> Vec<$item> {
                std::mem::take(&mut self.0)
            }

            /// Keeps only the items matching `f`.
            ///
            /// A NAMED length-shrinking operation, and the reason it belongs
            /// here rather than downstream: without it every caller does
            /// `take()`, `retain` on the raw `Vec`, and rebuild, which hands a
            /// `&mut Vec<_>` to a closure that can do anything at all. That is
            /// `DerefMut` with ceremony, and `DerefMut` is what closing these
            /// fields removed. Three such helpers had grown in one downstream
            /// pipeline before this existed.
            ///
            /// It is also the operation an invariant would most need to
            /// re-check, and here it CAN: it owns `&mut self`.
            pub fn retain(&mut self, f: impl FnMut(&$item) -> bool) {
                self.0.retain(f);
            }
        }
    };
}
