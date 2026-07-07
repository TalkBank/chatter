//! Generator for the mechanical conformance inventory
//! (`tests/generated_traversal_conformance/inventory.rs`).
//!
//! The inventory is derived, byte-for-byte, from two committed inputs:
//!
//! * `crates/talkbank-parser/src/generated_traversal.rs`, the generated
//!   typed CST traversal (itself produced by `tree-sitter-grammar-utils`), and
//! * `grammar/src/node-types.json`, tree-sitter's authoritative list of node
//!   kinds (the `"named": true` entries are the real, dispatchable kinds).
//!
//! Both the committed `inventory.rs` and the staleness guard
//! (`tests/conformance_inventory_current.rs`) call [`generate_inventory`], so a
//! future visitor regeneration that forgets to regenerate the inventory fails
//! the test suite instead of silently drifting. The runnable entry point is the
//! `gen_conformance_inventory` example.
//!
//! # What is derived
//!
//! Below a fixed prelude (module docs, `#![allow(...)]`, `use` lines, and the
//! three `impl_inspect_*!` `macro_rules!` definitions, kept verbatim in
//! [`PRELUDE`]) the generator emits four mechanical sections, parsed out of the
//! typed traversal with `syn`:
//!
//! 1. **Leaf list**: one `impl_inspect_leaf!(...)` listing every
//!    `pub struct XxxNode<'tree>(pub tree_sitter::Node<'tree>);` tuple wrapper.
//! 2. **Choice impls**: one `impl_inspect_choice!(XxxChoice { .. });` per
//!    `pub enum XxxChoice<'tree>`, variants in declaration order.
//! 3. **Struct impls**: one `impl_inspect_struct!(XxxChildren { .. });` per
//!    `pub struct XxxChildren<'tree>`, positional fields in declaration order
//!    (the `trailing_extras` and `unexpected` sink fields are not inspected and
//!    are excluded).
//! 4. **Dispatch fn**: one match arm per `extract_<snake>` free function whose
//!    `<snake>` is a `"named": true` node kind, keyed on the node kind. The arm
//!    form follows the function's first parameter type: a typed wrapper
//!    (`extract_x(XxxNode(node))`) or a bare `tree_sitter::Node`
//!    (`extract_x(node)`).
//!
//! The assembled source is then normalized through `rustfmt --edition 2024`
//! (see [`format_rust_source`]) so the committed file is rustfmt-clean and the
//! guard can compare byte-for-byte.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use syn::{Fields, FnArg, Item, Pat, Type};

/// Errors that can arise while generating the conformance inventory.
#[derive(Debug, thiserror::Error)]
pub enum InventoryGenError {
    /// The typed-traversal source did not parse as Rust.
    #[error("failed to parse generated_traversal.rs as Rust: {0}")]
    SynParse(#[from] syn::Error),

    /// `grammar/src/node-types.json` did not parse as the expected JSON shape.
    #[error("failed to parse node-types.json: {0}")]
    NodeTypesJson(#[from] serde_json::Error),

    /// An `extract_*` free function had no parameters (it must take the node).
    #[error("extract fn `extract_{0}` has no first parameter")]
    ExtractMissingParam(String),

    /// An `extract_*` free function's first parameter was neither a
    /// `tree_sitter::Node` nor a `XxxNode` typed wrapper.
    #[error("extract fn `extract_{name}` has an unrecognized first parameter type `{ty}`")]
    ExtractUnexpectedParam {
        /// The `extract_` suffix (the node kind in snake_case).
        name: String,
        /// The offending parameter type, rendered for the diagnostic.
        ty: String,
    },

    /// Invoking `rustfmt` failed at the I/O level.
    #[error("failed to run rustfmt: {0}")]
    Rustfmt(#[from] std::io::Error),

    /// `rustfmt` ran but exited non-zero (the assembled source did not parse).
    #[error("rustfmt exited with status {status}; stderr:\n{stderr}")]
    RustfmtFailed {
        /// The rustfmt process exit status, rendered.
        status: String,
        /// Captured rustfmt stderr, to point at the offending construct.
        stderr: String,
    },
}

/// The fixed head of `inventory.rs`: module docs, the `#![allow(...)]`, the
/// `use` lines, and the three `impl_inspect_*!` `macro_rules!` definitions. Kept
/// verbatim here (this is the "keep verbatim" region called out in the header);
/// everything after it is mechanically derived. rustfmt normalizes the exact
/// whitespace, so the meaningful content is the doc text and macro bodies.
const PRELUDE: &str = r#"//! MECHANICAL conformance inventory -- DO NOT HAND-EDIT.
//!
//! Generated from `crates/talkbank-parser/src/generated_traversal.rs`: a
//! no-op `Inspect` impl per leaf node wrapper (its own node kind is separately
//! visited and dispatched below), a variant-dispatching `Inspect` impl per
//! `*Choice` enum (recurse into whichever variant is actually present), a
//! field-recursing `Inspect` impl per `*Children` struct, and one `dispatch`
//! arm per `extract_*` free function (the arm key is the node kind == the rule
//! name).
//!
//! Regenerate with the committed generator after a grammar/visitor regen:
//! `cargo run -p talkbank-parser-tests --example gen_conformance_inventory`.
//! The staleness guard `conformance_inventory_is_current` re-derives this file
//! from the current typed traversal + node-types.json and fails if the
//! committed copy has drifted, so a forgotten regen breaks the suite instead of
//! silently losing coverage. The harness + allowlist live in the parent
//! `generated_traversal_conformance.rs`.

#![allow(clippy::too_many_lines)]

use talkbank_parser_tests::generated_traversal::*;

use super::{Inspect, InspectField, RawViolation};

/// Generate a no-op `Inspect` for a leaf node wrapper: its own node kind is
/// separately visited by `walk_all` and dispatched below, so there is nothing
/// further to recurse into from here.
macro_rules! impl_inspect_leaf {
    ($($name:ident),* $(,)?) => {
        $(
            impl<'tree> Inspect for $name<'tree> {
                fn inspect(&self, _rule: &'static str, _out: &mut Vec<RawViolation>) {}
            }
        )*
    };
}

/// Generate a variant-dispatching `Inspect` for a `*Choice` enum: recurse into
/// whichever variant is actually present (a leaf-wrapper payload's own
/// `Inspect` is a no-op; a synthetic Children-group payload recurses for real).
macro_rules! impl_inspect_choice {
    ($name:ident { $($variant:ident),* $(,)? }) => {
        impl<'tree> Inspect for $name<'tree> {
            fn inspect(&self, rule: &'static str, out: &mut Vec<RawViolation>) {
                match self {
                    $( Self::$variant(inner) => inner.inspect(rule, out), )*
                }
            }
        }
    };
}

/// Generate an `Inspect` impl that recurses into each named field. The
/// field-shape-specific logic (required / optional / repeat, and whether the
/// payload itself needs further recursion) lives once in the harness's
/// blanket `InspectField` impls, so every field is visited identically here
/// regardless of its declared shape.
macro_rules! impl_inspect_struct {
    ($name:ident { $($field:ident),* $(,)? }) => {
        impl<'tree> Inspect for $name<'tree> {
            fn inspect(&self, rule: &'static str, out: &mut Vec<RawViolation>) {
                $( self.$field.inspect_field(rule, stringify!($field), out); )*
            }
        }
    };
}
"#;

/// The doc comment stamped immediately before the generated `dispatch` fn.
const DISPATCH_DOC: &str = "/// Drive the generated `extract_*` for `node` if its kind has one, then\n/// inspect the returned children. One arm per `extract_*` free function.\n";

/// One `*Choice` enum: its type name and its variant identifiers, in
/// declaration order.
struct ChoiceImpl {
    name: String,
    variants: Vec<String>,
}

/// One `*Children` struct: its type name and its positional field identifiers,
/// in declaration order (the `trailing_extras` / `unexpected` sinks excluded).
struct StructImpl {
    name: String,
    fields: Vec<String>,
}

/// How an `extract_<snake>` free function takes its node argument, which fixes
/// the shape of the generated dispatch arm.
enum ExtractArg {
    /// First parameter is a bare `tree_sitter::Node`; the arm passes `node`.
    BareNode,
    /// First parameter is a typed wrapper `XxxNode`; the arm passes
    /// `XxxNode(node)`. Holds the wrapper type name.
    TypedWrapper(String),
}

/// One dispatchable `extract_<snake>` free function.
struct ExtractFn {
    /// The `extract_` suffix; equal to the node kind (the match-arm key).
    snake: String,
    /// The argument-passing shape.
    arg: ExtractArg,
}

/// One `grammar/src/node-types.json` entry (only the fields we need).
#[derive(serde::Deserialize)]
struct NodeTypeEntry {
    /// The node kind string.
    #[serde(rename = "type")]
    kind: String,
    /// Whether tree-sitter treats this as a real named node.
    named: bool,
}

/// Produce the fully formatted `inventory.rs` source from the two committed
/// inputs. This is the function both the `gen_conformance_inventory` example
/// and the `conformance_inventory_is_current` guard call, so their outputs are
/// identical by construction.
///
/// * `typed_src`: the text of `generated_traversal.rs`.
/// * `node_types_json`: the text of `grammar/src/node-types.json`.
pub fn generate_inventory(
    typed_src: &str,
    node_types_json: &str,
) -> Result<String, InventoryGenError> {
    format_rust_source(&generate_inventory_source(typed_src, node_types_json)?)
}

/// Assemble the (not-yet-formatted) `inventory.rs` source. Split out from
/// [`generate_inventory`] so the formatting step is testable in isolation and
/// so a caller could format with a different tool if ever needed.
pub fn generate_inventory_source(
    typed_src: &str,
    node_types_json: &str,
) -> Result<String, InventoryGenError> {
    let file = syn::parse_file(typed_src)?;

    let named_kinds = named_node_kinds(node_types_json)?;

    let mut leaves: Vec<String> = Vec::new();
    let mut choices: Vec<ChoiceImpl> = Vec::new();
    let mut structs: Vec<StructImpl> = Vec::new();
    let mut extracts: Vec<ExtractFn> = Vec::new();

    for item in &file.items {
        match item {
            Item::Struct(item_struct) => {
                let name = item_struct.ident.to_string();
                match &item_struct.fields {
                    // Leaf node wrapper: `pub struct XxxNode<'tree>(pub tree_sitter::Node<'tree>);`
                    Fields::Unnamed(unnamed)
                        if unnamed.unnamed.len() == 1
                            && is_tree_sitter_node(&unnamed.unnamed[0].ty) =>
                    {
                        leaves.push(name);
                    }
                    // Synthetic children carrier: `pub struct XxxChildren<'tree> { .. }`
                    Fields::Named(named) if name.ends_with("Children") => {
                        let fields = named
                            .named
                            .iter()
                            .filter_map(|field| field.ident.as_ref().map(ToString::to_string))
                            .filter(|f| f != "trailing_extras" && f != "unexpected")
                            .collect();
                        structs.push(StructImpl { name, fields });
                    }
                    _ => {}
                }
            }
            Item::Enum(item_enum) => {
                let name = item_enum.ident.to_string();
                if name.ends_with("Choice") {
                    let variants = item_enum
                        .variants
                        .iter()
                        .map(|variant| variant.ident.to_string())
                        .collect();
                    choices.push(ChoiceImpl { name, variants });
                }
            }
            Item::Fn(item_fn) => {
                let fn_name = item_fn.sig.ident.to_string();
                if let Some(snake) = fn_name.strip_prefix("extract_") {
                    let arg = classify_extract_arg(snake, item_fn)?;
                    extracts.push(ExtractFn {
                        snake: snake.to_owned(),
                        arg,
                    });
                }
            }
            _ => {}
        }
    }

    // Deterministic ordering, mirroring the section conventions of the file:
    //
    // * Leaf list: sorted by wrapper type name (byte-wise). The leaves are
    //   `syn`-collected in source declaration order (which for the tuple
    //   wrappers is literal-token order, e.g. `LParenNode` first), so an
    //   explicit sort is what gives the familiar alphabetical leaf list.
    // * Choice / struct impls: kept in source DECLARATION order (no sort). The
    //   `file.items` walk already preserves source order, and grouping related
    //   synthetic `*Choice` / `*Children` types the way the generator emitted
    //   them keeps the file diff-stable against a visitor regen.
    // * Dispatch arms: sorted by node kind (the match-arm key).
    //
    // Within a single item, order is always source declaration order (a
    // choice's variants, a struct's fields).
    leaves.sort();

    // A dispatch arm is emitted only for extract fns whose kind is a real named
    // node kind; internal sub-rule extracts (reached only by recursion, never by
    // `node.kind()`) get none.
    let mut dispatch: Vec<&ExtractFn> = extracts
        .iter()
        .filter(|extract| named_kinds.contains(&extract.snake))
        .collect();
    dispatch.sort_by(|a, b| a.snake.cmp(&b.snake));

    Ok(render_source(&leaves, &choices, &structs, &dispatch))
}

/// Parse `node-types.json` and collect the set of `"named": true` node kinds.
fn named_node_kinds(
    node_types_json: &str,
) -> Result<std::collections::BTreeSet<String>, InventoryGenError> {
    let entries: Vec<NodeTypeEntry> = serde_json::from_str(node_types_json)?;
    Ok(entries
        .into_iter()
        .filter(|entry| entry.named)
        .map(|entry| entry.kind)
        .collect())
}

/// True iff `ty` is the tuple-wrapper payload type `tree_sitter::Node<'tree>`.
fn is_tree_sitter_node(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    let segments = &type_path.path.segments;
    segments.len() == 2 && segments[0].ident == "tree_sitter" && segments[1].ident == "Node"
}

/// Classify the first parameter of an `extract_<snake>` free function into the
/// dispatch-arm shape it implies.
fn classify_extract_arg(
    snake: &str,
    item_fn: &syn::ItemFn,
) -> Result<ExtractArg, InventoryGenError> {
    let first = item_fn
        .sig
        .inputs
        .first()
        .ok_or_else(|| InventoryGenError::ExtractMissingParam(snake.to_owned()))?;

    let FnArg::Typed(pat_type) = first else {
        // A `self` receiver: these are free functions, so this never happens,
        // but treat it as unexpected rather than silently guessing.
        return Err(InventoryGenError::ExtractUnexpectedParam {
            name: snake.to_owned(),
            ty: "self".to_owned(),
        });
    };
    // The pattern must bind the node; we only need the type to classify.
    let _ = matches!(&*pat_type.pat, Pat::Ident(_));

    let Type::Path(type_path) = &*pat_type.ty else {
        return Err(InventoryGenError::ExtractUnexpectedParam {
            name: snake.to_owned(),
            ty: "non-path type".to_owned(),
        });
    };
    let segments = &type_path.path.segments;

    // Bare `tree_sitter::Node<'tree>`.
    if segments.len() == 2 && segments[0].ident == "tree_sitter" && segments[1].ident == "Node" {
        return Ok(ExtractArg::BareNode);
    }
    // Typed wrapper `XxxNode<'tree>`: a single path segment ending in `Node`.
    if let Some(last) = segments.last()
        && segments.len() == 1
        && last.ident.to_string().ends_with("Node")
    {
        return Ok(ExtractArg::TypedWrapper(last.ident.to_string()));
    }
    Err(InventoryGenError::ExtractUnexpectedParam {
        name: snake.to_owned(),
        ty: render_path_segments(segments),
    })
}

/// Render a path type's segment idents joined by `::`, for diagnostics only.
fn render_path_segments(
    segments: &syn::punctuated::Punctuated<syn::PathSegment, syn::token::PathSep>,
) -> String {
    segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// Emit the assembled, not-yet-rustfmt source: the verbatim prelude followed by
/// the four mechanical sections.
fn render_source(
    leaves: &[String],
    choices: &[ChoiceImpl],
    structs: &[StructImpl],
    dispatch: &[&ExtractFn],
) -> String {
    let mut out = String::with_capacity(64 * 1024);
    out.push_str(PRELUDE);

    // 1. Leaf list.
    out.push_str("\n// --- leaf node wrapper impls ---\nimpl_inspect_leaf!(\n");
    for leaf in leaves {
        out.push_str("    ");
        out.push_str(leaf);
        out.push_str(",\n");
    }
    out.push_str(");\n");

    // 2. Choice impls.
    out.push_str("\n// --- *Choice enum impls ---\n");
    for choice in choices {
        out.push_str("impl_inspect_choice!(");
        out.push_str(&choice.name);
        out.push_str(" { ");
        out.push_str(&choice.variants.join(", "));
        out.push_str(" });\n");
    }

    // 3. Struct impls.
    out.push_str("\n// --- *Children struct impls ---\n");
    for struct_impl in structs {
        out.push_str("impl_inspect_struct!(");
        out.push_str(&struct_impl.name);
        out.push_str(" { ");
        out.push_str(&struct_impl.fields.join(", "));
        out.push_str(" });\n");
    }

    // 4. Dispatch fn.
    out.push('\n');
    out.push_str(DISPATCH_DOC);
    out.push_str(
        "pub(super) fn dispatch(node: tree_sitter::Node, out: &mut Vec<RawViolation>) {\n    match node.kind() {\n",
    );
    for extract in dispatch {
        out.push_str("        \"");
        out.push_str(&extract.snake);
        out.push_str("\" => ");
        match &extract.arg {
            ExtractArg::BareNode => {
                out.push_str("extract_");
                out.push_str(&extract.snake);
                out.push_str("(node)");
            }
            ExtractArg::TypedWrapper(wrapper) => {
                out.push_str("extract_");
                out.push_str(&extract.snake);
                out.push('(');
                out.push_str(wrapper);
                out.push_str("(node))");
            }
        }
        out.push_str(".inspect(\"");
        out.push_str(&extract.snake);
        out.push_str("\", out),\n");
    }
    out.push_str("        _ => {}\n    }\n}\n");

    out
}

/// Run `rustfmt --edition 2024` over `src` (via stdin, so no filename banner is
/// emitted) and return the formatted text. This is the same normalization
/// `cargo fmt` applies, verified idempotent on the committed inventory.
pub fn format_rust_source(src: &str) -> Result<String, InventoryGenError> {
    let mut child = Command::new("rustfmt")
        .arg("--edition")
        .arg("2024")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Feed the source on stdin. Scope the handle so it is dropped (closing the
    // pipe) before we wait, avoiding a deadlock on large inputs.
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| std::io::Error::other("rustfmt stdin was not captured"))?;
        stdin.write_all(src.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(InventoryGenError::RustfmtFailed {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Path helpers, resolved from this crate's manifest directory
/// (`<repo>/crates/talkbank-parser-tests`). Shared by the example and the guard
/// so neither hard-codes the layout.
///
/// Absolute path of the generated typed traversal (the primary input).
#[must_use]
pub fn typed_traversal_path() -> PathBuf {
    crate_dir()
        .join("..")
        .join("talkbank-parser")
        .join("src")
        .join("generated_traversal.rs")
}

/// Absolute path of tree-sitter's `node-types.json` (the named-kind authority).
#[must_use]
pub fn node_types_json_path() -> PathBuf {
    repo_root()
        .join("grammar")
        .join("src")
        .join("node-types.json")
}

/// Absolute path of the committed conformance inventory.
#[must_use]
pub fn inventory_path() -> PathBuf {
    crate_dir()
        .join("tests")
        .join("generated_traversal_conformance")
        .join("inventory.rs")
}

/// This crate's manifest directory.
fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The repository root (two levels up from this crate).
fn repo_root() -> PathBuf {
    crate_dir().join("..").join("..")
}
