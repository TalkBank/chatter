# chatter

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/chatter/README.md)

`chatter` is the command-line interface for [CHAT format](https://talkbank.org/0info/manuals/CHAT.html)
validation, normalization, conversion, and cache inspection.

Prebuilt binaries and install scripts for macOS, Linux, and Windows are
attached to each release on the [Releases page](https://github.com/TalkBank/chatter/releases).
The Cargo package is `talkbank-chatter`; its executable remains `chatter`.
Registry publication is a deliberate maintainer action, not implied by package
metadata. Actual registry installation must be verified for the published
candidate; see the [publication contract](https://github.com/TalkBank/chatter/blob/main/book/src/contributing/crates-io-publication.md).

## Common Commands

```bash
chatter validate file.cha
chatter validate corpus/ --format json
chatter normalize file.cha -o normalized.cha
chatter to-json file.cha -o file.json
chatter from-json file.json -o file.cha
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
cargo install --path crates/chatter --locked
# or, for day-to-day development:
cargo run -p talkbank-chatter -- validate file.cha
```

## License

MIT OR Apache-2.0.
