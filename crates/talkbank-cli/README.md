# talkbank-cli

**Last modified:** 2026-06-15 20:38 EDT

`chatter` is the command-line interface for [CHAT format](https://talkbank.org/0info/manuals/CHAT.html)
validation, normalization, conversion, and cache inspection.

Prebuilt binaries and install scripts for macOS, Linux, and Windows are
attached to each release on the [Releases page](https://github.com/TalkBank/chatter/releases).
crates.io publication is intentionally deferred (`publish = false`) until the
library APIs stabilize.

## Common Commands

```bash
chatter validate file.cha
chatter validate corpus/ --format json
chatter normalize file.cha -o normalized.cha
chatter to-json file.cha -o file.json
chatter from-json file.json -o file.cha
chatter lint corpus/ --fix
chatter cache stats
chatter schema
```

See the book’s CLI reference for the verified public surface and support posture.

## Installation

Most users install a prebuilt binary from the
[Releases page](https://github.com/TalkBank/chatter/releases) (the install
scripts attached to each release handle macOS, Linux, and Windows).

To build from a checkout of this repository instead:

```bash
cargo install --path crates/talkbank-cli --locked
# or, for day-to-day development:
cargo run -p talkbank-cli -- validate file.cha
```

## License

MIT OR Apache-2.0.
