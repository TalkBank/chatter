//! Shared macros for model-layer newtypes and interned string wrappers.
//!
//! These macros generate reusable wrapper types used across header codes,
//! main-tier content tokens, and dependent-tier tokens.
//! Runtime and dependency paths are explicit; caller names cannot replace
//! generated API types, and display writes directly to its formatter.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

#[cfg(test)]
#[path = "macro_hygiene_tests.rs"]
mod tests;

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
            pub fn as_str(&self) -> &::core::primitive::str {
                &self.0
            }
        }

        impl $crate::model::WriteChat for $name {
            /// Writes the wrapped string content directly as CHAT text.
            fn write_chat<W: $crate::model::ChatWriter>(
                &self,
                w: &mut W,
            ) -> ::core::result::Result<(), W::Error> {
                w.write_str(&self.0)
            }
        }
        impl ::std::fmt::Display for $name {
            /// Displays the wrapped string without additional formatting.
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl ::std::ops::Deref for $name {
            type Target = ::core::primitive::str;

            /// Borrows the wrapped value as `str`.
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl ::core::convert::AsRef<::core::primitive::str> for $name {
            /// Returns a borrowed `str` view of the wrapped value.
            fn as_ref(&self) -> &::core::primitive::str {
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
        #[derive(::serde::Serialize, ::serde::Deserialize, ::schemars::JsonSchema, ::core::fmt::Debug, ::core::clone::Clone, ::core::cmp::PartialEq, ::core::cmp::Eq, ::core::hash::Hash, ::talkbank_derive::SemanticEq, ::talkbank_derive::SpanShift)]
        $(#[$meta])*
        $vis struct $name(::smol_str::SmolStr);

        $crate::string_newtype_read_impls!($name);

        impl $name {
            /// Constructs a new wrapper value from owned or borrowed text.
            ///
            /// This constructor performs no normalization so lexical content is
            /// preserved exactly as provided by callers.
            pub fn new(value: impl ::core::convert::Into<::smol_str::SmolStr>) -> Self {
                Self(value.into())
            }
        }

        impl ::core::convert::From<::std::string::String> for $name {
            /// Converts an owned string into the newtype.
            fn from(value: ::std::string::String) -> Self {
                Self(::smol_str::SmolStr::from(value))
            }
        }

        impl ::core::convert::From<&::core::primitive::str> for $name {
            /// Converts a borrowed string slice into the newtype.
            fn from(value: &::core::primitive::str) -> Self {
                Self(::smol_str::SmolStr::from(value))
            }
        }
    };
}

/// The admission surface every header-content value shares: one fallible
/// constructor (`try_new`), `TryFrom<String>` and `TryFrom<&str>` through it,
/// JSON deserialization through it, and serialization as its text.
///
/// The type supplies two things: `from_content(HeaderText) -> Self`, its
/// infallible constructor from content the one emptiness decision already
/// admitted, and `as_str(&self) -> &str`, its text as written. Every
/// fallible route then goes through
/// [`HeaderContent::admit`](crate::model::HeaderContent::admit), so blank text
/// (empty or only whitespace) is refused by every route the same way a
/// parser refuses it, by making the header `Header::EmptyContent` instead.
///
/// Used by [`non_empty_string_newtype!`] and by the parsed header values
/// (`ChatDate`, `TimeDurationValue`, `TimeStartValue`), which differ only in
/// what `from_content` does with the text. Pass `render` to also emit
/// `Display` and `WriteChat` from `as_str`, and `schema` to also emit a JSON
/// schema of a string with `minLength: 1` (a type deriving its schema, for
/// the doc comment, omits it).
#[macro_export]
macro_rules! header_content_admission {
    ($name:ident $(, $extra:ident)*) => {
        impl $name {
            /// Admits header content exactly as written, with no
            /// normalization.
            ///
            /// # Errors
            ///
            /// [`BlankContent`]($crate::model::BlankContent) when `value` is
            /// empty or only whitespace.
            pub fn try_new(
                value: impl ::core::convert::AsRef<::core::primitive::str>,
            ) -> ::core::result::Result<Self, $crate::model::BlankContent> {
                $crate::model::HeaderContent::admit(value.as_ref()).map(Self::from_content)
            }
        }

        impl ::core::convert::TryFrom<::std::string::String> for $name {
            type Error = $crate::model::BlankContent;

            /// Admits an owned string, refusing blank text.
            fn try_from(value: ::std::string::String) -> ::core::result::Result<Self, Self::Error> {
                Self::try_new(value)
            }
        }

        impl ::core::convert::TryFrom<&::core::primitive::str> for $name {
            type Error = $crate::model::BlankContent;

            /// Admits a borrowed string, refusing blank text.
            fn try_from(value: &::core::primitive::str) -> ::core::result::Result<Self, Self::Error> {
                Self::try_new(value)
            }
        }

        impl ::serde::Serialize for $name {
            /// Writes the text as written.
            fn serialize<S: ::serde::Serializer>(
                &self,
                serializer: S,
            ) -> ::core::result::Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            /// Reads the text, borrowed when the format allows, and admits it
            /// through `try_new`, so the wire cannot carry blank content past
            /// the type.
            fn deserialize<D: ::serde::Deserializer<'de>>(
                deserializer: D,
            ) -> ::core::result::Result<Self, D::Error> {
                let text = <::std::borrow::Cow<'de, ::core::primitive::str> as ::serde::Deserialize>::deserialize(deserializer)?;
                Self::try_new(&*text).map_err(::serde::de::Error::custom)
            }
        }

        $($crate::header_content_admission!(@$extra $name);)*
    };
    (@render $name:ident) => {
        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl $crate::model::WriteChat for $name {
            fn write_chat<W: $crate::model::ChatWriter>(
                &self,
                w: &mut W,
            ) -> ::core::result::Result<(), W::Error> {
                w.write_str(self.as_str())
            }
        }
    };
    (@schema $name:ident) => {
        impl ::schemars::JsonSchema for $name {
            fn schema_name() -> ::std::borrow::Cow<'static, ::core::primitive::str> {
                ::core::stringify!($name).into()
            }

            fn json_schema(_generator: &mut ::schemars::SchemaGenerator) -> ::schemars::Schema {
                ::schemars::json_schema!({ "type": "string", "minLength": 1 })
            }
        }
    };
}

/// Macro to generate string newtypes for header content: NEVER empty, and
/// never only whitespace.
///
/// For header payloads whose only meaning is their text (`@Situation`,
/// `@PID`, a gem label, ...). Blank content would say "this header has
/// content, and the content is nothing", which is the job of
/// `Header::EmptyContent`; so the payload type cannot hold it, and a parser
/// has to decide emptiness once, before it builds the payload.
///
/// Every route in is checked:
/// - `from_content` takes a [`HeaderText`](crate::model::HeaderText), the
///   proof only the one emptiness decision mints, so it is infallible
///   without being unchecked.
/// - `try_new`, `TryFrom<String>`, `TryFrom<&str>` and deserialization go
///   through that decision ([`header_content_admission!`]) and refuse blank
///   text with [`BlankContent`](crate::model::BlankContent); the JSON schema
///   says `minLength: 1`.
///
/// There is deliberately no infallible `new`, `From<String>` or `From<&str>`:
/// an infallible constructor beside an invariant makes it a suggestion.
/// Read and render surface: [`string_newtype_read_impls!`].
#[macro_export]
macro_rules! non_empty_string_newtype {
    ($(#[$meta:meta])* $vis:vis struct $name:ident;) => {
        // The schema is derived (so the doc comment stays the description)
        // with the length bound on the field.
        #[derive(::schemars::JsonSchema, ::core::fmt::Debug, ::core::clone::Clone, ::core::cmp::PartialEq, ::core::cmp::Eq, ::core::hash::Hash, ::talkbank_derive::SemanticEq, ::talkbank_derive::SpanShift)]
        $(#[$meta])*
        $vis struct $name(#[schemars(length(min = 1))] ::smol_str::SmolStr);

        $crate::string_newtype_read_impls!($name);
        $crate::header_content_admission!($name);

        impl $name {
            /// Builds the value from content the one emptiness decision
            /// admitted, exactly as written.
            pub fn from_content(value: $crate::model::HeaderText<'_>) -> Self {
                Self(::smol_str::SmolStr::from(value.as_str()))
            }
        }
    };
}

/// Macro to generate a header's closed vocabulary: the known values, and an
/// `Unsupported` value for any other nonblank text, preserved for the
/// validator to name.
///
/// Generates the enum, `from_text(HeaderText)`, `as_str`, serialization as
/// the text, deserialization that refuses blank text (the same refusal a
/// parser makes, by building `Header::EmptyContent` instead), and a string
/// schema with `minLength: 1`. `Unsupported` holds
/// [`UnsupportedSpelling<Self>`](crate::model::UnsupportedSpelling), which
/// only the generated `from_text` mints after its match refused every known
/// spelling: no route can make an unsupported value that is blank, or one
/// that is a known value in disguise. The constructor is crate-internal, so
/// the macro serves this crate's vocabularies only.
#[macro_export]
macro_rules! closed_vocabulary {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($(#[$vmeta:meta])* $variant:ident => $text:literal,)+
        }
    ) => {
        $(#[$meta])*
        #[derive(::core::fmt::Debug, ::core::clone::Clone, ::core::cmp::PartialEq, ::core::cmp::Eq, ::core::hash::Hash, ::talkbank_derive::SemanticEq, ::talkbank_derive::SpanShift, ::talkbank_derive::ValidationTagged)]
        $vis enum $name {
            $($(#[$vmeta])* $variant,)+
            /// Unrecognized nonblank value preserved for validation.
            Unsupported($crate::model::UnsupportedSpelling<$name>),
        }

        impl $name {
            /// Maps content to a known value, or keeps any other text as
            /// `Unsupported` so the validator can flag it.
            pub fn from_text(value: $crate::model::HeaderText<'_>) -> Self {
                match value.as_str() {
                    $($text => Self::$variant,)+
                    _ => Self::Unsupported(
                        $crate::model::UnsupportedSpelling::unmatched_by_vocabulary(value),
                    ),
                }
            }

            /// Returns the canonical CHAT token, or the unsupported text as
            /// written.
            pub fn as_str(&self) -> &::core::primitive::str {
                match self {
                    $(Self::$variant => $text,)+
                    Self::Unsupported(text) => text.as_str(),
                }
            }

            /// Admits text as a value, refusing blank text.
            ///
            /// # Errors
            ///
            /// [`BlankContent`]($crate::model::BlankContent) when `value` is
            /// empty or only whitespace.
            pub fn try_new(
                value: impl ::core::convert::AsRef<::core::primitive::str>,
            ) -> ::core::result::Result<Self, $crate::model::BlankContent> {
                $crate::model::HeaderContent::admit(value.as_ref()).map(Self::from_text)
            }
        }

        impl $crate::model::WriteChat for $name {
            /// Writes the CHAT token.
            fn write_chat<W: $crate::model::ChatWriter>(
                &self,
                w: &mut W,
            ) -> ::core::result::Result<(), W::Error> {
                w.write_str(self.as_str())
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(
                &self,
                serializer: S,
            ) -> ::core::result::Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            /// Reads the text and admits it through `try_new`, refusing blank
            /// text.
            fn deserialize<D: ::serde::Deserializer<'de>>(
                deserializer: D,
            ) -> ::core::result::Result<Self, D::Error> {
                let text = <::std::borrow::Cow<'de, ::core::primitive::str> as ::serde::Deserialize>::deserialize(deserializer)?;
                Self::try_new(&*text).map_err(::serde::de::Error::custom)
            }
        }

        impl ::schemars::JsonSchema for $name {
            fn schema_name() -> ::std::borrow::Cow<'static, ::core::primitive::str> {
                ::core::stringify!($name).into()
            }

            fn json_schema(_generator: &mut ::schemars::SchemaGenerator) -> ::schemars::Schema {
                ::schemars::json_schema!({ "type": "string", "minLength": 1 })
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
            ::serde::Serialize,
            ::serde::Deserialize,
            ::schemars::JsonSchema,
            ::core::fmt::Debug,
            ::core::clone::Clone,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
            ::core::hash::Hash,
            ::talkbank_derive::SemanticEq,
            ::talkbank_derive::SpanShift
        )]
        #[serde(transparent)]
        $(#[$meta])*
        $vis struct $name(pub ::std::sync::Arc<::core::primitive::str>);

        impl $name {
            /// Create a new interned value.
            ///
            /// Uses the selected global interner to reuse stored strings.
            /// Concurrent first misses may return equal strings in different
            /// allocations, so pointer identity is not an equality contract.
            pub fn new(value: impl ::core::convert::AsRef<::core::primitive::str>) -> Self {
                let s = value.as_ref();
                Self($interner_fn.intern(s))
            }

            /// Get the value as a string slice.
            ///
            /// Callers should prefer this over touching the inner `Arc<str>`
            /// directly so representation details stay encapsulated.
            pub fn as_str(&self) -> &::core::primitive::str {
                &self.0
            }
        }

        impl $crate::model::WriteChat for $name {
            /// Writes the interned string content directly as CHAT text.
            fn write_chat<W: $crate::model::ChatWriter>(&self, w: &mut W) -> ::core::result::Result<(), W::Error> {
                w.write_str(&self.0)
            }
        }

        impl ::std::fmt::Display for $name {
            /// Displays the interned string without additional formatting.
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl ::std::ops::Deref for $name {
            type Target = ::core::primitive::str;

            /// Borrows the interned value as `str`.
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl ::core::convert::AsRef<::core::primitive::str> for $name {
            /// Returns a borrowed `str` view of the interned value.
            fn as_ref(&self) -> &::core::primitive::str {
                &self.0
            }
        }

        impl ::std::borrow::Borrow<::core::primitive::str> for $name {
            /// Returns a borrowed `str` view for map/set lookup APIs.
            fn borrow(&self) -> &::core::primitive::str {
                &self.0
            }
        }

        impl ::core::convert::From<::std::string::String> for $name {
            /// Converts an owned string into the interned newtype.
            fn from(value: ::std::string::String) -> Self {
                Self::new(value)
            }
        }

        impl ::core::convert::From<&::core::primitive::str> for $name {
            /// Converts a borrowed string slice into the interned newtype.
            fn from(value: &::core::primitive::str) -> Self {
                Self::new(value)
            }
        }
    };
}

/// The accessor set every `Vec`-backed collection newtype in the model owes a
/// consumer: the non-shrinking half of [`collection_newtype_ops!`]
/// (`as_slice`, `as_mut_slice`, `into_vec`), and nothing that can change the
/// collection's length.
///
/// A collection with a nonempty invariant (`ChatOptionFlags`) invokes only
/// this half: every operation here keeps the length, or consumes the value,
/// so none of them can empty it. Every other collection invokes
/// [`collection_newtype_ops!`], which adds the shrinking half
/// ([`collection_newtype_shrink_ops!`]).
///
/// `is_empty`/`len`/`iter` are deliberately NOT here: `Deref<Target = Vec<T>>`
/// already supplies them, and emitting them would collide with the
/// hand-written ones on unrelated types sharing these files.
#[macro_export]
macro_rules! collection_newtype_read_ops {
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
            /// through a named operation or reconstruction, so the length
            /// only ever changes at a point the type can see.
            pub fn as_mut_slice(&mut self) -> &mut [$item] {
                &mut self.0
            }

            /// Consumes the wrapper and returns the owned items.
            ///
            /// A consuming accessor cannot violate an invariant, because the
            /// value it came from no longer exists; rebuilding rechecks it.
            pub fn into_vec(self) -> ::std::vec::Vec<$item> {
                self.0
            }
        }
    };
}

/// The length-shrinking half of [`collection_newtype_ops!`]: `take` and
/// `retain`. Never invoked by a collection with a nonempty invariant.
#[macro_export]
macro_rules! collection_newtype_shrink_ops {
    ($name:ident, $item:ty) => {
        impl $name {
            /// Moves the items out, leaving this collection empty.
            ///
            /// The `&mut` counterpart of [`into_vec`](Self::into_vec), for the
            /// common "rebuild this list in place" shape. Without it every
            /// caller writes `std::mem::replace(x, T::new(Vec::new())).into_vec()`
            /// by hand; that incantation appeared sixteen times in one
            /// downstream migration before this existed.
            pub fn take(&mut self) -> ::std::vec::Vec<$item> {
                ::std::mem::take(&mut self.0)
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
            pub fn retain(
                &mut self,
                f: impl ::core::ops::FnMut(&$item) -> ::core::primitive::bool,
            ) {
                self.0.retain(f);
            }
        }
    };
}

/// The full accessor set for a `Vec`-backed collection newtype with no
/// length invariant: [`collection_newtype_read_ops!`] and
/// [`collection_newtype_shrink_ops!`].
///
/// # Why this exists
///
/// The model's `Vec`-backed collection newtypes keep the `Vec` behind a
/// private field, and before these macros each hand-wrote its own subset. They had already drifted:
/// `into_vec` was on 6 of 17, `as_slice` on 11, `as_mut_slice` on 6, so what
/// a consumer could do depended on which type it happened to hold. v0.9.0
/// then closed the fields without the consuming half, and a downstream
/// pipeline could not move data out of the ones that lacked `into_vec` at
/// all.
///
/// One owner ends that: a type either invokes this and has the whole set,
/// invokes only the read half because it carries a nonempty invariant, or
/// does not and has none of it. Per-type extras (`push`, `insert`, `remove`,
/// `append`, `pop`, `extend`) stay hand-written, because they genuinely
/// differ.
///
/// # What closing the field does and does not buy, stated honestly
///
/// It prevents literal construction and destructuring. For the types that
/// invoke this full set it does NOT currently reserve room for a future
/// invariant, because each also has `impl From<Vec<T>>` and
/// `impl Deref<Target = Vec<T>>`, so a caller can still build any list at all
/// in one call and read all of it. Deciding whether these types should carry
/// invariants (and therefore whether `From` should become `TryFrom`) is an
/// open design question; `ChatOptionFlags` is the one that answered it, with
/// a fallible constructor and only the read half.
#[macro_export]
macro_rules! collection_newtype_ops {
    ($name:ident, $item:ty) => {
        $crate::collection_newtype_read_ops!($name, $item);
        $crate::collection_newtype_shrink_ops!($name, $item);
    };
}
