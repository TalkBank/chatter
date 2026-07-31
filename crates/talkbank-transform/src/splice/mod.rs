//! Byte-exact application of typed, validator-computed replacements.
//!
//! The engine splices only the spans it is handed, so every other byte of a
//! transcript is preserved by construction rather than by care. This is what
//! lets a fixer repair one utterance in a file whose other regions did not
//! parse.
//!
//! # What "format-agnostic" does and does not mean here
//!
//! The engine has no CHAT-FORMAT knowledge: nothing here knows about tiers,
//! words, utterances, headers, or any other transcript structure. It moves
//! bytes at offsets its caller computed.
//!
//! It is NOT dependency-free of the CHAT crates, and deliberately so:
//! [`EditProvenance::Diagnostic`] carries a [`talkbank_model::ErrorCode`] so
//! that a rejected edit can name the rule that produced it. Flattening that to
//! an opaque string label would make the engine importable by a non-CHAT
//! caller, but at the cost of turning a checked enum into a stringly-typed
//! one, where a typo silently produces a provenance that matches nothing. The
//! typing is worth more than the theoretical decoupling, since every consumer
//! of this module already depends on `talkbank-model`.

/// Health-gated edit admission: the gate in front of the engine, which
/// admits an edit only when the utterance containing it parsed clean.
pub mod admit;

/// The fix catalog: the single, batch-safety-tiered answer to "what fix
/// does error code X get".
pub mod catalog;

/// The byte-splicing engine: typed edits over a source string, applied by
/// span rather than by search-and-replace.
pub mod engine;

/// The write gate: verifies a spliced result reproduces from its recorded
/// edits before it reaches disk. Necessary, not sufficient; see the module
/// docs for what it does not prove.
pub mod gate;

pub use admit::{Admission, SkipReason, Skipped, admit_edits};
pub use catalog::{BatchSafety, CatalogFix, FixKind, NamedAlternative, catalog_fix};
pub use engine::{
    EditProvenance, EditTarget, MappedEdit, Replacement, SpliceEdit, SpliceError, TransformName,
    apply_edits, mapped_edit_sites,
};
pub use gate::{GateError, apply_edits_verified, verify_splice};
