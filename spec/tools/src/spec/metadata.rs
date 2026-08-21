//! The value types of the error-spec format, re-exported.
//!
//! The types themselves (the closed enums and the validated newtypes) live in
//! [`talkbank_spec_vocabulary`], so the other cargo workspace can share them
//! instead of keeping a second copy that drifted. They are re-exported here,
//! unchanged, so every call site in this crate keeps working and there is
//! still one name to import.
//!
//! # What Phase 1b deleted from this module
//!
//! `parse_spec_title`, and its six tests. It read a spec's H1 to recover the
//! code and the name, absorbing three separator dialects (`:`, `,` and ` - `)
//! because two parsers of these files had disagreed about which one was
//! meant: one took the first whitespace token and stripped a trailing colon,
//! the other split at the first `:` or `,`, and they gave different answers on
//! eleven specs.
//!
//! Every one of those tests was POLICY, pinning what a human meant by a
//! separator, which is exactly the kind of question no type can settle. The
//! frontmatter format does not ask it: `code` and `name` are separate declared
//! fields, so there is no heading to split and nothing to disagree about. That
//! is the shape this repository looks for, and the reason to prefer a type to
//! a test: not a better test, but a question that stops being askable.

pub use talkbank_spec_vocabulary::{
    SpecDescription, SpecErrorCode, SpecLevel, Status, spec_file_paths,
};
