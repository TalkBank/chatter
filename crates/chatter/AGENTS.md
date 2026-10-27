# CLI crate agent guidance

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/chatter/AGENTS.md)

Read the [root guidance](../../AGENTS.md) first. This file adds the CLI's
ownership and boundary rules; it does not create separate gates.

## What this crate is

The `chatter` executable (Cargo package `talkbank-chatter`): argument parsing
(`cli/`), one module per command (`commands/`), output and progress
(`output.rs`, `progress.rs`, `stdout.rs`), exit codes (`exit_codes.rs`) and
the validation TUI (`ui/validation_tui/`). It is an adapter over
`talkbank-transform`: it owns the command surface, formatting and process
behavior, not CHAT parsing, validation semantics or rewrites.
Reference: [CLI startup and the program stack](../../book/src/architecture/cli-startup-and-stack.md),
the book's CLI reference, [error diagnostics UX](../../book/src/architecture/errors-and-validation/error-diagnostics-ux.md).

## Surfaces that are contracts

- Command names, flags, exit codes and the JSON / NDJSON output are public
  surfaces on the road to 1.0. A change to any of them is a breaking-change
  note in `CHANGELOG.md`, and `exit_codes.rs` is the one owner of what each
  outcome returns. The desktop app and `send2clan` call the same shared
  functions this crate calls; do not add a CLI-local domain step they cannot
  reach.
- `stdout` carries the command's result only (`stdout.rs`); progress, cache
  housekeeping and diagnostics go to stderr or, in JSON mode, to typed
  records. A new record type is a typed model serialized once, never a
  concatenated string.
- Diagnostics are rendered through `talkbank_transform::render_diagnostics()`;
  the TUI (`ui/validation_tui/`) is the reference presentation the desktop app
  must match, so a display change here is a desktop parity item.
- `main()` moves the program onto a 16 MiB thread; program logic never runs
  on the bare OS main thread (`tests/integration/stack_limit_tests.rs`).

## Behavior rules

- No panics in long-lived code: typed errors from the command through
  `exit_codes`, never `unwrap`/`expect` in production paths.
- A command that writes files is report-first or writes elsewhere: a bare
  `fix` reports and `--apply` writes (`cli/args/fix_mode.rs`, a typed
  `Flag<FixMode>`, not a boolean); `format` and `convert to-json` write to
  `--output`; the in-place experimental transforms preview unless `--apply`
  is passed. A new writing
  command follows one of those shapes and never overwrites its input on the
  strength of a returned model alone.
- The validation cache is the user's; isolate it in tests through
  `CliHarness`, never `HOME` tricks, and never delete it without an explicit
  request.
- Parser selection (`--parser re2c`) and validation options map onto
  `ValidationConfig`; both backends go through the same runner.

## Verification

`cargo test -p talkbank-chatter --bin chatter` and `tests/integration/` (subprocess
tests through `CliHarness`, exit-code and JSON-shape contracts, the stack
control). A behavior change needs a real subprocess test at the CLI
boundary, not only a unit test of the function behind it.
