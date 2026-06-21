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
//! Runtime-aware bootstrap and parser/model validation tools now live in the
//! sibling `spec/runtime-tools` crate so ordinary generation does not pull Rust
//! parser/model crates into the default spec workflow.
//!
//! # Running the generators
//!
//! Each generator is its own `[[bin]]` target; there is no aggregate Makefile
//! wrapper in this repo, run the binaries directly. Note that `spec/tools/` is a
//! **separate Cargo workspace**, so commands must specify `--manifest-path`
//! when run from the repo root, or you must `cd spec/tools` first:
//!
//! ```bash
//! # Tree-sitter corpus test generation (the most common one)
//! cargo run --manifest-path spec/tools/Cargo.toml \
//!     --bin gen_tree_sitter_tests -- \
//!     -o ../tree-sitter-talkbank/test/corpus \
//!     -t spec/tools/templates
//!
//! # Rust validation test generation
//! cargo run --manifest-path spec/tools/Cargo.toml \
//!     --bin gen_rust_tests -- \
//!     -o crates/talkbank-parser-tests/tests/generated
//!
//! # Validation fixture corpus + manifest (validation-layer errors only)
//! cargo run --manifest-path spec/tools/Cargo.toml \
//!     --bin gen_validation_corpus -- \
//!     --corpus-dir crates/talkbank-parser-tests/tests/error_corpus/validation_errors
//!
//! # Error documentation (Markdown)
//! cargo run --manifest-path spec/tools/Cargo.toml \
//!     --bin gen_error_docs -- \
//!     -o docs/errors
//!
//! # Coverage dashboard (how many constructs have specs)
//! cargo run --manifest-path spec/tools/Cargo.toml \
//!     --bin gen_coverage_dashboard
//! ```
//!
//! # Module map
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`spec`] | Loaders and types for construct/error spec Markdown files |
//! | [`output`] | Formatters that turn parsed specs into generated artifacts |
//! | [`templates`] | Tera template engine for wrapping CHAT fragments into complete files |
//! Runtime-aware tooling such as bootstrap, corpus mining, and live
//! parser/model validation now lives in `spec/runtime-tools`.
//!
//! ## Binary entry points
//!
//! Each generator is a separate `[[bin]]` target in `Cargo.toml`:
//!
//! | Binary | Purpose |
//! |--------|---------|
//! | `gen_tree_sitter_tests` | Generate `tree-sitter test` corpus files from specs |
//! | `gen_rust_tests` | Generate Rust `#[test]` files from construct + error specs |
//! | `gen_validation_corpus` | Generate the validation fixture corpus + manifest.json from validation-layer error specs |
//! | `gen_error_docs` | Generate Markdown error documentation pages |
//! | `validate_spec` | Validate individual spec file format integrity |
//! | `coverage` | Report spec coverage of grammar node types |
//! | `gen_coverage_dashboard` | Generate HTML/Markdown coverage dashboard |
//! | `corpus_to_specs` | Bulk-convert corpus examples to spec format |
//! | `fix_spec_layers` | Auto-fix layer classifications in error specs |
//! | `enhance_specs` | Add missing metadata to existing specs |
//! | `corpus_node_coverage` | Analyze CST node type coverage across the corpus |
//! | `perturb_corpus` | Generate perturbed corpus files for fuzz-like testing |
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
//!
//! let specs = ErrorSpec::load_all("../../spec/errors")
//!     .expect("failed to load error specs");
//!
//! for spec in &specs {
//!     for err in &spec.errors {
//!         println!(
//!             "{} ({}) -- {} [{}, {}]",
//!             err.code,
//!             err.name,
//!             err.severity,
//!             spec.metadata.error_type,
//!             spec.metadata.status,
//!         );
//!     }
//! }
//! ```

pub mod output;
pub mod spec;
pub mod templates;

// Re-exports
pub use spec::{
    construct::{ConstructExample, ConstructMetadata, ConstructSpec},
    error::{ErrorExample, ErrorMetadata, ErrorReference, ErrorSpec},
};
