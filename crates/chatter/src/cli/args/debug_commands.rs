//! `chatter experimental transform` subcommands, internal debugging / audit /
//! sanitization tools.

use crate::cli::catalog::{Contract, Shape, command_tree};
use clap::ValueEnum;
use std::path::PathBuf;

/// Scope selector for `chatter experimental transform join-retrace --scope`.
///
/// Selects which dangling-retrace kinds are eligible for the join transform.
/// See [`talkbank_transform::join_retrace::RetraceJoinScope`] for the
/// full semantics of each variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum JoinRetraceScope {
    /// Join only `[/]` partial-repetition retraces where the successor's
    /// leading words repeat the retraced material. This is the conservative
    /// default (Wave 1, OBVIOUS-only repair).
    #[default]
    Repetition,
    /// Also join `[//]`/`[///]`/`[/-]` correction retraces. Corrections
    /// replace rather than repeat, so no prefix check is applied; same-speaker
    /// presence alone is the gate (Wave 3a).
    Corrections,
    /// Join ANY dangling retrace kind (including `[/]` Partial) with the
    /// immediately-following same-speaker utterance, with NO repeat-prefix
    /// match required. Use `the default preview` first to review proposed joins (Wave 3b).
    All,
}

command_tree! {
/// Specialist transforms under `chatter experimental transform`.
pub enum DebugCommands {
    /// Rewrite whole-utterance runs of `@s` into utterance precodes (`[- LANG]`).
    ///
    /// Walks `.cha` files under the given paths, detects the same E255
    /// whole-utterance language-switch pattern that `chatter validate` rejects,
    /// and previews qualifying utterances. `--apply` publishes them in place. Files that need no changes
    /// are left untouched.
    LanguageRuns {
        /// Path to CHAT file(s) or directory trees to rewrite in place.
        #[arg(required = true)]
        path: Vec<PathBuf>,
        /// Publish the proposed edits; without this flag only preview.
        #[command(flatten)]
        commit: super::Flag<crate::commands::debug::Commit>,
    } => Shape::Leaf(Contract::LanguageRuns),

    /// Preview retagging one language code as another; --apply publishes.
    ///
    /// A language code is named four ways, by `@Languages`, by a `[- code]`
    /// utterance scope, by a `word@s:code` marker and by a `<a b> [@s:code]`
    /// span, and a repair that moves some of them leaves a file contradicting
    /// itself. This moves the first three together and REFUSES a file using the
    /// fourth, rather than half-finishing.
    ///
    /// For a code the file already declares alongside the wrong one, the wrong
    /// entry is removed rather than duplicated.
    RetagLanguage {
        /// The code to replace, for example `sun`.
        #[arg(long)]
        from: String,

        /// The code to replace it with, for example `fin`.
        #[arg(long)]
        to: String,

        /// Path to CHAT file(s) or directory trees to rewrite in place.
        #[arg(required = true)]
        path: Vec<PathBuf>,
        /// Publish the proposed edits; without this flag only preview.
        #[command(flatten)]
        commit: super::Flag<crate::commands::debug::Commit>,
    } => Shape::Leaf(Contract::RetagLanguage),

    /// Join dangling-retrace utterances (E370) with their successor.
    ///
    /// Repairs the unambiguous subset of E370 ("dangling retrace"): an
    /// utterance whose last main-tier content is a retrace marker with nothing
    /// after it, followed by a same-speaker utterance. The `--scope` flag
    /// selects how broadly the join applies:
    ///
    /// - `repetition` (default, Wave 1): only `[/]` partial-repetition
    ///   retraces are joined, and only when the successor's leading words
    ///   repeat the retraced material.
    /// - `corrections` (Wave 3a): also joins `[//]`/`[///]`/`[/-]`
    ///   correction retraces using same-speaker presence alone as the gate.
    /// - `all` (Wave 3b): joins ANY dangling retrace kind, including `[/]`
    ///   where the successor does NOT repeat the retraced material. Covers
    ///   genuine child-language disfluencies (false starts, partial words,
    ///   fillers). Use `the default preview` first to review proposed joins.
    ///
    /// When either side carried dependent tiers (`%mor`/`%gra`/...), those
    /// tiers are dropped on the joined utterance and reported as needing
    /// re-morphotag.
    JoinRetrace {
        /// Path to CHAT file(s) or directory trees to repair in place.
        #[arg(required = true)]
        path: Vec<PathBuf>,

        /// Write the joins, or with `the default preview` only report them.
        #[command(flatten)]
        commit: super::Flag<crate::commands::debug::Commit>,

        /// Select which dangling-retrace kinds are eligible for joining.
        /// `repetition` (default): `[/]` with prefix-match gate. `corrections`:
        /// also `[//]`/`[///]`/`[/-]`. `all`: any kind, no prefix match.
        #[arg(long, value_enum, default_value_t = JoinRetraceScope::Repetition)]
        scope: JoinRetraceScope,
    } => Shape::Leaf(Contract::JoinRetrace),

    /// Sanitize a CHAT file for protected-corpus debugging, strip
    /// contributor lexical content while preserving structure.
    ///
    /// Replaces all word content, free-text dependent tiers, and
    /// free-text headers with placeholders / `[redacted]`, while
    /// preserving timing bullets, %wor offsets, speaker codes, POS
    /// tags, language markers, and structural counts. Intended for use
    /// before loading a protected `.cha` into LLM-assisted debugging
    /// tools. Redaction preserves identifiers and structure; it is not an anonymity guarantee.
    Redact {
        /// Path to a single .cha file to sanitize.
        input: PathBuf,

        /// Output path. If omitted, writes to stdout.
        #[arg(short, long, value_name = "PATH")]
        output: Option<PathBuf>,
    } => Shape::Leaf(Contract::Redact),
}
}
