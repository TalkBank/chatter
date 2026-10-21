# talkbank-derive

**Status:** Current
**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/talkbank-derive/README.md)

Procedural derive macros for the TalkBank CHAT Rust model crates.

## Overview

`talkbank-derive` centralizes code generation that would otherwise be repeated
across the TALKBank AST/model crates. It is primarily intended for
`talkbank-model` and sibling crates that share the same `crate::model` /
`talkbank_model` path layout.

Provided macros:

| Macro | Generates | Purpose |
| --- | --- | --- |
| `#[derive(SemanticEq)]` | `SemanticEq` + `SemanticDiff` | Compare AST nodes while ignoring spans or other metadata |
| `#[derive(SpanShift)]` | `SpanShift` | Shift byte-offset spans after source edits |
| `#[derive(ValidationTagged)]` | `ValidationTagged` | Classify enum variants as clean, warning, or error |
| `#[error_code_enum]` | attribute expansion | Generate exact parsing, ordered enumeration, status lists and serde glue |

## Consumer contract

The generated impls intentionally target paths under `crate::model` and
`talkbank_model`. That makes the crate a good fit for the TalkBank workspace, but
it is not a fully generic derive toolkit for arbitrary unrelated crate layouts.

## Usage

The block below is `ignore`d rather than compiled, and the reason is the
limitation stated two paragraphs above: the generated impls reference
`crate::model`, so this cannot build in a crate that has no such module. It is
illustrative by necessity. Every OTHER ```rust block in this README is compiled
by `cargo test --doc` through `ReadmeDoctests` in `src/lib.rs`; keep new
examples compilable so they cannot rot.

```rust,ignore
use talkbank_derive::{SemanticEq, SpanShift};

#[derive(SemanticEq, SpanShift)]
struct MyNode {
    content: String,
    #[semantic_eq(skip)]
    #[span_shift(skip)]
    debug_info: Option<String>,
}
```

Common field-level controls:

- `#[semantic_eq(skip)]`: ignore a field during semantic comparison and diffing
- `#[span_shift(skip)]`: exclude a field from recursive span shifting

Variant-level `#[validation_tag(clean)]`, `#[validation_tag(warning)]` and
`#[validation_tag(error)]` override the naming policy. Unknown, malformed,
duplicate, container or field annotations are compile errors. Tagging accepts
generic enums and all payload shapes without adding payload trait bounds.

The skip controls are field-only and accept exactly one `skip` identifier.
Unknown flags, malformed arguments, repeated attributes and container/variant
placement are compile errors, not implicit inclusion. `SemanticEq` admits one
field plan shared with its companion `SemanticDiff`; `SpanShift` admits its own
policy. Generated enum bindings do not depend on user field spelling, including
raw identifiers and names such as `offset` or `delta`.
Recursive field operations use qualified trait calls: an inherent method named
`semantic_eq`, `semantic_diff_into` or `shift_spans_after` cannot override the
corresponding trait operation inside a generated implementation.

The derives accept zero-variant enums as uninhabited types, emitting
an exhaustive match on the dereferenced receiver. They do not fabricate a
variant name, comparison result, validation tag or span movement for an impossible value.
One-variant enums retain ordinary field comparison, diff and shifting behavior.

`SpanShift` infers bounds on type parameters referenced by participating field
types, not parameters used only in skipped fields. Explicit bounds and where
clauses remain intact. Opaque type macros conservatively retain bounds on every
type parameter. These are parameter bounds, not minimal bounds inferred from
resolved wrapper or associated types.

Diff context selection is unchanged: among syntactically span-like named fields,
prefer `span`, then `speaker_span`, then the last candidate. Tuple enum variants
use the first candidate; tuple structs do not select context. This is syntax
recognition of `Span` / `Option<Span>`, not resolved Rust type identity. Skipping
semantic comparison does not discard a selected field's diff context.

## Error code registry

`error_code_enum` accepts no macro arguments and admits only unit variants,
without generic parameters, where clauses or explicit discriminants. Each variant
needs exactly one `#[code("...")]` literal, ascending by the existing parsed
prefix/numeric key; `UnknownError` is required. An optional `#[status(planned)]`
or `#[status(opt_in)]` selects one enforcement state. Absence means Active;
malformed, duplicate and misplaced metadata is not absence.

Variant `cfg` and nested `cfg_attr` gates apply to the generated enum, lookup
arms and fixed-size `all`/`planned`/`opt_in` arrays. The consuming compiler
evaluates predicates; the macro does not evaluate target configuration on its
host. Metadata and ordering apply to all declarations seen by the macro, even
conditionally disabled ones; code/status annotations must be unconditional.
Every enabled registry must retain `UnknownError`. Conditional non-configuration
attributes remain on their original declaration, not on generated match arms.

```rust
use talkbank_derive::error_code_enum;

#[error_code_enum]
enum Code {
    #[code("E001")]
    Active,
    #[code("E002")]
    #[status(planned)]
    Planned,
    #[code("E003")]
    #[status(opt_in)]
    OptIn,
    #[code("E999")]
    UnknownError,
}

assert_eq!(Code::parse_exact("E001"), Some(Code::Active));
assert_eq!(Code::parse_exact("E1"), None);
assert_eq!(Code::planned(), &[Code::Planned]);
assert_eq!(Code::opt_in(), &[Code::OptIn]);
assert_eq!(Code::Active.documentation_url(), "https://talkbank.org/errors/E001");
assert_eq!(Code::all().len(), Code::iter().count());
```

The admitted registry owns code literals and exclusive status values before
ordering and completeness admission. The emitter accepts only the complete
owner. `as_str`, `parse_exact`, Display, `documentation_url`, `all`, `iter`,
`planned`, `opt_in` and serde retain their canonical code strings and declaration
order. There is no coercing `new` constructor.

## License

MIT OR Apache-2.0.
