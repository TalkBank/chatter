//! # CHAT Specification Generators
//!
//! This crate is the spec-driven generation engine: it reads the authoritative
//! CHAT specification files in `spec/constructs/` and `spec/errors/`, parses
//! them into structured Rust types ([`spec`]), and generates downstream
//! artifacts through the [`output`] formatters:
//!
//! - **Tree-sitter corpus tests** -- `*.txt` files written to
//!   `grammar/test/corpus/`, consumed by `tree-sitter test`.
//! - **Rust parser tests** -- `#[test]` source files (constructs + parser-layer
//!   error examples) for the parser crates.
//! - **Validation fixture corpus** -- one `.cha` fixture per validation-layer
//!   example plus a `manifest.json`, consumed by the data-driven runner in
//!   `talkbank-parser-tests`.
//! - **Error documentation** -- Markdown pages cataloging all error codes with
//!   examples and fix suggestions.
//!
//! The [`templates`] module handles wrapping sub-document fragments (words,
//! tiers) into complete CHAT files so tree-sitter can parse them.
//!
//! Corpus candidate selection and live parser/model validation live in the
//! sibling `spec/runtime-tools` crate so ordinary generation does not pull Rust
//! parser/model crates into the default spec workflow.
//!
//! # Running the generators
//!
//! One command, from anywhere in the checkout:
//!
//! ```bash
//! just spec-gen      # regenerate every committed artifact derived from spec/
//! just spec-check    # report staleness, writing nothing
//! ```
//!
//! Both drive [`artifacts::ARTIFACTS`] (plus the half in `spec/runtime-tools`
//! that needs the live `ErrorCode` enum). Every destination is a constant in
//! that registry, so there is no `--output-dir` to get wrong, and the checker
//! compares without writing because each [`artifacts::Artifact`] RETURNS its
//! files rather than emitting them.
//!
//! # Module map
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`spec`] | Loaders and types for construct/error spec Markdown files |
//! | [`output`] | Formatters that turn parsed specs into generated artifacts |
//! | [`templates`] | Tera template engine for wrapping CHAT fragments into complete files |
//! Corpus candidate selection and live parser/model validation live in
//! `spec/runtime-tools`. The bootstrap machinery this line used to name was
//! removed 2026-03-22.
//!
//! ## Binary entry points
//!
//! `just --list` names them, each with the question it answers. This module doc
//! carried its own table until 2026-08-21; it was the sixth copy of that list in
//! the tree and it was already wrong, describing `coverage` as reporting grammar
//! node coverage (which is `corpus_node_coverage`'s job, two rows below it) and
//! omitting `gen_form_markers` while asserting that the generation binaries were
//! gone. `spec/docs/ERROR_SPEC_FORMAT.md` holds the taxonomy, which is the part
//! that is not derivable from `ls src/bin`.
//!
//! # Examples
//!
//! Load all construct specs and inspect their examples:
//!
//! ```no_run
//! use generators::ConstructSpec;
//!
//! let specs = ConstructSpec::load_all("../../spec/constructs")
//!     .expect("failed to load construct specs");
//!
//! for spec in &specs {
//!     println!(
//!         "Category: {} / {} ({} examples)",
//!         spec.metadata.level,
//!         spec.metadata.category,
//!         spec.examples.len(),
//!     );
//!     for ex in &spec.examples {
//!         println!("  - {}: {}", ex.name, ex.description);
//!     }
//! }
//! ```
//!
//! Load all error specs and list their codes:
//!
//! ```no_run
//! use generators::ErrorSpec;
//! use talkbank_spec_vocabulary::registry::CodeRegistry;
//!
//! // The registry first: a spec resolves the code it names, so there is no
//! // way to load one without it, and no consumer downstream holding an
//! // `Option<Status>`.
//! let registry = CodeRegistry::load("../..".as_ref())
//!     .expect("failed to load the code registry");
//! let specs = ErrorSpec::load_all("../../spec/errors", &registry)
//!     .expect("failed to load error specs");
//!
//! for spec in &specs {
//!     println!(
//!         "{} ({}) -- {:?} [{}]",
//!         spec.error.code,
//!         spec.error.name,
//!         spec.kind(),
//!         spec.status(),
//!     );
//! }
//! ```

// A dangling rustdoc link is a doc naming an API that does not exist, which is
// the cheapest possible form of the rot this crate exists to prevent. Phase 1b
// shipped two of them within one commit: a link to a module the same commit
// deleted, and one to a method that never existed. Denied rather than warned,
// so deleting a symbol breaks the build instead of rotting a reference.
#![deny(rustdoc::broken_intra_doc_links)]

pub mod artifacts;
pub mod form_markers;
pub mod node_coverage;
pub mod output;
pub mod owned_output;
pub mod repo_paths;
pub mod rust_source;
pub mod spec;
pub mod templates;

// Re-exports
pub use spec::{
    construct::{ConstructExample, ConstructMetadata, ConstructSpec},
    error::{ErrorExample, ErrorSpec},
};

/// Registry fixtures for this crate's tests.
///
/// Four modules were hand-writing near-identical registry TOML with
/// backslash-continuation escaping, three of them byte-identical apart from
/// `status`. `CodeRegistry::parse` is still the only route in, so this does
/// not weaken the type's proof; the doctrine's "every route in" rule
/// explicitly sanctions a `#[cfg(test)]` path.
#[cfg(test)]
pub(crate) mod test_registry {
    use talkbank_spec_vocabulary::Status;
    use talkbank_spec_vocabulary::registry::CodeRegistry;

    /// A registry declaring exactly the given codes.
    ///
    /// Each code doubles as its own variant name: `E999` is a legal
    /// `UpperCamelCase` ASCII identifier, so a fixture needs no second
    /// vocabulary of its own.
    pub(crate) fn declaring(codes: &[(&str, Status)]) -> CodeRegistry {
        let toml: String = codes
            .iter()
            .map(|(code, status)| {
                format!(
                    "[[code]]\ncode = '{code}'\nvariant = '{code}'\n\
                     summary = 'A test code.'\nkind = 'Invalidity'\n\
                     status = '{}'\n",
                    status.as_str()
                )
            })
            .collect();
        CodeRegistry::parse(&toml).expect("a well-formed fixture registry")
    }

    /// Write a fixture registry into a temporary checkout root, at the one
    /// path `CodeRegistry::load` reads.
    pub(crate) fn write_into(root: &std::path::Path, codes: &[(&str, Status)]) {
        let path = root.join(talkbank_spec_vocabulary::registry::REGISTRY_PATH);
        std::fs::create_dir_all(path.parent().expect("the registry has a parent"))
            .expect("create spec/codes");
        let toml: String = codes
            .iter()
            .map(|(code, status)| {
                format!(
                    "[[code]]\ncode = '{code}'\nvariant = '{code}'\n\
                     summary = 'A test code.'\nkind = 'Invalidity'\n\
                     status = '{}'\n",
                    status.as_str()
                )
            })
            .collect();
        std::fs::write(&path, toml).expect("write the fixture registry");
    }
}
