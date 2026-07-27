# Coding Standards

**Status:** Current
**Last updated:** 2026-07-25 22:40 EDT

## Rust Conventions

- **Edition**: 2024
- **Formatting**: `cargo fmt` before every commit
- **Linting**: CI owns clippy (single pass, no flags; the workspace `[lints.clippy]` table denies only the panic family). Do not run clippy as a local habit; see the clippy policy in the root CLAUDE.md.

## Error Handling

- No panics for recoverable conditions, use `thiserror`/`miette` for error types
- Library code uses the `ErrorSink` trait for error reporting, not `Result`
- Use `ParseOutcome<T>` in parser code (parsed or rejected)

## Logging

- Library crates use `tracing` (never `println!` or `eprintln!`)
- CLI binaries write to stdout (results) and stderr (diagnostics)
- Use appropriate log levels: `error!`, `warn!`, `info!`, `debug!`, `trace!`

## Naming

- Follow standard Rust conventions (snake_case for functions, CamelCase for types)
- Conventional Commits for commit messages: `<type>[scope]: <description>`
  - Types: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`

## Dependencies

Preferred crates:
- `clap`: CLI argument parsing
- `serde`: serialization
- `miette`: user-facing diagnostics
- `insta`: snapshot testing
- `tracing`: structured logging
- `rayon` / `crossbeam`, concurrency
- `smallvec`: small-buffer optimization

## Code Organization

- Keep crate boundaries clean, lower crates should not depend on higher ones
- The model crate should not depend on any parser
- Parsing code should not depend on serialization/transform code
- All CHAT parsing and serialization goes through the AST, never ad-hoc string manipulation
- Treat 10 or more named struct fields as an audit trigger. Wide boundary or
  report records can be acceptable, but wide runtime state bags need explicit
  review. See `architecture/chat-model/wide-structs.md`.

## Testing

- Prefer spec-driven tests over hand-written tests for parser behavior
- Use `cargo test` for unit tests (except doctests)
- Snapshot tests with `insta` for complex output comparisons

## Generated Files

Never hand-edit generated artifacts:
- `parser.c`: generated from `grammar.js`
- `grammar/test/corpus/`: generated from specs
- `crates/talkbank-parser-tests/tests/generated/`: generated from specs
- `crates/talkbank-model/src/generated/symbol_sets.rs`: generated from symbol registry

Always regenerate from source inputs.

## Full Rust Standards Charter (canonical)

### Edition and Tooling

- Rust **2024 edition**.
- `cargo fmt` before committing. Use `cargo fmt` (not standalone
  `rustfmt`) for workspace-consistent formatting.
- **Prefer `cargo test`** for faster parallel-per-test
  execution. Use `cargo test --doc` for doctests (they are not part of the normal run
  those).
- CI runs **single-pass clippy** (`--workspace --all-targets`, no
  flags): the workspace `[lints]` table denies the panic family in
  production code; test code relaxes it via in-source attributes. Red
  means a panic-policy violation, nothing else. See the clippy policy
  section above.

### Error Handling

- **No panics for recoverable conditions.** Use typed errors
  (`thiserror`); use `miette` for rich diagnostics where appropriate.
- **No silent swallowing.** Every unexpected condition must be
  handled with explicit error reporting, no `.ok()`,
  `.unwrap_or_default()`, or silent fallbacks that hide bugs.

### Output and Logging

- **Library crates:** `tracing` macros (`tracing::info!`,
  `tracing::warn!`, etc.), never `println!`/`eprintln!`.
- **CLI binaries:** `println!`/`eprintln!` for user-facing output;
  `tracing` for debug logging.
- **Test code:** `println!` is acceptable (cargo captures it).

### Lazy Initialization

- `LazyLock<Regex>` (from `std::sync`) for constant regex patterns.
  Never call `Regex::new()` inside functions or loops.
- `OnceLock` for per-instance memoization of runtime-determined
  values.
- Prefer `const` when possible (even better than lazy).
- All lazy init via `std::sync`, no external crate dependencies
  needed.

### Type Design

- **No boolean blindness.** Enums over bools for anything beyond
  simple on/off. This is a hard rule.
  - **Banned:** 2+ bool parameters on a function, 2+ related bool
    fields on a struct, opposite bool pairs (`foo`/`no_foo`), bool
    return where meaning is unclear without reading docs.
  - `#[derive(Default, clap::ValueEnum)]` enum with named variants.
    For clap CLI args, use `#[arg(value_enum)]` instead of
    `--flag`/`--no-flag` pairs.
  - **OK as bool:** `verbose`, `force`, `quiet`, `dry_run`, single
    `include_*`/`skip_*` flags, anything where the parameter name
    fully communicates what `true` means.
- **`BTreeMap` for deterministic JSON** in tests and snapshot tests
  (not `HashMap`). Ensures consistent, reviewable diffs.
- Prefer explicit enums over ambiguous `Option` when there are
  multiple meaningful states.

### Newtypes Over Primitives

- **No primitive obsession.** Domain values must have domain types.
  Function signatures should be self-documenting through type
  names, not parameter names.
- Use newtype structs (e.g., `struct TimestampMs(u64)`,
  `struct SpeakerId(String)`) or the `interned_newtype!` /
  `string_newtype!` macros from `talkbank-model`. Newtypes should
  implement `Display`, `From`/`Into` for the underlying type, and
  derive `Clone`, `Debug`, `PartialEq`, `Eq` as appropriate.
- **Scope:** Applies to public API boundaries, struct fields, and
  function signatures. Local variables inside a function body may
  use bare primitives when the context is unambiguous.
- **Parsing boundaries:** Parse raw strings into newtypes at the
  boundary (file I/O, CLI args, IPC). Interior code should never
  handle raw strings for typed values.
- **No ad-hoc format parsing.** Use real parsers (JSON:
  `serde_json`, etc.) not regex or string splitting for
  structured formats. Regex is appropriate only for flat text
  pattern matching (search, normalization, validation of simple
  formats).

### Integer Discipline

- **Distinguish meaning.** Not all `usize` values are
  interchangeable. Separate:
  - **Index**: position into a collection (`UtteranceIndex`,
    `GraIndex`)
  - **Count**: accumulated quantity (`WordCount`,
    `UtteranceCount`)
  - **Limit**: upper bound for iteration or reporting
    (`UtteranceLimit`, `WordLimit`)
  - **Threshold**: minimum value for inclusion
    (`FrequencyThreshold`)
  - **ID**: opaque identifier (`NodeId`, `SpeakerIndex`)
- Non-negative quantities use unsigned types; newtypes enforce
  domain semantics.
- **No bare numeric literals** except `0`, `1`, and simple loop
  bounds. All other numbers must be named constants. Assess whether
  each constant should be configurable.

### Closed-Set Strings and Constants

- **Closed sets must be enums.** If a string value comes from a
  known finite set (tier labels, command names, output formats),
  represent it as an `enum` with a `FromStr` parser and `Display`
  serializer. Use `Other(String)` escape hatch only when the set is
  genuinely extensible.
- **All remaining string literals must be defined constants.** No
  scattered `"mor"` or `"cod"` strings, use `TierKind::Mor` or
  `const DEFAULT_TIER: &str = "cod"`.
- **Config defaults:** Use `const` values or enum variants in
  `Default` impls, not `"string".to_owned()` (avoids runtime
  allocation, makes the default visible at the type level).

### File Path Discipline

- File paths use `PathBuf`/`&Path`, never `String`. Convert to
  strings only at display/serialization boundaries via `.display()`
  or `.to_string_lossy()`.
- Distinguish base filename (e.g., `MediaFilename` newtype, no
  extension) from full filesystem path (`PathBuf`).
- Use `.display()` for user-facing output; `.to_string_lossy()`
  only for cache keys or hashing.

### Configurability

- Hardcoded thresholds and limits belong in config struct fields
  with documented defaults.
- If a default is useful to change per-invocation → CLI flag.
- If a default is useful to change per-user → future `defaults.toml`
  file (not yet implemented).
- Config structs must be constructible in tests without filesystem
  or network access.

### Rustdoc as Primary Documentation

- **Types are the primary documentation layer.** A reader of
  crates.io rustdocs should understand the domain by reading type
  definitions alone.
- Every `pub` type and function must have a doc comment explaining
  role, ownership, invariants, and CHAT manual references where
  applicable.
- Newtypes must document valid values, units, and meaningful
  operations.
- Enum variants must document when each variant applies.

### File Size Limits

- **Recommended:** ≤400 lines per file.
- **Hard limit:** ≤800 lines per file (must be split).

### Testability

- **No global mutable state.** All command state flows through
  explicit `State` types (the `AnalysisCommand` trait pattern).
  Enforce this going forward.
- Config structs must be constructible in tests without filesystem,
  network, or environment setup.
- Stateful resources (caches, pools, registries) must accept
  injected dependencies for test control.

### Refactoring Triggers

Stop and refactor when you see:

- `x: i32, y: i32` for domain data → use domain structs
- `start_ms: u64, end_ms: u64` → use `TimestampMs` newtype or
  `TimeSpan` struct
- `fn foo(lang: &str, speaker: &str, path: &str)` → use
  `LanguageCode`, `SpeakerId`, typed path
- Multiple booleans for state → use enum with variants
- `fn foo(a: bool, b: bool)` or `--flag`/`--no-flag` pairs → use
  enum with `clap::ValueEnum`
- `fn parse() -> Option<T>` where failure reason matters → use
  `Result<T, ParseError>`
- `match s { "win" => ... }` on raw strings → parse to `enum` at
  boundary
- `"mor"` or `"cod"` string literals → use `TierKind::Mor` or
  `TierKind::Cod`
- `limit: usize` or `max_X: usize` → use domain-specific newtype
  (`UtteranceLimit`, `WordLimit`)
- Bare `0.5` or `60` in logic → named constant or config field
- Regex or `split()`/`find()` on XML, JSON, or other structured
  formats → use a proper parser
