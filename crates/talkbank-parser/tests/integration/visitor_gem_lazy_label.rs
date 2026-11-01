// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
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

/// Whether `h` is one of the 3 GEM header variants migrated in Task 2e, or a
/// gem header written with nothing after its colon.
fn is_gem(h: &Header) -> bool {
    use talkbank_model::model::{HeaderKind, PlainKind};
    matches!(h.kind(), HeaderKind::Plain(PlainKind::Gem(_)))
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
        .as_slice()
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
// `@G:\t` (colon form without label): empty content, not the bare marker
// ---------------------------------------------------------------------------

/// Bare `@G` is legal. The colon form with nothing after its separator is the
/// one empty-content state, refused by validation (E553) rather than parsed
/// as the bare marker nobody wrote; parsing reports nothing.
#[test]
fn g_separator_no_label_is_empty_content_not_the_bare_marker() {
    let input = wrap("@G:\t\n");
    let (headers, diags) = gem_headers_and_diags(&input);
    assert_eq!(
        headers,
        vec![r#"EmptyContent(Plain { kind: Gem(Lazy), separator: WrittenSeparator })"#.to_string()],
        "@G with separator but no label is empty content"
    );
    assert!(diags.is_empty(), "no parse diagnostic: {diags:?}");
}

// ---------------------------------------------------------------------------
// `@Bg:` (colon, no tab separator, no label): bug-fix regression guard
// ---------------------------------------------------------------------------

/// `@Bg:` (colon suffix, no `:\t` header_sep, no label text): the parser must
/// lower NO gem header, neither `LazyGem` nor the legal bare `BeginGem`.
///
/// ## History
///
/// The pre-fix `gem::bg()` had a recovery branch producing
/// `Header::LazyGem { label: None }` for this line, because tree-sitter parses
/// the bare `:` as an ERROR child (emitting E316) and the branch scanned the
/// raw node text for a colon. Removing that branch left the ERROR group
/// lowered as `BeginGem { label: None }`: the legal bare marker, which the
/// author did not write, fabricated from a recovery node. Now an ERROR in the
/// group position lowers no header at all, exactly as an ERROR where any
/// other header's content belongs; the E316 for the unparsable `:` stays.
#[test]
fn bg_colon_no_label_lowers_no_gem_header_and_reports_e316() {
    let input = wrap("@Bg:\n");
    let (headers, diags) = gem_headers_and_diags(&input);
    assert!(
        headers.is_empty(),
        "@Bg: must lower no gem header, not a fabricated bare marker: {headers:?}"
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
