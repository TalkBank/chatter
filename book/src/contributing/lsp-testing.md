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

The `document_links` selection checks model-owned `@Media` references. Its
real stdio case preserves a quoted URL containing commas and a non-BMP
character, then changes the unsaved buffer to a local filename and removes or
malforms the header. It checks the exact UTF-16 filename range on LF and CRLF
sources. A local-file control checks extension resolution. Rejected media
headers do not acquire links from a second text parser.

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
editor's suggestion UI. A second journey types unfinished speaker and dependent
tier prefixes, applies the returned prefix edits, and completes valid content
through incremental changes in LF, CRLF and CR documents. Unknown names,
indented continuations and prefix-looking content must not receive draft edits.

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
The `stdio_lifecycle::diagnostics::rename` selection checks the same capability
requirement for speaker rename, including direct requests from unsupported
clients. It checks revision numbers after buffer changes and reopen, exact token
edits with CRLF/non-BMP text, preservation of prose and other speakers, and the
diagnostics after applying the returned edits. The same selection checks invalid replacement and collision refusals, explicit
no-op requests, syntax-recovery refusal, and rename with missing metadata.
Shared transform admission owns replacement policy; feature controls cover
speaker-specific headers and other-speaker speech events.
The personal-header protocol case initiates rename from `@Birth of`,
`@Birthplace of` and `@L1 of` in an unsaved revision, with LF and CRLF.
It checks prepare-rename ranges, versioned edits, exact preservation of header
payloads and identity-like comment text, and diagnostics after applying edits.

Separate native controls distinguish equal-text/equal-version reopened documents
by their open-document identity. These guarantees concern server admission and
wire version binding; actual rejection of a late edit is the client's job and
is not established by a transport harness alone.

An invalid change batch puts the document session into an explicit
resynchronization-required state. That state retains the last observed version,
but exposes no source to features. The server clears published diagnostics,
logs a request for full text or reopen, and returns `ContentModified` for pull
diagnostics. Saves and later incremental changes cannot restore synchronization.
A newer full replacement (optionally followed by ranged edits in the same
batch), or close/reopen, restores normal operation. The reversed-range stdio
control checks these transitions, stale-version refusal, document and workspace
symbol withholding, and fresh diagnostics after recovery. Native controls also
ensure that retained analysis snapshots cannot pass current-revision admission
while the session awaits replacement.

The `stdio_lifecycle::diagnostics::recovery` selection checks PHO/MOD/SIN
recovery through actual open, change, pull/push diagnostics, repair and reopen
cycles with LF and CRLF. Formatting retains raw dependent tiers when a rewrite
is admitted. A separate unrepresented-suffix control requires formatting to
refuse an edit that would drop source content; repairing the buffer restores
normal formatting. The handler consumes the transform crate's `Rewrite` proof,
which checks ordered source-line content ignoring whitespace. This is a loss
guard, not semantic-validity or byte-identity certification.

Formatting also retains the producing open-document identity until response
projection. If that revision changes, closes, reopens (even with equal source
and version), or loses synchronization, the response is `ContentModified`.
Native admission controls exercise each transition between preparation and
projection; the stdio resynchronization journey checks formatting refusal and
recovery after reopen. Formatting reuses the document-owned admission used by
quick fixes and rename, without requesting diagnostic validation just to format.
Unlike a versioned workspace edit, a formatting response contains plain text
edits. This server-side check does not prove that a graphical client discards
an edit which arrives after another local change.

Completion, on-type formatting and linked-editing ranges use the same private
revision-bound response owner. Their producers read the captured revision and
cannot return their response after its identity changes. The resynchronization
stdio journey requires `ContentModified` from all four edit-related endpoints,
then verifies formatting and linked editing after reopen. Existing completion
and on-type journeys retain their positive edit/application controls. This does
not add wire versions to these protocol responses or qualify client-side races.

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

The position helpers and line-based feature readers share a source-bound
protocol-line representation. LF, CRLF and lone CR are line endings; CRLF is
atomic and a trailing ending starts an empty final line, as required by the
[LSP text-document contract](https://github.com/microsoft/language-server-protocol/blob/gh-pages/_specifications/lsp/3.17/types/textDocuments.md).
Mixed-ending boundary controls check UTF-16 columns, non-BMP characters and
clamping. The real-stdio journey compares speaker symbol ranges and semantic
tokens across all three transports, then checks a ranged emoji insertion
without replacing the document. This does not normalize the caller's bytes or
promise identical source offsets across different transports.

Workspace symbols come from the current buffer's parsed headers and utterances.
The source-bound analysis supplies both structure and navigation ranges; a line
starting with `@` or `*` is not sufficient by itself. The real-stdio control
replaces a reference utterance with an unparsed draft, checks that the draft
is absent while healthy regions remain navigable, restores the utterance, and
closes the document. It runs across LF, CRLF and CR. Retained recovery structures
can remain navigable; a symbol is not a whole-document validity certificate.

Document outline, folding and code lenses also consume source-bound analysis.
Gem validation and navigation share the model's label-scoped `GemState`:
different labels can overlap, and an unmatched end cannot close another label.
Only located pairs project to editor ranges. Folding includes the full closing
header, including continuation lines. The stdio controls exercise bare,
mismatched, multiline and overlapping gems through unsaved buffer replacements
in all three line endings, alongside current speaker counts and their parsed
participant-header anchors. These are navigation controls, not a claim that a
paired region or displayed count certifies the entire file.

All advertised `workspace/executeCommand` operations keep failures separate from
successful payloads. Malformed arguments or an unknown command name produce a
JSON-RPC error with code `-32602`; an admitted command that fails during execution
uses `-32001`. Errors have no `result` field. Clients must handle the error before
using a result as transcript text, an ID line or structured data; the old pre-1.0
successful error strings are no longer returned. Feature services retain typed
domain errors until the single execute-command wire boundary.
The real-stdio controls enumerate advertised commands, reject missing arguments,
exercise unavailable documents and invalid ID fields, then verify successful
payloads after opening the reference document. Closing the document restores
unavailability rather than returning replacement text containing an error.

The `talkbank/scopedFind` command returns each match as `range`, `tier`,
`speaker` and `line_text`. `range` is an LSP range with UTF-16 `start` and `end`
positions; clients must use both endpoints, including their line numbers.
`line_text` is the original start-line preview, not a reconstruction of a
multiline match. This replaces the pre-1.0 `line`/`character`/`length` fields.
Matching retains original-source spans; it never projects offsets from a
lowercased copy. Plain queries are escaped literals with Unicode simple case
folding (the regex library's case-insensitive policy), including overlapping
matches. An empty plain query returns no matches. Explicit regex queries keep
normal non-overlapping regex iteration and zero-width matching behavior.
Invalid regex input or a source span that cannot select UTF-8 text refuses the
request with a JSON-RPC error. A match whose endpoint splits a CRLF pair or
exceeds the protocol coordinate space also refuses the request: rounding would
select different bytes. Use complete line endings in such queries. Representable
zero-width matches remain valid empty ranges.
The real-stdio regression covers Unicode search, source-line previews and
multiline literal/regex matches across LF, CRLF and CR. It applies the returned
range as an incremental edit, verifies the joined line and preserved suffix,
then searches the updated buffer. This is server/protocol evidence, not a
qualification of a graphical client's search picker.

Header quick fixes for missing `@UTF8` and `@End` consume the shared transform
catalog. Insertions use the first observed LF, CRLF or CR ending; a document
without an ending uses LF. Existing mixed endings remain untouched. An `@End`
repair inserts a separator only when nonempty source has an unterminated final
line. The real stdio control applies each versioned edit, checks current
post-edit diagnostics and refuses the stale action after repair. Catalog header
proposals do not change the CLI splice engine's utterance-only admission policy.

Speaker references honor `includeDeclaration`: `@Participants` entries are
included only when requested, while `@ID` metadata and other parsed speaker
uses remain references. Go-to-definition consumes the same occurrence
classification and reaches the participant entry through its contents wrapper.
The real stdio journey checks exact ranges under LF, CRLF and CR, declaration-only
buffers, unsaved removal/restoration of uses, and close. Speaker text inside a
comment is not a reference. Navigation through a recovered document does not
certify that document's validity.

Alignment inlay hints use each annotation's own tier-end position, before the
physical line ending. The MOR mismatch label belongs to the main tier; the GRA
mismatch label belongs to `%gra`, even when the main tier is outside the visible
range. The full UTF-16 position must lie in the requested half-open range: its
start is included and its end excluded. Unlocated or inexact source positions
produce no hint. Counts still come from the shared model's alignment results.
The real-stdio journey checks both mismatch kinds, GRA-only views, column-sized
ranges, Unicode, LF/CRLF/CR, repair/restoration of unsaved text and document close.
This is protocol evidence, not a graphical client's rendering qualification.

## Evidence limits

A real stdio client can verify initialization, document updates, requests and
shutdown without a graphical editor. It does not prove UI keybindings, extension
installation, user preferences, signing or another operating system. Record the
actual client, server artifact, platform and observed interactions. A successful
source build is not a released-binary or independent-user drill.

## Alignment snippet presentation

Main-tier hover and sidecar snippets use the shared model CHAT renderer.
Annotations, group delimiters and word markers remain visible. Sidecars carry
plain CHAT strings; the hover adapter escapes punctuation for Markdown. The
stdio alignment-snippet test checks both surfaces before and after an editor
change. Rendering is not a claim that recovered content is valid.

MOR items and GRA chunks are distinct index spaces. Hovering a main-tier word
or MOR item uses the shared item-start projection followed by the computed
GRA alignment pair. Reference clitic tests check both directions into GRA and
withhold the relation when alignment metadata is absent. The stdio check
requires the word after a cliticized item to display its own relation.
