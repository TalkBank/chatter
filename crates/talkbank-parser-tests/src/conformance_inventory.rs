//! Generator for the mechanical conformance inventory
//! (`src/conformance/inventory.rs`).
//!
//! The inventory is derived, byte-for-byte, from two committed inputs:
//!
//! * `crates/talkbank-parser/src/generated_traversal.rs`, the generated
//!   typed CST traversal (itself produced by `tree-sitter-grammar-utils`), and
//! * `grammar/src/node-types.json`, tree-sitter's authoritative list of node
//!   kinds (the `"named": true` entries are the real, dispatchable kinds).
//!
//! Both the committed `inventory.rs` and the staleness guard
//! (`tests/integration/conformance_inventory_current.rs`) call [`generate_inventory`], so a
//! future visitor regeneration that forgets to regenerate the inventory fails
//! the test suite instead of silently drifting. The runnable entry point is the
//! `gen_conformance_inventory` example.
//!
//! # What is derived
//!
//! Below a fixed prelude (module docs, `#![allow(...)]`, `use` lines, and the
//! three `impl_inspect_*!` `macro_rules!` definitions, kept verbatim in
//! `PRELUDE`) the generator emits four mechanical sections, parsed out of the
//! typed traversal with `syn` into an [`Inventory`]:
//!
//! 1. **Leaf list**: one `impl_inspect_leaf!(...)` listing every
//!    `pub struct XxxNode<'tree>(tree_sitter::Node<'tree>);` tuple wrapper.
//! 2. **Choice impls**: one `impl_inspect_choice!(XxxChoice { .. });` per
//!    `pub enum XxxChoice<'tree, ..>`, variants in declaration order.
//! 3. **Struct impls**: one `impl_inspect_struct!(XxxChildren { .. });` per
//!    `pub struct XxxChildren<'tree, ..>`, positional fields in declaration
//!    order (the `trailing_extras` and `unexpected` sink fields are not
//!    inspected and are excluded).
//! 4. **Dispatch fn**: one match arm per public `extract_<snake>` free function whose
//!    `<snake>` is a `"named": true` node kind, keyed on the node kind. The arm
//!    form follows the function's first parameter type: a typed wrapper
//!    (`extract_x(classify::<XxxNode>(node))`) or a bare `tree_sitter::Node`
//!    (`extract_x(node)`).
//!
//! # Which reading of a shape is inspected
//!
//! A generated carrier or choice is declared once, generic over the axes it
//! has: a range phase `R: RangePhase<'tree> = Raw`, and, for a shape reaching
//! a slot the grammar can narrow, a kind proof `K: KindProof = Broad` before
//! it. Every axis has a default, so `X<'tree>` names the `Broad`, `Raw`
//! reading, which is the one `extract_<rule>` returns and therefore the only
//! one the dispatch below reaches. The inventory names every shape that way.
//! A shape whose generics are not exactly the tree lifetime followed by
//! defaulted axes is refused ([`InventoryGenError::UnadmittedShapeGenerics`])
//! rather than skipped, because a skipped shape is a position nothing
//! inspects. The `Admitted*` names are `pub type` aliases of the
//! `KindAdmitted` instantiation; they are not items of their own and get no
//! impl (one would be a second impl of the same shape under another proof the
//! dispatch never produces).
//!
//! # Layout, without a formatter
//!
//! The generator writes the final layout itself; no formatter runs at
//! generation or in the staleness guard, so the committed bytes depend on the
//! two inputs and on nothing installed on the machine. The layout is the one
//! rustfmt (edition 2024, default width heuristics) gives these constructs, so
//! the file stays clean under `cargo fmt --check`:
//!
//! * the leaf list is a call with one argument per line;
//! * a choice or struct macro call is one line when its member list is at
//!   most [`STRUCT_LIT_WIDTH`] columns and the whole line at most
//!   [`MAX_WIDTH`], and one member per line when the member list is wider.
//!   Those are rustfmt's `struct_lit_width` and `max_width`, and this rule
//!   reproduces every such call in the inventory rustfmt last produced. The
//!   remaining case, a narrow member list on a line too long for one line, no
//!   inventory has reached, and rustfmt's layout for it was never observed, so
//!   it is refused ([`InventoryGenError::UnverifiedLayout`]) rather than
//!   guessed;
//! * the dispatch arms are one per line, under `#[rustfmt::skip]` on the
//!   function: rustfmt's chain and long-call fallbacks for those arms are
//!   heuristics a generator should not imitate.

use std::collections::BTreeSet;
use std::path::PathBuf;

use syn::{Fields, FnArg, GenericParam, Item, Type, TypeParamBound};

/// rustfmt's default `max_width`: the widest line the macro-call layout emits
/// on one line.
pub const MAX_WIDTH: usize = 100;

/// rustfmt's default `struct_lit_width` (the width heuristic at `max_width`
/// 100): the widest member list a struct-literal-shaped macro call keeps on
/// one line.
pub const STRUCT_LIT_WIDTH: usize = 18;

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

    /// A macro call whose rustfmt layout was never observed: a member list
    /// narrow enough for one line on a line too long for one.
    #[error(
        "the inventory call for `{name}` has a layout rustfmt was never observed to produce; \
         verify rustfmt's layout for it and extend the generator"
    )]
    UnverifiedLayout {
        /// The shape's type name.
        name: String,
    },

    /// A carrier struct or `*Choice` enum declared generics other than the
    /// tree lifetime followed by defaulted range-phase and kind-proof axes, so
    /// `Name<'tree>` would not name the reading the dispatch produces.
    #[error(
        "generated shape `{name}` declares generics the inventory cannot name as `{name}<'tree>`"
    )]
    UnadmittedShapeGenerics {
        /// The shape's type name.
        name: String,
    },
}

/// The fixed head of `inventory.rs`: module docs, the `#![allow(...)]`, the
/// `use` lines, and the three `impl_inspect_*!` `macro_rules!` definitions. Kept
/// verbatim here (this is the "keep verbatim" region called out in the header);
/// everything after it is mechanically derived. Its layout is already the
/// formatted one, so it is emitted as written.
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
//! `just conformance-gen`.
//! The staleness guard `conformance_inventory_is_current` re-derives this file
//! from the current typed traversal + node-types.json and fails if the
//! committed copy has drifted, so a forgotten regen breaks the suite instead of
//! silently losing coverage. The harness lives in the parent module
//! `conformance`; the conformance test's allowlist in
//! `tests/integration/generated_traversal_conformance.rs`.

#![allow(clippy::too_many_lines)]

use crate::classify;
use crate::generated_traversal::*;

use super::{Inspect, InspectField, Observation, Position};

/// Generate a no-op `Inspect` for a leaf node wrapper: its own node kind is
/// separately visited by `walk_all` and dispatched below, so there is nothing
/// further to recurse into from here.
macro_rules! impl_inspect_leaf {
    ($($name:ident),* $(,)?) => {
        $(
            impl<'tree> Inspect for $name<'tree> {
                fn inspect(&self, _rule: &'static str, _out: &mut Vec<Observation>) {}
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
            fn inspect(&self, rule: &'static str, out: &mut Vec<Observation>) {
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
            fn inspect(&self, rule: &'static str, out: &mut Vec<Observation>) {
                $( self.$field.inspect_field(
                    rule,
                    Position::new(stringify!($name), stringify!($field)),
                    out,
                ); )*
            }
        }
    };
}
"#;

/// The blank line, doc comment and attributes stamped immediately before the
/// generated `dispatch` fn.
const DISPATCH_HEAD: &str = r"
/// Drive the generated `extract_*` for `node` if its kind has one, then
/// inspect the returned children. One arm per `extract_*` free function.
// Conformance assertions must fail the test on producer faults.
#[allow(clippy::expect_used)]
// One arm per line, as the generator lays them out; see the generator's
// module docs for why rustfmt's layout is not imitated here.
#[rustfmt::skip]
";

/// The axis a generated shape may declare after its `'tree` lifetime. Closed:
/// any other generic parameter refuses the shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShapeAxis {
    /// `R: RangePhase<'tree> = Raw`.
    Range,
    /// `K: KindProof = Broad`.
    Kind,
}

impl ShapeAxis {
    /// The axis `param` declares, when it is a defaulted type parameter bounded
    /// by exactly one of the two axis traits.
    fn of(param: &GenericParam) -> Option<Self> {
        let GenericParam::Type(param) = param else {
            return None;
        };
        param.default.as_ref()?;
        let mut bounds = param.bounds.iter();
        let (Some(TypeParamBound::Trait(bound)), None) = (bounds.next(), bounds.next()) else {
            return None;
        };
        let trait_name = bound.path.segments.last()?.ident.to_string();
        match trait_name.as_str() {
            "RangePhase" => Some(Self::Range),
            "KindProof" => Some(Self::Kind),
            _ => None,
        }
    }
}

/// Whether a shape can be named `Name<'tree>` for the reading the dispatch
/// produces: its first parameter is the `'tree` lifetime and every other one is
/// a defaulted axis, each at most once.
fn names_tree_reading(generics: &syn::Generics) -> bool {
    let mut params = generics.params.iter();
    let Some(GenericParam::Lifetime(lifetime)) = params.next() else {
        return false;
    };
    if lifetime.lifetime.ident != "tree" {
        return false;
    }
    let mut seen: Vec<ShapeAxis> = Vec::with_capacity(2);
    params.all(|param| match ShapeAxis::of(param) {
        Some(axis) if !seen.contains(&axis) => {
            seen.push(axis);
            true
        }
        Some(_) | None => false,
    })
}

/// One `*Choice` enum: its type name and its variant identifiers, in
/// declaration order.
#[derive(Debug, PartialEq, Eq)]
pub struct ChoiceImpl {
    name: String,
    variants: Vec<String>,
}

/// One `*Children` struct: its type name and its positional field identifiers,
/// in declaration order (the `trailing_extras` / `unexpected` sinks excluded).
#[derive(Debug, PartialEq, Eq)]
pub struct StructImpl {
    name: String,
    fields: Vec<String>,
}

impl StructImpl {
    /// Admit the generator's positional-carrier shape, not a name suffix
    /// shared with generic runtime wrappers. Minted carrier names may also
    /// have collision suffixes after `Children`.
    ///
    /// `Ok(None)` for a struct that is not carrier-shaped (no public named
    /// fields, or no `trailing_extras` and `unexpected` sinks: the runtime's
    /// `SourceChildren`, `LeafSpan` and the like). A carrier-shaped struct
    /// whose generics cannot be named `Name<'tree>` is an error, never a skip.
    fn from_carrier(item: &syn::ItemStruct) -> Result<Option<Self>, InventoryGenError> {
        let Fields::Named(named) = &item.fields else {
            return Ok(None);
        };
        if !named
            .named
            .iter()
            .all(|field| matches!(field.vis, syn::Visibility::Public(_)))
        {
            return Ok(None);
        }
        let names: Vec<_> = named
            .named
            .iter()
            .filter_map(|field| field.ident.as_ref().map(ToString::to_string))
            .collect();
        if !names.iter().any(|name| name == "trailing_extras")
            || !names.iter().any(|name| name == "unexpected")
        {
            return Ok(None);
        }
        // A runtime type with a second lifetime (a source binding) is not a
        // generated carrier. One with no lifetime at all falls through to the
        // generics check, which refuses it.
        if item.generics.lifetimes().count() > 1 {
            return Ok(None);
        }
        if !names_tree_reading(&item.generics) {
            return Err(InventoryGenError::UnadmittedShapeGenerics {
                name: item.ident.to_string(),
            });
        }
        Ok(Some(Self {
            name: item.ident.to_string(),
            fields: names
                .into_iter()
                .filter(|name| name != "trailing_extras" && name != "unexpected")
                .collect(),
        }))
    }

    /// The carrier's type name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The positional fields the generated impl inspects, in order.
    #[must_use]
    pub fn fields(&self) -> &[String] {
        &self.fields
    }
}

impl ChoiceImpl {
    /// Admit a `*Choice` enum, refusing generics `Name<'tree>` cannot name.
    fn from_enum(item: &syn::ItemEnum) -> Result<Option<Self>, InventoryGenError> {
        let name = item.ident.to_string();
        if !name.ends_with("Choice") {
            return Ok(None);
        }
        if !names_tree_reading(&item.generics) {
            return Err(InventoryGenError::UnadmittedShapeGenerics { name });
        }
        Ok(Some(Self {
            name,
            variants: item
                .variants
                .iter()
                .map(|variant| variant.ident.to_string())
                .collect(),
        }))
    }
}

/// How an `extract_<snake>` free function takes its node argument, which fixes
/// the shape of the generated dispatch arm.
#[derive(Debug, PartialEq, Eq)]
pub enum ExtractArg {
    /// First parameter is a bare `tree_sitter::Node`; the arm passes `node`.
    BareNode,
    /// First parameter is a typed wrapper `XxxNode`; the arm passes
    /// `classify::<XxxNode>(node)`. Holds the wrapper type name.
    TypedWrapper(String),
}

/// Whether the producer returns a carrier directly or admits reconstruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractReturn {
    /// Returns the carrier itself (supertype self-classification).
    Direct,
    /// Returns `Result<_, ReconstructionFault>`; the arm fails the test on
    /// `Err`.
    Fallible,
}

/// One dispatchable `extract_<snake>` free function.
#[derive(Debug, PartialEq, Eq)]
pub struct ExtractFn {
    /// The `extract_` suffix; equal to the node kind (the match-arm key).
    snake: String,
    /// The argument-passing shape.
    arg: ExtractArg,
    result: ExtractReturn,
}

impl ExtractFn {
    /// How the arm passes the node.
    #[must_use]
    pub fn arg(&self) -> &ExtractArg {
        &self.arg
    }

    /// Whether the arm unwraps a reconstruction result.
    #[must_use]
    pub fn result(&self) -> ExtractReturn {
        self.result
    }
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

/// Everything the inventory is rendered from, read once out of the typed
/// traversal. Built only by [`Inventory::parse`], whose orderings are the
/// rendered orderings, so [`Inventory::render`] has no decision left to make.
#[derive(Debug)]
pub struct Inventory {
    /// Leaf wrapper names, sorted byte-wise.
    leaves: Vec<String>,
    /// Choice enums, in source declaration order.
    choices: Vec<ChoiceImpl>,
    /// Carrier structs, in source declaration order.
    structs: Vec<StructImpl>,
    /// Dispatchable extractors for named kinds, sorted by kind.
    dispatch: Vec<ExtractFn>,
}

impl Inventory {
    /// Read the inventory out of the typed traversal source, keeping only the
    /// extractors whose kind `node_types_json` lists as named.
    pub fn parse(typed_src: &str, node_types_json: &str) -> Result<Self, InventoryGenError> {
        let file = syn::parse_file(typed_src)?;
        let named_kinds = named_node_kinds(node_types_json)?;

        let mut leaves: Vec<String> = Vec::new();
        let mut choices: Vec<ChoiceImpl> = Vec::new();
        let mut structs: Vec<StructImpl> = Vec::new();
        let mut dispatch: Vec<ExtractFn> = Vec::new();

        for item in &file.items {
            match item {
                Item::Struct(item_struct) => match &item_struct.fields {
                    // Leaf node wrapper: `pub struct XxxNode<'tree>(tree_sitter::Node<'tree>);`
                    Fields::Unnamed(unnamed)
                        if unnamed.unnamed.len() == 1
                            && is_tree_sitter_node(&unnamed.unnamed[0].ty) =>
                    {
                        leaves.push(item_struct.ident.to_string());
                    }
                    // Synthetic children carrier: `pub struct XxxChildren<'tree, ..> { .. }`
                    Fields::Named(_) => {
                        structs.extend(StructImpl::from_carrier(item_struct)?);
                    }
                    Fields::Unnamed(_) | Fields::Unit => {}
                },
                Item::Enum(item_enum) => choices.extend(ChoiceImpl::from_enum(item_enum)?),
                Item::Fn(item_fn) => {
                    // Private proof-carrying helpers are not raw-node entrypoints.
                    // Visibility, not a suffix convention, defines that boundary.
                    if !matches!(item_fn.vis, syn::Visibility::Public(_)) {
                        continue;
                    }
                    let fn_name = item_fn.sig.ident.to_string();
                    let Some(snake) = fn_name.strip_prefix("extract_") else {
                        continue;
                    };
                    let arg = classify_extract_arg(snake, item_fn)?;
                    // A dispatch arm is emitted only for extract fns whose kind
                    // is a real named node kind; internal sub-rule extracts
                    // (reached only by recursion, never by `node.kind()`) get
                    // none.
                    if named_kinds.contains(snake) {
                        dispatch.push(ExtractFn {
                            snake: snake.to_owned(),
                            arg,
                            result: extract_return(&item_fn.sig.output),
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
        // * Choice / struct impls: kept in source DECLARATION order (no sort).
        //   The `file.items` walk already preserves source order, and grouping
        //   related synthetic `*Choice` / `*Children` types the way the
        //   generator emitted them keeps the file diff-stable against a visitor
        //   regen.
        // * Dispatch arms: sorted by node kind (the match-arm key).
        //
        // Within a single item, order is always source declaration order (a
        // choice's variants, a struct's fields).
        leaves.sort();
        dispatch.sort_by(|a, b| a.snake.cmp(&b.snake));

        Ok(Self {
            leaves,
            choices,
            structs,
            dispatch,
        })
    }

    /// The carrier named `name`, if the traversal declares it as one.
    #[must_use]
    pub fn carrier(&self, name: &str) -> Option<&StructImpl> {
        self.structs.iter().find(|carrier| carrier.name == name)
    }

    /// The dispatch arm for node kind `kind`, if it has one.
    #[must_use]
    pub fn dispatch_arm(&self, kind: &str) -> Option<&ExtractFn> {
        self.dispatch.iter().find(|extract| extract.snake == kind)
    }

    /// Render the complete `inventory.rs`, in its final layout: the verbatim
    /// prelude followed by the four mechanical sections.
    ///
    /// # Errors
    ///
    /// [`InventoryGenError::UnverifiedLayout`] for a call whose rustfmt
    /// layout was never observed.
    pub fn render(&self) -> Result<String, InventoryGenError> {
        let mut out = String::with_capacity(160 * 1024);
        out.push_str(PRELUDE);

        // 1. Leaf list: one argument per line.
        out.push_str("\n// --- leaf node wrapper impls ---\nimpl_inspect_leaf!(\n");
        for leaf in &self.leaves {
            out.push_str("    ");
            out.push_str(leaf);
            out.push_str(",\n");
        }
        out.push_str(");\n");

        // 2. Choice impls.
        out.push_str("\n// --- *Choice enum impls ---\n");
        for choice in &self.choices {
            push_struct_literal_call(
                &mut out,
                "impl_inspect_choice",
                &choice.name,
                &choice.variants,
            )?;
        }

        // 3. Struct impls.
        out.push_str("\n// --- *Children struct impls ---\n");
        for struct_impl in &self.structs {
            push_struct_literal_call(
                &mut out,
                "impl_inspect_struct",
                &struct_impl.name,
                &struct_impl.fields,
            )?;
        }

        // 4. Dispatch fn, one arm per line.
        out.push_str(DISPATCH_HEAD);
        out.push_str(
            "pub fn dispatch(node: tree_sitter::Node, out: &mut Vec<Observation>) {\n    match node.kind() {\n",
        );
        for extract in &self.dispatch {
            push_dispatch_arm(&mut out, extract);
        }
        out.push_str("        _ => {}\n    }\n}\n");

        Ok(out)
    }
}

/// Emit `macro_name!(name { members });` in rustfmt's struct-literal layout:
/// one line when the member list is at most [`STRUCT_LIT_WIDTH`] columns and
/// the line at most [`MAX_WIDTH`], one member per line (no comma after the
/// last) when the member list is wider, and a refusal for the unobserved case
/// between them.
fn push_struct_literal_call(
    out: &mut String,
    macro_name: &str,
    name: &str,
    members: &[String],
) -> Result<(), InventoryGenError> {
    let body = members.join(", ");
    // `macro!(` + name + ` { ` + body + ` });`
    let one_line_width =
        macro_name.chars().count() + 2 + name.chars().count() + 3 + body.chars().count() + 4;
    let narrow = body.chars().count() <= STRUCT_LIT_WIDTH;
    if narrow && one_line_width > MAX_WIDTH {
        return Err(InventoryGenError::UnverifiedLayout {
            name: name.to_owned(),
        });
    }
    if narrow {
        out.push_str(macro_name);
        out.push_str("!(");
        out.push_str(name);
        out.push_str(" { ");
        out.push_str(&body);
        out.push_str(" });\n");
    } else {
        out.push_str(macro_name);
        out.push_str("!(");
        out.push_str(name);
        out.push_str(" {\n");
        // rustfmt writes no trailing comma after the last member here.
        let mut members = members.iter().peekable();
        while let Some(member) = members.next() {
            out.push_str("    ");
            out.push_str(member);
            out.push_str(if members.peek().is_some() {
                ",\n"
            } else {
                "\n"
            });
        }
        out.push_str("});\n");
    }
    Ok(())
}

/// Emit one dispatch arm on one line.
fn push_dispatch_arm(out: &mut String, extract: &ExtractFn) {
    out.push_str("        \"");
    out.push_str(&extract.snake);
    out.push_str("\" => extract_");
    out.push_str(&extract.snake);
    match &extract.arg {
        ExtractArg::BareNode => out.push_str("(node)"),
        ExtractArg::TypedWrapper(wrapper) => {
            // CLASSIFY, do not assert: the wrapper's field is private, so
            // `from_node` compares the node against the kind literal the
            // generator emitted rather than trusting the arm key. The panic
            // lives once, in `classify`, instead of 160 times here.
            out.push_str("(classify::<");
            out.push_str(wrapper);
            out.push_str(">(node))");
        }
    }
    match extract.result {
        ExtractReturn::Direct => {}
        // A producer fault must fail conformance, never silently drop a node.
        ExtractReturn::Fallible => {
            out.push_str(".expect(\"generated reconstruction must preserve its selected plan\")");
        }
    }
    out.push_str(".inspect(\"");
    out.push_str(&extract.snake);
    out.push_str("\", out),\n");
}

/// Produce the complete `inventory.rs` source from the two committed inputs.
/// This is the function both the `gen_conformance_inventory` example and the
/// `conformance_inventory_is_current` guard call, so their outputs are
/// identical by construction.
///
/// * `typed_src`: the text of `generated_traversal.rs`.
/// * `node_types_json`: the text of `grammar/src/node-types.json`.
pub fn generate_inventory(
    typed_src: &str,
    node_types_json: &str,
) -> Result<String, InventoryGenError> {
    Inventory::parse(typed_src, node_types_json)?.render()
}

/// Whether an extractor returns its carrier or a reconstruction `Result`.
fn extract_return(output: &syn::ReturnType) -> ExtractReturn {
    match output {
        syn::ReturnType::Type(_, ty)
            if matches!(ty.as_ref(), Type::Path(path)
                if path.path.segments.last().is_some_and(|s| s.ident == "Result")) =>
        {
            ExtractReturn::Fallible
        }
        syn::ReturnType::Type(..) | syn::ReturnType::Default => ExtractReturn::Direct,
    }
}

/// Parse `node-types.json` and collect the set of `"named": true` node kinds.
fn named_node_kinds(node_types_json: &str) -> Result<BTreeSet<String>, InventoryGenError> {
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
        .join("src")
        .join("conformance")
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

#[cfg(test)]
mod tests {
    use super::{Inventory, InventoryGenError};

    #[test]
    fn inventory_dispatch_respects_extractor_visibility() {
        let private = "fn extract_proof(parent: AdmittedParent) {}";
        assert!(Inventory::parse(private, "[]").is_ok());
        let restricted = "pub(crate) fn extract_proof(parent: AdmittedParent) {}";
        assert!(Inventory::parse(restricted, "[]").is_ok());
        let public = "pub fn extract_proof(parent: AdmittedParent) {}";
        assert!(matches!(
            Inventory::parse(public, "[]"),
            Err(InventoryGenError::ExtractUnexpectedParam { .. })
        ));
    }

    /// The struct-literal rule at its member-width limit: a member list of
    /// exactly the limit stays on one line and one column more does not.
    #[test]
    fn macro_call_layout_breaks_one_column_past_the_member_width() {
        let mut out = String::new();
        let at_limit = vec!["abcdefgh".to_owned(), "ijklmnop".to_owned()]; // 18 wide
        super::push_struct_literal_call(&mut out, "impl_inspect_choice", "X", &at_limit)
            .expect("observed layout");
        assert_eq!(out, "impl_inspect_choice!(X { abcdefgh, ijklmnop });\n");
        out.clear();
        let past_limit = vec!["abcdefgh".to_owned(), "ijklmnopq".to_owned()]; // 19 wide
        super::push_struct_literal_call(&mut out, "impl_inspect_choice", "X", &past_limit)
            .expect("observed layout");
        assert_eq!(
            out,
            "impl_inspect_choice!(X {\n    abcdefgh,\n    ijklmnopq\n});\n"
        );
    }

    /// The line-width limit, which no inventory has reached: a narrow member
    /// list on a line one column too long for one line is refused, not laid
    /// out by a guess, and one column shorter is one line.
    #[test]
    fn a_narrow_call_too_long_for_one_line_is_refused() {
        let members = vec!["a".to_owned()];
        // `impl_inspect_choice!(` (21) + name + ` { a });` (8) = 100 at 71.
        let fits = "N".repeat(71);
        let mut out = String::new();
        super::push_struct_literal_call(&mut out, "impl_inspect_choice", &fits, &members)
            .expect("a 100-column line is one line");
        assert_eq!(out.trim_end().chars().count(), super::MAX_WIDTH);
        let too_long = "N".repeat(72);
        assert!(matches!(
            super::push_struct_literal_call(&mut out, "impl_inspect_choice", &too_long, &members),
            Err(InventoryGenError::UnverifiedLayout { name }) if name == too_long
        ));
    }
}
