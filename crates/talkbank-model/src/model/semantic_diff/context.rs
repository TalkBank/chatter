//! Traversal context for semantic-diff operations.
//!
//! The context tracks the best current source span while recursive diff logic
//! walks model nodes. This keeps emitted diffs location-aware without forcing
//! every call site to thread span plumbing manually.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use crate::Location;

/// Tracks the current source span during semantic-diff traversal.
///
/// The value behaves like a lightweight stack frame pointer for span metadata:
/// nested nodes may temporarily override it, then restore the previous value.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
#[derive(Debug, Clone)]
pub struct SemanticDiffContext {
    current: Location,
}

impl Default for SemanticDiffContext {
    /// Creates an empty context with no active span.
    ///
    /// This keeps default traversal behavior deterministic when no source spans
    /// have been pushed yet.
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticDiffContext {
    /// Creates a new [`SemanticDiffContext`] with no active span.
    ///
    /// Callers usually allocate one context per semantic diff traversal.
    pub fn new() -> Self {
        Self {
            current: Location::Unlocated,
        }
    }

    /// The location of the nearest enclosing located node, or unlocated.
    ///
    /// This is read by report helpers to attach location metadata to diffs.
    pub fn current_location(&self) -> Location {
        self.current
    }

    /// Enter a node: a located node becomes the current location, an unlocated
    /// one leaves the enclosing location in force. Returns the previous value
    /// for restoration.
    pub fn push_location(&mut self, location: Location) -> Location {
        let prev = self.current;
        if let Location::Located(_) = location {
            self.current = location;
        }
        prev
    }

    /// Restore a previous location returned by [`push_location`](Self::push_location).
    ///
    /// Callers should always restore in reverse traversal order to preserve
    /// context correctness.
    pub fn pop_location(&mut self, prev: Location) {
        self.current = prev;
    }
}
