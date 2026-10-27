//! Core `ChatFile` type and line-container wrappers.
//!
//! CHAT reference anchors:
//! - [File headers](https://talkbank.org/0info/manuals/CHAT.html#File_Headers)
//! - [Participants header](https://talkbank.org/0info/manuals/CHAT.html#Participants_Header)
//! - [ID header](https://talkbank.org/0info/manuals/CHAT.html#ID_Header)

use crate::validation::{Validate, ValidationContext};
use crate::{ErrorSink, LineMap};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use talkbank_derive::{SemanticEq, SpanShift};

use crate::Line;

/// Top-level CHAT transcript model.
///
/// `ChatFile` preserves transcript line ordering and carries parser-derived
/// headers plus optional runtime indexing (`line_map`).
///
/// # Structure
///
/// A CHAT file is a sequence of lines where each line is either:
/// - **Header**: Metadata starting with `@` (e.g., `@Begin`, `@Comment`)
/// - **Utterance**: Transcribed speech with main tier and dependent tiers
///
/// Headers and utterances are **interleaved** to preserve file structure:
///
/// ```text
/// @UTF8
/// @Begin
/// *CHI: hello .
/// @Comment: Between utterances
/// *MOT: hi .
/// @End
/// ```
///
/// # CHAT Manual Reference
///
/// - [CHAT Format Overview](https://talkbank.org/0info/manuals/CHAT.html)
/// - [File Structure](https://talkbank.org/0info/manuals/CHAT.html#File_Headers)
///
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
pub struct ChatFile {
    /// Sequence of lines (headers + utterances) in file order.
    ///
    /// The vector preserves exact interleaving from source text.
    ///
    /// Headers and utterances can appear in any order, though typical files have:
    /// - File headers at the beginning (@UTF8, @Begin, @Participants, @ID)
    /// - Utterances in the middle
    /// - Comments and notes interleaved with utterances
    /// - @End header at the end
    pub lines: ChatFileLines,

    /// Byte-offset line index for O(log n) offset-to-line lookups.
    ///
    /// `Some(...)` for files parsed from source text; `None` for programmatically
    /// constructed files (synthesis pipelines, tests) where no source text exists.
    /// Rebuilt from source on re-parse, never serialized.
    #[serde(skip, default)]
    #[schemars(skip)]
    #[semantic_eq(skip)]
    #[span_shift(skip)]
    pub line_map: Option<LineMap>,
}

impl ChatFile {
    /// Build a mutable file whose metadata is derived from its current headers.
    pub fn new(lines: Vec<Line>) -> Self {
        Self {
            lines: lines.into(),
            line_map: None,
        }
    }

    /// Build a mutable file with its source byte-offset index.
    pub fn with_line_map(lines: Vec<Line>, line_map: LineMap) -> Self {
        Self {
            lines: lines.into(),
            line_map: Some(line_map),
        }
    }
}

/// Newtype wrapper around a list of lines in a CHAT file.
///
/// Preserves the interleaved ordering of headers and utterances.
///
/// # Reference
///
/// - [File headers](https://talkbank.org/0info/manuals/CHAT.html#File_Headers)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct ChatFileLines(Vec<Line>);

crate::collection_newtype_ops!(ChatFileLines, Line);

impl ChatFileLines {
    /// Inserts a line at `index`, shifting the rest right.
    ///
    /// A NAMED length-changing operation, so insertions happen at sites the
    /// type can see rather than through a raw `Vec`. A future invariant (a
    /// CHAT file's headers precede its utterances) COULD then be enforced
    /// here, but none is today, and this is not yet the only door:
    /// `From<Vec<Line>>` still builds one from an arbitrary list in a single
    /// call. See `collection_newtype_ops!` in `model/macros.rs`.
    ///
    /// # Panics
    ///
    /// If `index > len`, like `Vec::insert`.
    pub fn insert(&mut self, index: usize, line: Line) {
        self.0.insert(index, line);
    }

    /// Removes and returns the line at `index`, shifting the rest left.
    ///
    /// The other named length-changing operation; see [`Self::insert`].
    ///
    /// # Panics
    ///
    /// If `index >= len`, like `Vec::remove`.
    pub fn remove(&mut self, index: usize) -> Line {
        self.0.remove(index)
    }
}

impl ChatFileLines {
    /// Wrap line values while preserving file order.
    ///
    /// The wrapper exists to keep a typed boundary around the interleaved line
    /// stream while still exposing `Vec` ergonomics through `Deref`.
    pub fn new(lines: Vec<Line>) -> Self {
        Self(lines)
    }

    /// Return `true` when the file has no lines.
    ///
    /// This check is intentionally structural and does not inspect header state.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Deref for ChatFileLines {
    type Target = Vec<Line>;

    /// Borrows the underlying line vector.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Vec<Line>> for ChatFileLines {
    /// Wraps owned lines without copying.
    fn from(lines: Vec<Line>) -> Self {
        Self(lines)
    }
}

impl<'a> IntoIterator for &'a ChatFileLines {
    type Item = &'a Line;
    type IntoIter = std::slice::Iter<'a, Line>;

    /// Iterates immutably over file lines.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut ChatFileLines {
    type Item = &'a mut Line;
    type IntoIter = std::slice::IterMut<'a, Line>;

    /// Iterates mutably over file lines.
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl IntoIterator for ChatFileLines {
    type Item = Line;
    type IntoIter = std::vec::IntoIter<Line>;

    /// Consumes the wrapper and yields owned lines.
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Validate for ChatFileLines {
    /// Line-level validation is driven from `ChatFile` where header/order context exists.
    fn validate(&self, _context: &ValidationContext, _errors: &impl ErrorSink) {}
}
