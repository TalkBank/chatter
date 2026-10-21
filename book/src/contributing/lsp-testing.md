# LSP Testing and Diagnosis

**Status:** Current
**Last modified:** {{git-dates:page}}

For editor setup and a first diagnostic/edit cycle, use
[Editor Integration](../chatter/user-guide/editor-integration.md). This page is
for contributors investigating the server or its protocol boundary.

## Start with the observed boundary

Distinguish failure to launch, failure to initialize, failure to attach a CHAT
buffer, and a wrong result after a document update. Record the selected binary
path, server version from initialization, client version, filetype and whether
the CLI was reading the same saved bytes. Avoid changing a transcript merely to
silence a suspected tool defect.

The standalone `talkbank-lsp` executable speaks LSP over stdio; stdout is not a
log stream. Use the editor client's LSP log to inspect startup failures and
protocol traffic. That traffic can contain full document contents and paths,
so use synthetic/reference input and inspect any log before sharing it.

The library emits Rust `tracing` events, but the current standalone entrypoint
does not install a tracing subscriber. Setting `RUST_LOG` alone is therefore
not a supported promise of server-side debug output. An embedder must configure
its own subscriber, send logs to stderr or another sink, and keep stdout clean
when using stdio transport. Internal log labels are not a public protocol contract.

## A bounded manual editing drill

Use a copy of a canonical reference transcript or the tool-generated practice
file from the user tutorial. Keep the original. Check behavior after changes,
not only after opening the document:

| Edit or action | Observation |
| --- | --- |
| Remove and restore an ordinary utterance terminator | The missing-terminator diagnostic appears, then clears |
| Edit one utterance's words | Diagnostics and hover reflect the new buffer rather than stale content |
| Insert or delete an utterance with its dependent tiers | Positions and remaining tier relationships update coherently |
| Change participant or language headers | Context-sensitive diagnostics are recomputed for the changed facts |
| Edit around a non-BMP character or CRLF line ending | Highlights, edits and diagnostic ranges use the right positions |
| Request formatting or a quick fix | Review the returned edit and its scope before saving |
| Close the document and editor connection | Diagnostics clear as appropriate and the server exits normally |

The public contract is correct current results, not a particular internal
incremental/splice log path or an assertion that diagnostics never redraw.
A fallback may be correct. Investigate performance separately with the input,
operation and timing retained; do not prescribe a fast path before checking its
preconditions.

## Automated protocol contracts

The existing `position_conversion_tests` integration target includes position
conversion and real stdio lifecycle cases. Discover its actual test names before
selecting a focused regression:

```bash
cargo test -p talkbank-lsp --test position_conversion_tests -- --list
```

The lifecycle selection is:

```bash
cargo test -p talkbank-lsp --test position_conversion_tests stdio_lifecycle
```

The `stdio_lifecycle::completion` selection checks the completion transport
boundary with a reference transcript: initialization advertises the triggers,
the server returns an explicit postcode edit, and a later buffer revision moves
that edit's UTF-16 range. It applies the returned edit through an incremental
`didChange`, then checks current-version diagnostics and formatted server text.
The source uses CRLF and non-BMP text before the target. This complements the
per-feature handler tests; it does not certify every completion context or an
editor's suggestion UI.

The `stdio_lifecycle::diagnostics::code_actions` selection checks quick-fix
admission. Client diagnostics select matching current server diagnostics; stale
or foreign diagnostics cannot authorize edits. Fixes carry the document version
in `documentChanges`, so clients must advertise
`workspace.workspaceEdit.documentChanges`. Without that capability the server
does not advertise or return quick-fix edits; there is no unversioned fallback.
The tests include edits during the diagnostic debounce interval, close/reopen,
UTF-16/CRLF geometry, application of returned edits, and resulting diagnostics.
E305/E306 proposals come from the shared diagnosis-owned repair catalog: the
terminator is inserted before the typed line ending, and deleting an empty turn
with dependent tiers is withheld to preserve their ownership.
Separate native controls distinguish equal-text/equal-version reopened documents
by their open-document identity. These guarantees concern server admission and
wire version binding; actual rejection of a late edit is the client's job and
is not established by a transport harness alone.

The editor sends `shutdown`, waits for its successful response, then sends
`exit`. Successful shutdown permits exit code 0; `exit` without successful
shutdown requires code 1. The client may retain its stdin pipe until the child
exits: EOF is not required. See the
[LSP lifecycle specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#exit).

The server's lifecycle owner distinguishes those states. The standalone
entrypoint also avoids waiting for Tokio's uncancellable stdin read after the
protocol session ends; embedders own their runtime teardown. Existing process
tests use bounded waits and child cleanup. A library-only test cannot establish
that an actual editor child shuts down.

Add cases to the existing owner for the affected feature. Keep shared CHAT
semantics in the parser/model/transform crates, not an LSP-specific text parser
or alignment implementation. LSP coordinates are UTF-16 code units; use the
shared LSP conversion helpers and retain non-BMP and line-ending controls.
Use the [contributor workflow](setup.md) for focused feedback and final gates;
this page does not create a separate automatic commit gate.

## Evidence limits

A real stdio client can verify initialization, document updates, requests and
shutdown without a graphical editor. It does not prove UI keybindings, extension
installation, user preferences, signing or another operating system. Record the
actual client, server artifact, platform and observed interactions. A successful
source build is not a released-binary or independent-user drill.
