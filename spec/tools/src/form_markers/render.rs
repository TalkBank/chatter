//! Rendering the form-marker registry into the four sites that used to spell
//! the inventory out by hand.
//!
//! Every renderer takes a [`FormMarkerRegistry`], which is only obtainable by
//! loading and checking the registry, so there is no way to render from
//! unchecked data.
//!
//! These functions return `String` rather than writing files, which is what
//! makes the drift gate honest: the test asserts that the committed file
//! equals what THIS code produces, calling the generator itself rather than a
//! second implementation of it. A gate that re-derived the expected output
//! independently would be the very shape this registry exists to remove.

use crate::form_markers::registry::FormMarkerRegistry;
use crate::form_markers::registry::LabelPolicy;
use crate::form_markers::registry::MarkerRow;
use std::fmt::Write;

/// Why a form-marker site could not be rendered.
///
/// Every failure here is a rustfmt failure, so this is the shared
/// [`RustfmtError`](crate::rust_source::RustfmtError) rather than a second
/// enum listing the same three cases.
pub type RenderError = crate::rust_source::RustfmtError;

/// The CHAT manual's base URL. Every marker links to its own anchor under it.
const MANUAL: &str = "https://talkbank.org/0info/manuals/CHAT.html";

/// One generated artifact: where it goes, what it is, and the function that
/// produces it.
///
/// The renderer lives ON the descriptor rather than beside it. The first
/// version of this module had the binary pair path with renderer by hand and
/// the drift gate pair them by hand a second time, so `write(RE2C_OUTPUT.path,
/// render_book(&registry))` type-checked and a fourth artifact added to one
/// list and not the other would have passed silently. That is the same
/// knowledge-with-no-owner shape this registry exists to delete, one level up.
pub struct GeneratedFile {
    /// The path to write, relative to the repository root.
    pub path: &'static str,
    /// What the file is, named in the generator's output and in the gate's
    /// failure message.
    pub what: &'static str,
    /// How to produce it.
    pub render: fn(&FormMarkerRegistry) -> Result<String, RenderError>,
}

/// Every artifact derived from the registry.
///
/// The binary writes this list and the drift gate checks this list, so the two
/// cannot describe different sets of files.
pub const OUTPUTS: &[GeneratedFile] = &[
    GeneratedFile {
        path: "crates/talkbank-model/src/generated/form_markers.rs",
        what: "the FormType enum and its marker mappings",
        render: render_rust,
    },
    GeneratedFile {
        path: "crates/talkbank-parser-re2c/src/generated_form_markers.re",
        what: "the re2c form-marker code set",
        render: render_re2c,
    },
    GeneratedFile {
        path: "book/src/chat-format/generated/form-markers.md",
        what: "the book's form-marker table",
        render: render_book,
    },
];

/// Everything said about a marker after its gloss: the example, the clause the
/// manual gives for it, and any deprecation.
///
/// One assembler for both the rustdoc line and the book's Notes cell. They
/// used to be built separately and had already diverged on punctuation
/// ("; deprecated" against ", deprecated"), which is two renderings of one
/// fact waiting to become two meanings.
fn notes(row: &MarkerRow) -> Vec<String> {
    let mut notes = vec![format!("`{}`", row.example())];
    if let Some(note) = &row.example_note {
        notes.push(note.to_string());
    }
    if let Some(deprecation) = &row.deprecated {
        notes.push(format!(
            "deprecated, use `{}` instead, because {}",
            deprecation.use_instead, deprecation.reason
        ));
    }
    notes
}

/// One line of prose describing a marker, shared by the type-level rustdoc and
/// the per-variant docs so that the two cannot describe the same marker
/// differently.
fn marker_summary(row: &MarkerRow) -> String {
    format!(
        "`{}` - {} ({})",
        row.marker_display(),
        row.gloss,
        notes(row).join(", ")
    )
}

/// A double-quoted literal. Marker codes are lowercase ASCII by construction
/// (`MarkerCode` refuses anything else), so this only ever has to quote, and
/// the result is valid in both Rust and re2c, which is why it is not named for
/// either.
fn quoted_ascii(value: &str) -> String {
    format!("{value:?}")
}

fn type_doc(registry: &FormMarkerRegistry) -> Vec<String> {
    let mut lines = vec![
        "Special-form suffix marker attached to a word token.".to_owned(),
        String::new(),
        "A word may carry one `@` marker naming what kind of form it is: a".to_owned(),
        "child-invented word, a letter, a sung passage. The set is closed, and".to_owned(),
        "declared once in `spec/form_markers/form_marker_registry.json`.".to_owned(),
        String::new(),
        "# Standard Markers".to_owned(),
        String::new(),
    ];
    for row in registry.markers() {
        lines.push(format!("- {}", marker_summary(row)));
    }
    lines.extend([
        String::new(),
        "# References".to_owned(),
        String::new(),
        format!("- [Special Form Markers]({MANUAL}#SpecialForm_Marker)"),
        String::new(),
        "Per-marker anchors are on each variant below.".to_owned(),
    ]);
    lines
}

fn render_variants(registry: &FormMarkerRegistry) -> String {
    registry
        .markers()
        .iter()
        .map(|row| {
            let doc = crate::rust_source::doc_comment(&[
                marker_summary(row),
                String::new(),
                format!("Reference: <{MANUAL}#{}>", row.manual_anchor),
            ]);
            // A label-taking marker carries its label; a label-free one cannot,
            // so `@x:foo` has nowhere to put the `foo` and is rejected rather
            // than silently dropped.
            let body = match row.label {
                LabelPolicy::Forbidden => format!("{},", row.variant),
                LabelPolicy::Required { .. } => format!("{}(String),", row.variant),
            };
            format!(
                "{doc}\n    #[serde(rename = {})]\n    {body}",
                quoted_ascii(row.marker.as_str())
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_from_payload(registry: &FormMarkerRegistry) -> String {
    let mut forbidden_arms = String::new();
    let mut required_arms = String::new();
    for row in registry.markers() {
        let code = quoted_ascii(row.marker.as_str());
        match row.label {
            LabelPolicy::Forbidden => {
                let _ = writeln!(
                    forbidden_arms,
                    "                {code} => Some(FormType::{}),",
                    row.variant
                );
            }
            LabelPolicy::Required { .. } => {
                let _ = writeln!(
                    required_arms,
                    "                {code} => Some(FormType::{}(label.to_owned())),",
                    row.variant
                );
            }
        }
    }

    format!(
        r#"    /// Read the text after a word's `@` into a declared marker.
    ///
    /// # Case handling, which is inherited and INCOHERENT
    ///
    /// Label-free codes are matched case-insensitively, so `word@B` parses as
    /// [`FormType::B`], while a label-taking code is matched exactly, so
    /// `word@Z:grm` is undeclared. Both behaviours are carried over verbatim
    /// from the three hand-written sites this replaces, where the first came
    /// from a `to_lowercase()` and the second from a literal `"z:"` prefix
    /// test in each caller. Neither is defensible and the corpus authority
    /// writes every marker lowercase, but changing what chatter accepts is not
    /// a de-duplication and needs a comparison over real corpora, so the incoherence
    /// is preserved here, in one place where it can be seen, instead of in
    /// three where it could not.
    ///
    /// The case fold is ASCII, not Unicode, and allocates only when the input
    /// actually carries an uppercase byte. Every marker code is ASCII by
    /// construction, so the Unicode fold could never match anything the ASCII
    /// one misses, and corpus markers are already lowercase in essentially
    /// every case: this runs about a million times per corpus pass, once per
    /// form-marked word, and previously allocated a `String` every time.
    pub fn from_payload(payload: FormMarkerPayload<'_>) -> Result<Self, UndeclaredFormMarker> {{
        let declared = match payload.label() {{
            None => {{
                let code = payload.code();
                let folded;
                let code = if code.bytes().any(|b| b.is_ascii_uppercase()) {{
                    folded = code.to_ascii_lowercase();
                    folded.as_str()
                }} else {{
                    code
                }};
                match code {{
{forbidden_arms}                    _ => None,
                }}
            }}
            // An empty label is unreachable through either parser (both require
            // at least one character after the colon) and is refused here so it
            // cannot be reached another way.
            Some(label) if !label.is_empty() => match payload.code() {{
{required_arms}                _ => None,
            }},
            Some(_) => None,
        }};
        declared.ok_or_else(|| UndeclaredFormMarker::new(payload))
    }}"#
    )
}

fn render_to_chat_marker(registry: &FormMarkerRegistry) -> String {
    let arms = registry
        .markers()
        .iter()
        .map(|row| match row.label {
            LabelPolicy::Forbidden => format!(
                "            FormType::{} => Cow::Borrowed({}),",
                row.variant,
                quoted_ascii(row.marker.as_str())
            ),
            LabelPolicy::Required { .. } => format!(
                "            FormType::{}(label) => Cow::Owned(format!(\"{}:{{label}}\")),",
                row.variant, row.marker
            ),
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"    /// The marker payload written after `@` in CHAT output.
    ///
    /// The exact inverse of [`FormType::from_payload`], because both are
    /// generated from the same registry rows: a marker cannot be readable and
    /// unwritable, or written under a code nothing parses.
    pub fn to_chat_marker(&self) -> Cow<'static, str> {{
        match self {{
{arms}
            // Verbatim, so a word carrying an undeclared marker serializes
            // back to exactly what was read.
            FormType::Undeclared(text) => Cow::Owned(text.clone()),
        }}
    }}"#
    )
}

/// The remedy shown with an E203 diagnostic.
///
/// Each marker is shown in the form a user must actually write it, so a
/// label-taking one appears as `@z:<label>` rather than bare `@z`. The
/// hand-written literal this replaced listed every marker bare, which was
/// worst precisely where it mattered: bare `@z` is itself undeclared, so the
/// old suggestion answered "what should I have written?" with an example of
/// the same mistake. Only the registry knows which markers take a label, which
/// is why the list could not be right until it had an owner.
fn render_suggestion(registry: &FormMarkerRegistry) -> String {
    let listed = registry
        .markers()
        .iter()
        .map(MarkerRow::marker_display)
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        r#"    /// The list of declared markers, as shown to a user who wrote one that
    /// is not declared. Generated, so retiring a marker cannot leave it
    /// advertised in a diagnostic.
    pub const DECLARED_MARKERS_SUGGESTION: &'static str =
        {};"#,
        quoted_ascii(&format!("Valid form markers: {listed}"))
    )
}

/// The Rust module: the `FormType` enum, its documentation, and both
/// directions of the marker mapping.
///
/// Formatted by `rustfmt` before it is returned, so a committed copy is
/// simultaneously up to date and formatted; see `rustfmt` below for why that has
/// to be one state rather than two.
pub fn render_rust(registry: &FormMarkerRegistry) -> Result<String, RenderError> {
    crate::rust_source::format_generated_rust(&format!(
        r#"// @generated by `gen_form_markers` (spec/tools) from
// spec/form_markers/form_marker_registry.json. DO NOT EDIT MANUALLY.
//
// Regenerate with `just form-markers-gen`.

//! The closed set of CHAT special-form markers, derived from the registry.
//!
//! Support types that do not depend on the registry (`FormMarkerPayload`,
//! `UndeclaredFormMarker`, the `WriteChat` impl) are hand-written and live in
//! `crate::model::content::word::form`.

use crate::model::content::word::form::FormMarkerPayload;
use crate::model::content::word::form::UndeclaredFormMarker;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use talkbank_derive::SemanticEq;
use talkbank_derive::SpanShift;

{type_doc}
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub enum FormType {{
{variants}

    /// Text that was written in a word's `@` slot but that no registry row
    /// declares (`word@zz`, `word@x:foo`).
    ///
    /// NOT a marker: it is a record of what the transcript actually said, kept
    /// so that recovery does not have to invent a marker that IS declared. The
    /// tree-sitter parser reports E203 and stores this; the payload is the
    /// text after the `@`, verbatim, so serializing the word reproduces the
    /// input exactly.
    ///
    /// Before this variant existed, that recovery path stored
    /// `UserDefined(payload)`, which claimed `word@zz` was the `@z`
    /// user-defined marker with label `zz`, and `to_chat_marker` rendered it
    /// back as `word@z:zz`. It was unobservable only because every command
    /// that serializes aborts on E203 first, which is a latent corruption
    /// rather than a safe one. The re2c parser stored nothing at all, so the
    /// two parsers disagreed on the same input.
    ///
    /// [`FormType::from_payload`] never returns this: it is reachable only
    /// from a parser that has already reported the marker undeclared.
    #[serde(rename = "undeclared")]
    Undeclared(String),
}}

impl FormType {{
{suggestion}

{from_payload}

{to_chat_marker}
}}
"#,
        type_doc = crate::rust_source::doc_comment(type_doc(registry)),
        variants = render_variants(registry),
        suggestion = render_suggestion(registry),
        from_payload = render_from_payload(registry),
        to_chat_marker = render_to_chat_marker(registry),
    ))
}

/// The re2c named definition for the closed set of marker codes.
///
/// Infallible, but returns `Result` so that it fits [`GeneratedFile::render`]
/// alongside [`render_rust`], which has to run rustfmt and genuinely can fail.
pub fn render_re2c(registry: &FormMarkerRegistry) -> Result<String, RenderError> {
    // Wrapped at a readable width rather than one code per line: this is read
    // beside the four rules that use it, and a 22-line definition would push
    // them off the screen.
    const WIDTH: usize = 76;
    const SEPARATOR: &str = " | ";
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for row in registry.markers() {
        let code = quoted_ascii(row.marker.as_str());
        if !current.is_empty() && current.len() + SEPARATOR.len() + code.len() > WIDTH {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push_str(SEPARATOR);
        }
        current.push_str(&code);
    }
    lines.push(current);

    let body = lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            if index == 0 {
                format!("w_form_code = {line}")
            } else {
                format!("             | {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!(
        r#"// @generated by `gen_form_markers` (spec/tools) from
// spec/form_markers/form_marker_registry.json. DO NOT EDIT MANUALLY.
//
// Regenerate with `just form-markers-gen`, then regenerate the vendored lexer
// (see build.rs for the exact re2rust invocation) in the same commit.
//
// The closed set of CHAT special-form marker codes, WITHOUT the leading `@`
// and WITHOUT any `:label`. Both of those are spelled out at each use site,
// because the four rules that lex a word tag the marker's boundaries
// differently and only the code itself is common to them.
//
// Note that this set does not decide which codes may take a `:label`: the
// lexer accepts `:label` after any of them and the typed model rejects the
// ones that must not have it (`@x:foo` is E203). Teaching the lexer that
// distinction would move a validity judgement into tokenization, which is the
// opposite of this codebase's parse-don't-validate rule.

{body};
"#
    ))
}

/// The book's marker table, and the notes on markers the CHAT manual gets
/// wrong.
///
/// Infallible; see [`render_re2c`] for why it returns `Result`.
pub fn render_book(registry: &FormMarkerRegistry) -> Result<String, RenderError> {
    /// A pipe in registry prose would otherwise end the table cell.
    fn cell(text: &str) -> String {
        text.replace('|', "\\|")
    }

    let rows = registry
        .markers()
        .iter()
        .map(|row| {
            format!(
                "| `{}` | [{}]({MANUAL}#{}) | {} |",
                row.marker_display(),
                cell(row.gloss.as_str()),
                row.manual_anchor,
                cell(&notes(row).join(", "))
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Rendered rather than restated by hand in the book. The `@x` disagreement
    // used to be written out in `symbols.md` AND in the registry README AND
    // recorded in the registry, where nothing read it: one fact in three
    // places, in the change whose whole thesis is that one fact belongs in one,
    // and the only machine-readable copy was the dead one.
    let disagreements: String = registry
        .markers()
        .iter()
        .filter_map(|row| {
            row.manual_disagreement
                .as_ref()
                .map(|note| format!("\n- `@{}`: {note}\n", row.marker))
        })
        .collect();
    let disagreements = if disagreements.is_empty() {
        String::new()
    } else {
        format!(
            "\nWhere the CHAT manual and chatter disagree, and why chatter is right:\n{disagreements}"
        )
    };

    Ok(format!(
        r#"<!-- @generated by `gen_form_markers` (spec/tools) from
     spec/form_markers/form_marker_registry.json. DO NOT EDIT MANUALLY.
     Regenerate with `just form-markers-gen`. -->

| Marker | Meaning | Notes |
|--------|---------|-------|
{rows}
{disagreements}"#
    ))
}
