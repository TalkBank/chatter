# Coding Standards

**Status:** Current
**Last updated:** 2026-03-24 00:01 EDT

## Rust Conventions

- **Edition**: 2024
- **Formatting**: `cargo fmt` before every commit
- **Linting**: `cargo clippy --all-targets -- -D warnings` must pass with zero warnings
- **No clippy silencing** without explicit approval

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
- Use `cargo nextest run` for unit tests (except doctests)
- Snapshot tests with `insta` for complex output comparisons

## Generated Files

Never hand-edit generated artifacts:
- `parser.c`: generated from `grammar.js`
- `grammar/test/corpus/`: generated from specs
- `crates/talkbank-parser-tests/tests/generated/`: generated from specs
- `crates/talkbank-model/src/generated/symbol_sets.rs`: generated from symbol registry

Always regenerate from source inputs.
