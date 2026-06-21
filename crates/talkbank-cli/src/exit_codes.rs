//! Conventional CLI exit codes shared across `chatter` subcommands.
//!
//! All chatter subcommands honor the same three-tier exit contract
//! plus the adjudication-specific exit 4. Batch / pipeline drivers
//! key off these constants when aggregating per-session outcomes;
//! producers (speaker-id, merge, pipeline) and consumers (batch)
//! must agree by *name*, not by literal, otherwise renumbering one
//! site silently fails the others.

/// Operation completed normally and the expected output was written.
pub const EXIT_SUCCESS: i32 = 0;

/// Invalid input: parse error, I/O failure, missing file. The
/// command can't begin its work because the inputs are unusable.
pub const EXIT_INPUT_ERROR: i32 = 1;

/// Precondition violation. Inputs parsed cleanly but the operation
/// itself can't proceed semantically, ambiguous speaker, language
/// mismatch, invalid mapping spec, etc.
pub const EXIT_PRECONDITION: i32 = 2;

/// Reference-mode speaker-id refused on low Jaccard margin. The
/// operator must adjudicate (typically via `chatter adjudicate`).
/// Batch drivers treat this as "needs adjudication" rather than a
/// hard error.
pub const EXIT_LOW_CONFIDENCE: i32 = 4;
