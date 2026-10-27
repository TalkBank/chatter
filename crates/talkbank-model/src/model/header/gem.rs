//! Label-scoped gem transitions shared by validation and source navigation.

use std::collections::{HashMap, hash_map::Entry};

use super::{GemLabel, Header};
use crate::Location;

/// Why an end marker has no matching open begin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GemEndFailure {
    /// No label has an open scope.
    NoOpenScope,
    /// Other labels remain open, but this label does not.
    LabelMismatch,
}

/// A same-label begin/end pair, produced only by the gem transition owner.
/// Pairing does not certify that the surrounding header sequence is valid.
#[derive(Debug)]
pub struct GemPair<'a> {
    label: Option<&'a GemLabel>,
    begin: Location,
    end: Location,
}

impl<'a> GemPair<'a> {
    /// The authored label, or an unlabeled scope.
    pub fn label(&self) -> Option<&'a GemLabel> {
        self.label
    }
    /// Location of the matched begin, including explicit absence of geometry.
    pub fn begin(&self) -> Location {
        self.begin
    }
    /// Location of the matching end.
    pub fn end(&self) -> Location {
        self.end
    }
}

/// One observed gem transition. Nongem headers produce no event.
#[derive(Debug)]
pub enum GemEvent<'a> {
    /// A new label scope opened.
    Opened,
    /// Another begin opened while the same label was already active.
    NestedBegin {
        /// Label already active when this begin was observed.
        label: Option<&'a GemLabel>,
        /// Location of the repeated begin.
        location: Location,
    },
    /// The most recent begin for this label closed.
    Closed(GemPair<'a>),
    /// An end did not match any open begin of the same label.
    UnmatchedEnd {
        /// Label sought by the end marker.
        label: Option<&'a GemLabel>,
        /// Location of the unmatched end.
        location: Location,
        /// Whether another label remains active.
        reason: GemEndFailure,
    },
    /// A lazy marker occurred while a paired scope was open.
    LazyInsideScope {
        /// Authored label of the lazy marker.
        label: Option<&'a GemLabel>,
        /// Location of the lazy marker.
        location: Location,
    },
}

/// One or more outstanding begins. A final close consumes this state.
struct OpenGemScope {
    first: Location,
    repeated: Vec<Location>,
}

impl OpenGemScope {
    fn close(mut self) -> (Location, Option<Self>) {
        match self.repeated.pop() {
            Some(location) => (location, Some(self)),
            None => (self.first, None),
        }
    }
}

/// Outstanding begins after the consuming end-of-input transition.
pub struct UnclosedGem<'a> {
    label: Option<&'a str>,
    scope: OpenGemScope,
}

impl<'a> UnclosedGem<'a> {
    /// The authored label, or an unlabeled scope.
    pub fn label(&self) -> Option<&'a str> {
        self.label
    }
    /// The earliest still-open begin for this label.
    pub fn first(&self) -> Location {
        self.scope.first
    }
    /// Number of still-open begins; always at least one.
    pub fn count(&self) -> usize {
        std::iter::once(&self.scope.first)
            .chain(&self.scope.repeated)
            .count()
    }
}

/// Incremental label-scoped gem pairing. Different labels may overlap;
/// repeated begins for one label close in reverse opening order.
#[derive(Default)]
pub struct GemState<'a> {
    open: HashMap<Option<&'a str>, OpenGemScope>,
}

impl<'a> GemState<'a> {
    /// Start with no open scopes.
    pub fn new() -> Self {
        Self::default()
    }

    /// Observe an authored header. Unlocated markers still participate in
    /// pairing and diagnostic decisions; location only controls presentation.
    pub fn observe(&mut self, header: &'a Header, location: Location) -> Option<GemEvent<'a>> {
        match header {
            Header::BeginGem { label } => {
                let key = label.as_ref().map(|label| label.as_str());
                match self.open.entry(key) {
                    Entry::Vacant(entry) => {
                        entry.insert(OpenGemScope {
                            first: location,
                            repeated: Vec::new(),
                        });
                        Some(GemEvent::Opened)
                    }
                    Entry::Occupied(mut entry) => {
                        entry.get_mut().repeated.push(location);
                        Some(GemEvent::NestedBegin {
                            label: label.as_ref(),
                            location,
                        })
                    }
                }
            }
            Header::EndGem { label } => {
                let key = label.as_ref().map(|label| label.as_str());
                match self.open.remove(&key) {
                    Some(scope) => {
                        let (begin, remaining) = scope.close();
                        if let Some(scope) = remaining {
                            self.open.insert(key, scope);
                        }
                        Some(GemEvent::Closed(GemPair {
                            label: label.as_ref(),
                            begin,
                            end: location,
                        }))
                    }
                    None => Some(GemEvent::UnmatchedEnd {
                        label: label.as_ref(),
                        location,
                        reason: if self.open.is_empty() {
                            GemEndFailure::NoOpenScope
                        } else {
                            GemEndFailure::LabelMismatch
                        },
                    }),
                }
            }
            Header::LazyGem { label } if !self.open.is_empty() => Some(GemEvent::LazyInsideScope {
                label: label.as_ref(),
                location,
            }),
            _ => None,
        }
    }

    /// Finish observation, consuming the live state and exposing only nonempty
    /// outstanding scopes. No zero-count entry survives the final close.
    pub fn finish(self) -> impl Iterator<Item = UnclosedGem<'a>> {
        self.open
            .into_iter()
            .map(|(label, scope)| UnclosedGem { label, scope })
    }
}
