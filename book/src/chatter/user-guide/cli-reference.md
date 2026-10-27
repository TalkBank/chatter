# CLI Reference

**Status:** Current
**Last modified:** {{git-dates:page}}

Run `chatter` for the task overview, or append `--help` to any command.
Every namespace also shows its help when invoked without a subcommand.
Everyday tasks are at the top level; specialist speaker and review workflows
are under `experimental`.

```bash
chatter validate transcript.cha
chatter fix transcript.cha                  # preview available catalog repairs
chatter format transcript.cha --output formatted.cha
chatter convert to-json transcript.cha
chatter inspect words transcript.cha
chatter checks explain E241
chatter commands list --format json
```

## Discovery

`chatter commands list` lists every executable leaf. Use
`chatter commands show convert to-json` for a command's full help and effects.
Both support `--format json`. The JSON document has schema identifier
`chatter.commands.v1` and a `commands` array. Each entry includes:

- `path`: the command words after `chatter`;
- `about` and `help`: the executable's descriptions;
- `semantics`: maturity, effects, stdout product, prerequisites and completion;
- `arguments`: clap-owned names, position, requiredness, arity, values,
  defaults, conflicts and help.

Argument groups and conditional constraints also appear in full `help`.
Machine discovery needs no transcript, cache, provider or credential. It
performs no update or model request. Wait for successful process exit before
accepting a complete result. A partial stdout stream is not a success receipt.

Separate regular-file artifacts are staged and flushed before replacement.
Write failures preserve an existing destination; replacement changes only the
selected name, leaving other hard links untouched. Existing symlinks select
their regular target, while dangling symlinks are refused. Source aliases are
still rejected. This protects bytes and ordinary permissions; in-place
transforms have their own stricter metadata-preservation policy. Publication
is per file, not a transaction across a batch or a pair of outputs. Streams
and devices cannot be rolled back. Directory conversion still removes stale
JSON when its source fails conversion, as documented below.

`chatter checks list` lists all diagnostic codes and their Active, Opt-in or
Planned enforcement status. `chatter checks explain CODE` identifies a code's
status and authoritative documentation. These commands do not validate files.

## Validation

```bash
chatter validate PATH...
chatter validate corpus/ --format json
chatter validate transcript.cha --refresh-cache
chatter validate corpus/ --audit diagnostics.jsonl
chatter watch transcript.cha
```

Validation walks directories recursively. Its default policy includes
supported dependent-tier alignment and Phon `%x` checks. `--skip-alignment`
selects structural checks without dependent-tier alignment. `--strict-linkers`
enables the opt-in cross-utterance linker checks. `--suppress CODE[,CODE...]`
or `--suppress xphon` explicitly suppresses diagnostics; suppressed diagnostics
do not cause an invalid-input exit. A successful suppressed run certifies only
the selected policy.

`--parser tree-sitter|re2c` selects the parser backend. Both use the shared
validation policy. `--roundtrip` adds a serializer round-trip check.
`--jobs N` and `--max-errors N` require positive integers. A stopped run reports
incomplete validation and fails; in-flight files may finish after the limit.

`--refresh-cache` clears the selected inputs' cached results and validates
again. It is independent of JSON export's `--rebuild`. `--audit FILE` writes
JSONL diagnostics and only reads existing cache state. It conflicts with
`--format`, `--quiet` and `--refresh-cache`. Audit output may remain partial
if writing fails; always preserve the exit status.

Validation `--format json` currently emits **JSON Lines**, one typed record
per line, including completion. It is not transcript-model JSON. See the
[diagnostic contract](../integrating/diagnostic-contract.md).
`--quiet` suppresses successful text output. Terminal presentation belongs to
validation: `--tui-mode auto|force|disable` and `--theme PRESET` are not global
options. JSON output cannot force a TUI.

`watch PATH` continuously validates changes, with its own `--parser`,
`--skip-alignment` and `--clear` options. It is a long-running stream rather
than a finite validation receipt.

## Repairs

```bash
chatter fix PATH...
chatter fix transcript.cha --apply
chatter fix transcript.cha --code E305 --apply
```

Bare `fix` reports proposed catalog repairs and leaves inputs unchanged.
`--apply` selects mechanical repairs. A semantic repair additionally requires
its explicit `--code`; ambiguous alternatives are never chosen automatically.
Unknown diagnostic codes are usage errors. `--skip-alignment` chooses the
validation coverage used around the repair.

This is a finite repair catalog. It does not invent participants, languages,
roles or other missing facts, nor does it promise to make any transcript valid.
Review the proposals and revalidate the result. An unoffered or refused repair
is not proof that the input is valid. There is no `--dry-run` or generic
`--force` repair authority.

Changed repairs and in-place experimental transforms stage and synchronize
their complete output before replacing a file. Publication checks that the
source identity and bytes still match the input used to prepare the edit.
Read-only files, symlinks and multiply-linked targets are refused. One refused
file does not prevent processing other inputs, but the run exits unsuccessfully.
This is per-file replacement, not a transaction across the selected files or
a lock against concurrent writers.

Existing-file replacement uses the platform's metadata policy, also for
`format --output` when the output already exists:

- macOS preserves permissions, ownership, ACLs, extended attributes and timestamps,
  including creation time.
- Linux preserves permissions, ownership, access/modification times and observable
  extended attributes, including POSIX ACLs. Inode identity, change time and birth
  time are new. Filesystem policy flags that cannot be preserved are refused.
- Windows stages an OS copy to retain named streams and extended attributes,
  copies and verifies owner/group/DACL security, and retains ordinary attributes
  and creation/access/modification times. Reparse points, encrypted, compressed,
  sparse and other specialist file states are refused.

An unavailable metadata operation or insufficient preservation privileges fails
before replacement. Other operating systems refuse existing-file replacement;
preview and new output remain available. These checks concern metadata observable
to the invoking account; they do not certify hidden privileged metadata or all
remote filesystems. Publication synchronizes file contents, but does not promise
directory crash durability.

## Formatting and new files

```bash
chatter format INPUT --output formatted.cha
chatter format INPUT --validate
chatter new --output new.cha
chatter new --speaker CHI --language eng --role Target_Child --corpus example
```

`format` emits canonical CHAT to stdout or `--output`. Its parsing/content
preservation admission is not full validation. `--validate` adds validation;
`--skip-alignment` only applies with that validation request. It refuses
source loss rather than silently serializing a recovered partial transcript.

`new` emits a minimal template to stdout or `--output`. Defaults are template
parameters: speaker `CHI`, language `eng`, role `Target_Child`, corpus `corpus`.
`--utterance TEXT` adds initial content. These defaults are not inferred facts
about an existing recording.

## JSON conversion

```bash
chatter convert to-json INPUT
chatter convert to-json INPUT --output transcript.json
chatter convert to-json corpus/ --output-dir json/
chatter convert to-json corpus/ --output-dir json/ --rebuild
chatter convert from-json INPUT --output transcript.cha
chatter schema
chatter schema --url
```

Single-file `to-json` emits a model JSON artifact to stdout or `--output`.
Directory mode requires `--output-dir`, preserves relative structure, and
skips up-to-date outputs. `--rebuild` requests regeneration; `--prune` separately
removes orphan JSON outputs. `--jobs N` belongs to directory mode. A file-mode
request with directory-only options fails instead of ignoring them.

`--compact` chooses compact JSON. CHAT validation and schema validation are
on by default. `--skip-validation` skips CHAT checks; `--skip-alignment`
selects structural CHAT checks and conflicts with `--skip-validation`.
`--skip-schema-validation` is a separate policy. Conversion success with
checks skipped does not establish full validity. Invalid input and partial
batch failures produce nonzero exits.

`from-json` imports model JSON and emits CHAT. `schema` emits the current
schema; `--url` emits only its canonical URL. Artifact stdout remains raw:
logging and diagnostics must be captured separately on stderr. See
[JSON output](../integrating/json-output.md) and
[JSON schema](../integrating/json-schema.md).

## Inspection

```bash
chatter inspect alignment INPUT --tier mor
chatter inspect words INPUT --diff-only --format json
chatter inspect overlaps PATH...
chatter inspect linkers PATH...
```

`alignment` displays dependent-tier alignment, optionally selecting
`mor|gra|pho|sin`; `--compact` changes its layout. `words` displays raw and
cleaned word projections, with `--diff-only` and `--format text|json`.
Neither command changes the transcript.

`overlaps` and `linkers` are exploratory reports, not validity certification.
Their optional `--database PATH` and `--anomalies PATH` destinations are new
JSONL report files, respectively; existing destinations are refused. A failed
report can retain partial output. Review any suggested changes independently.

## Cache

```bash
chatter cache stats --format json
chatter cache clear --all                   # preview
chatter cache clear --prefix corpus/        # preview a selected subtree
chatter cache clear --prefix corpus/ --apply
```

The validation cache stores per-file results under the platform cache directory.
`stats` supports text and JSON. `clear` requires exactly one scope: `--all` or
`--prefix PATH`. It counts without removing by default; `--apply` authorizes
removal. A preview does not create or migrate the cache. Cache results are tied
to validation identity and policy, not merely the visible version number.

## Experimental workflows

Experimental commands can change as their workflows mature. Every command is
under an explicit namespace, with a separate input shape for each mode.

| Command | Purpose |
| --- | --- |
| `chatter experimental speakers map` | Explicit `--mapping SPEC`; emits relabeled CHAT |
| `chatter experimental speakers match` | Requires `--reference`, `--anchor`, and `--inserted-role`; may refuse insufficient confidence |
| `chatter experimental speakers replay` | Requires `--override-file` and `--session-id`; replays a recorded decision |
| `chatter experimental speakers suggest` | Requests an LLM suggestion; requires `--write-pending`, never emits a relabeled transcript |
| `chatter experimental speakers rediarize` | Reattributes timed utterances using required `--turns` input |
| `chatter experimental speakers scan` | Heuristic QA with a required pending-decision destination |
| `chatter experimental review scripted` | Uses a required `--script` and `--operator` to resolve pending decisions |
| `chatter experimental review interactive` | Uses terminal input and a required `--operator` to resolve pending decisions |
| `chatter experimental transform language-runs` | Previews whole-utterance language-marker rewrites |
| `chatter experimental transform retag-language` | Previews explicit `--from` / `--to` substitutions |
| `chatter experimental transform join-retrace` | Previews joins under `--scope repetition|corrections|all` |
| `chatter experimental transform redact` | Emits structurally preserved redacted CHAT; no anonymity guarantee |
| `chatter experimental update` | Contacts GitHub Releases and replaces a supported installer-managed executable |

In-place transforms require `--apply` to publish. Join previews also report
loss of dependent tiers that need regeneration. Broad joining scopes are
semantic decisions, not automatic general repair. Review commands require an
explicit decision source and override destination; interactive review refuses
pipes instead of guessing whether to prompt. Provider options exist only on
`speakers suggest`, so mapping/replay cannot silently ignore LLM options.

Detailed guides: [Speaker ID](speaker-id.md), [Rediarize](rediarize.md),
[Review tools](merge-workflow.md), and [Redaction](sanitize.md).

## Migration

This command overhaul makes a clean break: old spellings are not aliases.

| Previous spelling | Canonical replacement |
| --- | --- |
| `normalize` | `format` |
| `to-json`, `from-json` | `convert to-json`, `convert from-json` |
| `show-alignment`, `clean` | `inspect alignment`, `inspect words` |
| `new-file` | `new` |
| `validate --list-checks` | `checks list` |
| `validate --force` | `validate --refresh-cache` |
| `to-json --force` | `convert to-json --rebuild` |
| `speaker-id` mode flags | Separate `experimental speakers map|match|replay|suggest` commands |
| `adjudicate` mode flags | Separate `experimental review scripted|interactive` commands |
| `rediarize`, `sanity-scan` | `experimental speakers rediarize|scan` |
| `debug overlap-audit`, `debug linker-audit` | `inspect overlaps|linkers` |
| `debug fix-s`, `debug retag-language`, `debug join-retrace` | `experimental transform language-runs|retag-language|join-retrace` |
| `debug sanitize` | `experimental transform redact` |
| `update` | `experimental update` |
| Cache/transform `--dry-run` | Omit `--apply` for preview; add it only to publish |
| `--check-xphon` | Removed no-op; checks already run by default |

## Exit codes and output

| Exit | Meaning |
| --- | --- |
| 0 | Command completed successfully under its selected policy |
| 1 | Invalid or unreadable input, output failure, or incomplete processing |
| 2 | Usage error or unsatisfied precondition |
| 3 | Internal startup or encoding failure where classified |
| 4 | Speaker reference decision needs review due to insufficient confidence |
| 130 | Forced validation interruption |

A failed stdout write, including a closed pipe, returns 1 and stops further
command work. JSON validation keeps stderr empty on output failure. Unpublished
regular audit stages are discarded on error or interruption; earlier published
files are retained. During validation, the first Ctrl-C requests cancellation;
the second requests exit 130 through the event loop. Terminal restoration runs
before returning, including after TUI setup or drawing failures.

Do not infer success from an output file, a JSON prefix or a report alone.
Preserve process exit and stderr. No command's preview is later-process
publication authority, and no inspection output certifies transcript validity.
Global `-v`/`--verbose` and `--log-format text|json` configure logging, not the
command's result encoding.
