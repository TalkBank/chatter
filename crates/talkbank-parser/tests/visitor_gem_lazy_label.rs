// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
)]

//! Characterization tests for GEM-header edge cases: the `NodeSlot::Missing`
//! path in `gem::g()` (Task 2e) and the `@Bg:` empty-label bug fix.
//!
//! ## Domain context
//!
//! There are three gem-header variants (Task 2e migration):
//!
//! - `@G`  (`g_header`)  : `NodeSlot<FreeTextNode>` (grammar-required child).
//!   -> `Header::LazyGem { label }`.
//! - `@Bg` (`bg_header`) : `Option<FreeTextNode>` (grammar-optional child).
//!   -> `Header::BeginGem { label }` always, even when
//!   malformed (e.g. `@Bg:` empty-label bug fix).
//! - `@Eg` (`eg_header`) : `Option<FreeTextNode>` (grammar-optional child).
//!   -> `Header::EndGem { label }`.
//!
//! The CHAT grammar rule for `@G` is:
//!   `g_header: seq(g_prefix, header_sep, free_text, newline)`
//! Both `header_sep` (`:\t`) and `free_text` are grammar-required. A fully bare
//! `@G` (no colon, no text) is NOT recognized as a `g_header` by tree-sitter:
//! it becomes E316 (unparsable content). The `LazyGem { label: None }` path is
//! reached when tree-sitter sees the `header_sep` (`:\t`) but the `free_text`
//! child is absent, triggering recovery: the `NodeSlot<FreeTextNode>` slot
//! becomes `NodeSlot::Missing`, which `gem::g()` maps to `None`, producing
//! `Header::LazyGem { label: None }`. The missing-node backstop then emits E342.
//!
//! ## What these tests pin
//!
//! 1. `@G:\t` (separator present, no text) -> `LazyGem { label: None }` + E342.
//!    This is the `NodeSlot::Missing` branch in `gem::g()`, previously uncovered.
//!
//! 2. `@Bg:` (colon present, no tab separator, no label text) ->
//!    `BeginGem { label: None }` + E316. Bug fix: the parser previously produced
//!    `LazyGem { label: None }` for this input (wrong: `@Bg` is never a lazy gem).
//!
//! ## Fixture note
//!
//! A bare `@G` (no `:\t` separator) is invalid CHAT (grammar requires
//! `header_sep + free_text`) and cannot be added to the reference corpus without
//! making the roundtrip gate go red. The inline CHAT strings below are therefore
//! purposefully malformed for recovery-path coverage; they are NOT in the
//! reference corpus. All asserted values were captured by running the current
//! parser (not guessed).

use talkbank_model::ErrorCollector;
use talkbank_model::model::{Header, Line};
use talkbank_parser::TreeSitterParser;

/// Whether `h` is one of the 3 GEM header variants migrated in Task 2e.
fn is_gem(h: &Header) -> bool {
    matches!(
        h,
        Header::BeginGem { .. } | Header::EndGem { .. } | Header::LazyGem { .. }
    )
}

/// Parse `input` at the real streaming boundary and return the `Debug` string of
/// every GEM header (in document order) plus every collected diagnostic as
/// `(code, message)`.
fn gem_headers_and_diags(input: &str) -> (Vec<String>, Vec<(String, String)>) {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let chat = parser.parse_chat_file_streaming(input, &errors);
    let headers = chat
        .lines
        .0
        .iter()
        .filter_map(|l| match l {
            Line::Header { header, .. } if is_gem(header) => Some(format!("{header:?}")),
            _ => None,
        })
        .collect();
    let diags = errors
        .into_vec()
        .into_iter()
        .map(|d| (d.code.as_str().to_string(), d.message))
        .collect();
    (headers, diags)
}

/// Minimal valid CHAT wrapper.
fn wrap(body: &str) -> String {
    format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
         @ID:\teng|corpus|CHI|3;00.||||Child|||\n{body}*CHI:\thello .\n@End\n"
    )
}

// ---------------------------------------------------------------------------
// `@G:\t` (separator present, no label text): NodeSlot::Missing branch
// ---------------------------------------------------------------------------

/// `@G:\t` with nothing after the tab: tree-sitter produces a `g_header` node
/// but the required `free_text` child is `NodeSlot::Missing`. `gem::g()` maps
/// Missing to `None`, which yields `Header::LazyGem { label: None }`. The
/// missing-node backstop emits E342.
///
/// This is the `NodeSlot::Missing` path in `gem::g()` that was uncovered before
/// this test was added (no reference fixture exercised it, since bare `@G` without
/// a valid label is invalid CHAT).
#[test]
fn g_separator_no_label_decodes_to_lazy_gem_none_with_e342() {
    let input = wrap("@G:\t\n");
    let (headers, diags) = gem_headers_and_diags(&input);
    assert_eq!(
        headers,
        vec![r#"LazyGem { label: None }"#.to_string()],
        "@G with separator but no label must produce LazyGem{{label: None}}"
    );
    assert_eq!(
        diags.len(),
        1,
        "exactly one diagnostic expected (E342 for missing free_text): {diags:?}"
    );
    assert_eq!(
        diags[0].0, "E342",
        "missing required element must be E342: {diags:?}"
    );
}

// ---------------------------------------------------------------------------
// `@Bg:` (colon, no tab separator, no label): bug-fix regression guard
// ---------------------------------------------------------------------------

/// `@Bg:` (colon suffix, no `:\t` header_sep, no label text): the parser must
/// produce `Header::BeginGem { label: None }`, NOT `Header::LazyGem`.
///
/// ## Bug history
///
/// The pre-fix `gem::bg()` had a recovery branch:
/// ```text
/// if label.is_none() && header_contains_colon(header_actual, input) {
///     ParseOutcome::parsed(Header::LazyGem { label: None })
/// }
/// ```
/// Because tree-sitter parses the bare `:` as an ERROR child (emitting E316),
/// `header_contains_colon` returned `true` (it scanned the raw node text, which
/// covers the ERROR child range), triggering the buggy `LazyGem` branch.
///
/// The fix: remove the branch entirely. `@Bg` NEVER produces `LazyGem`; only
/// `@G` does. The E316 diagnostic for the unparsable `:` is orthogonal to the
/// model kind and stays.
///
/// RED with the old code (produced `LazyGem { label: None }`); GREEN after fix.
#[test]
fn bg_colon_no_label_decodes_to_begin_gem_none_with_e316() {
    let input = wrap("@Bg:\n");
    let (headers, diags) = gem_headers_and_diags(&input);
    assert_eq!(
        headers,
        vec![r#"BeginGem { label: None }"#.to_string()],
        "@Bg: must produce BeginGem{{label: None}}, not LazyGem"
    );
    assert_eq!(
        diags.len(),
        1,
        "exactly one diagnostic expected (E316 for the unparsable ':'): {diags:?}"
    );
    assert_eq!(
        diags[0].0, "E316",
        "the unparsable ':' must emit E316: {diags:?}"
    );
}
