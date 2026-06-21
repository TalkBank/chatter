# talkbank-derive

**Status:** Current
**Last modified:** 2026-05-30 19:33 EDT

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
| `#[error_code_enum]` | attribute expansion | Generate `as_str()`, `new()`, `Display`, and serde glue for error code enums |

## Consumer contract

The generated impls intentionally target paths under `crate::model` and
`talkbank_model`. That makes the crate a good fit for the TalkBank workspace, but
it is not a fully generic derive toolkit for arbitrary unrelated crate layouts.

## Usage

```rust
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
- `#[validation_tag(error)]` / `#[validation_tag(warning)]`, override
  convention-based `ValidationTagged` classification

## License

MIT OR Apache-2.0.
