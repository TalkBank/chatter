# send2clan

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/send2clan/README.md)

Rust bindings for [send2clan](https://talkbank.org/), enables sending file open messages to the CLAN (Computerized Language Analysis) application.

## Overview

This crate provides idiomatic Rust bindings around the C `send2clan` library.
When invoked, it launches CLAN (if not already running) and sends a file path
with cursor position and optional error text so the user can jump directly to
the relevant location.

- **Platform implementations**: macOS and Windows; other platforms return an
  unsupported-platform error after parameter validation, not a successful no-op.
- **Stateless**: no context or configuration management needed

## Integration limits

Treat `Ok(())` as the bridge's transport result, not confirmation that the GUI
opened the intended file or positioned its cursor. The combined formatted
request uses a fixed buffer and can truncate long messages.

Do not issue overlapping Windows sends: they share one `CLAN_Message.txt`
without coordination, including across processes. Caller-local serialization
does not prevent another process from replacing that file. The timeout parameter
controls selected platform waits, not an end-to-end deadline; in particular,
Windows `SendMessageA` has no timeout in this implementation.

Availability and capability results are hints, not launch/delivery proofs.
On Windows the bridge admits the registered `InstallPath` (status, `REG_SZ`
type, termination, embedded NULs and `MAX_PATH` capacity) before probing or
launching anything, and falls back to the default path only when no TalkBank
key exists. `just send2clan-win32-doubles` verifies that admission by compiling
the real C source against deterministic Win32 doubles. That is not native
Windows qualification; do not treat the platform as qualified by the Rust
wrapper or the capability bits.
Native application interaction requires separate verification. Avoid automatic
retries unless duplicate GUI requests and partial effects are acceptable.

## Usage

```rust,no_run
use send2clan::send_to_clan;

fn main() -> Result<(), send2clan::Error> {
    // Request a 30-second platform wait, not a whole-operation deadline.
    send_to_clan(30, "/path/to/file.cha", 42, 10, Some("Syntax error"))?;
    println!("Successfully sent file to CLAN!");
    Ok(())
}
```

Helper functions:

- `is_platform_supported()`: check if the current OS is supported
- `is_clan_available()`: query the platform's installation-location heuristic
- `version()`: get library version string
- `get_capabilities()`: inspect legacy flags, not an executed capability test

## Build Requirements

A C compiler is required (the crate compiles the bundled C source via `cc`).

## License

MIT OR Apache-2.0.
