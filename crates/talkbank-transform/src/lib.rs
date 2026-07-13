#![deny(missing_docs)]
// Test code is exempt from this crate's `deny`-level panic lints,
// see `docs/panic-audit/talkbank-transform.md`.
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]
//! Focused transform building blocks for CHAT file processing.
//!
//! This crate exposes many leaf modules, but the crate root keeps a smaller
//! convenience surface for the most common pipeline entry points. Specialized
//! behavior continues to live in its owning module namespace (`json`,
//! `corpus`, `validation_runner`, `xml`, and so on).
//!
//! # Start here
//!
//! - [`parse_and_validate`] is the one-shot entry point for parse + validation
//! - [`parse_and_validate_with_parser`] is the same pipeline when you already
//!   have a reusable [`talkbank_parser::TreeSitterParser`]
//! - [`normalize_chat`] is the common root helper for normalized CHAT output
//! - [`json`] and [`xml`] own the format-conversion APIs
//! - [`corpus`] and [`validation_runner`] own discovery, caching, and
//!   directory-scale validation
//!
//! If you need a specialized transform (redaction, transcript merge, speaker ID,
//! adjudication, and so on), go directly to that module rather than expecting
//! the crate root to re-export every leaf API.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! ## Top-level entry points
//!
//! - Root re-exports such as [`parse_and_validate`] and [`normalize_chat`] are
//!   the common one-shot pipeline helpers.
//! - [`json`] and [`xml`] own the format-conversion surfaces.
//! - [`corpus`] and [`validation_runner`] own discovery,
//!   caching, and directory-scale validation workflows.
//!
//! # Design Principles
//!
//! - Streaming entry points require `ErrorSink` for diagnostics
//! - Cache paths are shared across tools for consistency
//!
//! # Examples
//!
//! ```no_run
//! use talkbank_transform::{parse_and_validate, PipelineError};
//! use talkbank_model::ParseValidateOptions;
//!
//! let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
//!     @ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n";
//! let options = ParseValidateOptions::default().with_validation();
//! let chat_file = parse_and_validate(content, options).unwrap();
//! assert_eq!(chat_file.utterances().count(), 1);
//! ```

// CHAT-format core: parse, serialize, validate, convert/normalize,
// dependent-tier handling, field extraction, redaction.
pub mod build_chat;
pub mod dependent_tiers;
pub mod extract;
pub mod fix_s;
pub mod join_retrace;
pub mod num_words;
pub mod parse;
pub mod redact;
pub mod rediarize;
pub mod serialize;
pub mod validate;

// Transcript merge / adjudication surface (corpus-agnostic workflow).
pub mod adjudication;
pub mod sanity_scan;
pub mod speaker_id;
pub mod transcript_merge;

// Format bridges and serialization boundaries.
pub mod json;
// CHAT → TalkBank XML emission (Rust emitter for the TalkBank XML format).
pub mod xml;

// Corpus-scale orchestration namespaces.
pub mod corpus;
pub mod validation_runner;

// Internal crate-root wiring for the convenience APIs below.
mod pipeline;
mod rendering;

// Common convenience re-exports. Detailed APIs continue to live in their
// owning modules above.
pub use self::corpus::{
    CorpusEntry, CorpusManifest, FailureReason, FileEntry, FileStatus as CorpusFileStatus,
    ManifestError, build_manifest, corpus_summary, discover_corpora, format_manifest,
};
pub use self::json::{
    JsonError, JsonResult, SCHEMA_JSON, is_schema_validation_available, schema_load_error,
    to_json_pretty_unvalidated, to_json_pretty_validated, to_json_unvalidated, to_json_validated,
    validate_json_string,
};
pub use self::pipeline::{
    PipelineError, chat_to_json, chat_to_json_unvalidated, normalize_chat, parse_and_validate,
    parse_and_validate_streaming, parse_and_validate_streaming_with_parser,
    parse_and_validate_with_parser, parse_file_and_validate,
};
pub use self::rendering::{
    RenderMode, RenderedDiagnostic, render_diagnostics, render_error_with_miette,
    render_error_with_miette_with_named_source, render_error_with_miette_with_source,
    render_error_with_miette_with_source_colored,
};
pub use self::validation_runner::{
    CacheMode, CacheOutcome, DirectoryMode, ErrorEvent, FileCompleteEvent, FileStatus, ParserKind,
    RoundtripEvent, ValidationCache, ValidationConfig, ValidationEvent, ValidationStats,
    ValidationStatsSnapshot, validate_directory_streaming,
};
pub use talkbank_cache::{CacheError, CachePool, CacheStats, UnifiedCache};
