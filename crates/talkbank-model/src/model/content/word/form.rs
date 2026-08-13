//! Word-form `@` suffix markers (`gumma@c`, `younz@d`, ...).
//!
//! [`FormType`] itself, its per-variant documentation, and the mapping in both
//! directions between a variant and its `@` code are GENERATED from
//! `spec/form_markers/form_marker_registry.json` into
//! [`crate::generated::form_markers`]. Change the registry, then run
//! `just form-markers-gen`; never edit the generated file.
//!
//! What lives here is the part that does not depend on which markers exist:
//! the type that names what a caller is holding ([`FormMarkerPayload`]), the
//! error for text that names no marker ([`UndeclaredFormMarker`]), and the
//! CHAT serialization impl.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Special_Form_Markers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>

use crate::model::WriteChat;

pub use crate::generated::form_markers::FormType;

/// The text of a form marker AFTER the `@`.
///
/// # Why this is a type and not a `&str`
///
/// The function this replaced took a `&str` and accepted BOTH `"b"` and
/// `"@b"`, so every arm of its match was written twice and each caller decided
/// for itself whether to strip the `@`. The two callers decided differently:
/// the tree-sitter parser passed `"@z:grm"` and tested for a `"@z:"` prefix,
/// the re2c parser passed `"z:grm"` and tested for `"z:"`. One fact, spelled
/// two ways, in two crates, with nothing relating them.
///
/// Naming the payload settles which of the two shapes a value is, and
/// [`FormType::from_payload`] splits the label itself, so no caller has to know
/// which markers take one.
///
/// The code and label are DERIVED on access rather than stored beside the
/// text. Storing all three meant an invariant (`text` is `code`, or `code` then
/// `:` then `label`) that only the constructor maintained, which is the shape
/// this whole registry exists to remove, in the type built to remove it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormMarkerPayload<'a> {
    /// The whole payload as written, without the `@`.
    text: &'a str,
}

impl<'a> FormMarkerPayload<'a> {
    /// Read the text that follows a word's `@`.
    ///
    /// The re2c lexer hands over exactly this; the tree-sitter parser strips
    /// the `@` from its token first. There is deliberately no constructor that
    /// accepts either shape: that leniency is what made the old `&str` seam
    /// ambiguous, and a constructor tolerating a stray `@` would additionally
    /// have had to claim, falsely, that a bare code cannot parse. It can:
    /// `after_at("b")` is `@b`.
    pub fn after_at(text: &'a str) -> Self {
        Self { text }
    }

    /// The part before any `:`.
    ///
    /// `split_once` rather than `split(':').next()`: a marker carries at most
    /// one colon and everything after the first one is the label, so `@z:a:b`
    /// has the label `a:b` rather than being silently truncated.
    pub fn code(&self) -> &'a str {
        self.text
            .split_once(':')
            .map_or(self.text, |(code, _)| code)
    }

    /// The part after the first `:`, absent when there is no colon. An empty
    /// label (`@z:`) is `Some("")`, which is distinct from `None` and is
    /// refused by [`FormType::from_payload`].
    pub fn label(&self) -> Option<&'a str> {
        self.text.split_once(':').map(|(_, label)| label)
    }

    /// The whole payload as written, without the `@`.
    pub fn text(&self) -> &'a str {
        self.text
    }
}

/// Text in a form-marker position that no registry row declares.
///
/// # Constructing one
///
/// The only constructor is `pub(crate)` and is called from exactly one place,
/// [`FormType::from_payload`], after the lookup has failed. There is no way to
/// assert this outcome while holding a payload that does parse, which is what
/// makes the error evidence rather than a label.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("undeclared form marker `@{payload}`")]
pub struct UndeclaredFormMarker {
    payload: String,
}

impl UndeclaredFormMarker {
    /// Record a payload that matched no declared marker.
    pub(crate) fn new(payload: FormMarkerPayload<'_>) -> Self {
        Self {
            payload: payload.text().to_owned(),
        }
    }

    /// The offending text, without the `@`.
    pub fn payload(&self) -> &str {
        &self.payload
    }

    /// Take the offending text, for a caller that has to keep it.
    ///
    /// Exists so the tree-sitter parser's E203 path does not copy the payload
    /// a second time to build its recovered word: the error already owns the
    /// only copy that needs to survive.
    pub fn into_payload(self) -> String {
        self.payload
    }

    /// What to tell the user instead. Generated from the registry, so a
    /// retired marker cannot stay advertised in a diagnostic; carried on the
    /// error so a reporting site cannot supply a different list of its own.
    pub fn suggestion(&self) -> &'static str {
        FormType::DECLARED_MARKERS_SUGGESTION
    }
}

impl WriteChat for FormType {
    /// Serializes marker payload without leading `@`.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.to_chat_marker().as_ref())
    }
}
