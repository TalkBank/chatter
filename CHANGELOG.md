# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Before 1.0, breaking changes to the CLI or library APIs bump the minor
version and are listed under "Changed" / "Removed".

## [Unreleased]

### Added

- New validation rule E752: timing bullets without an `@Media` header.
  A transcript carrying timing evidence (utterance bullets or `%wor`
  word timing) must declare the media those timestamps index; completes
  the media-consistency family (E544: declared linkage without timing;
  E552: declared `unlinked` contradicted by timing). Mirrors CLAN CHECK
  error 112.
- New validation rule E753: a word consisting only of a repetition
  segment (fully `↫...↫`-wrapped, no stem outside the delimiters) is
  rejected; word-category prefixes (`&-` filler, `&~` nonword, `0`
  omission) count as a stem. Adopted from GUI CLAN CHECK error 151 as
  a chatter-authority rule (the unix CHECK build never enforced it).
- New validation rule E754: the `@l` letter form must carry exactly one
  letter of stem (`b@l`); multi-letter content belongs under `@k` /
  `@ls`. Repeated-segment material (`↫b^↫b@l`) does not count toward
  the stem, matching real CLAN CHECK behavior. Mirrors CLAN CHECK
  error 76.
- New validation rule E755: a `[- CODE]` utterance-level language must
  be declared in `@Languages` (utterance-level presence is
  substantial). Mirrors CLAN CHECK error 152.

- Word-level explicit language codes (`word@s:CODE`) are now validated
  against the ISO 639-3 registry (E519), the same rule that guards
  `@Languages` and `@ID`; declaration in `@Languages` remains not
  required.

### Removed

- The E254 warning (word-level `@s:CODE` not listed in `@Languages`)
  is retired: an explicit word-level language code is self-contained
  and deliberately carries no declaration requirement. `@Languages`
  declares the transcript's substantial languages; a one-word
  insertion is not substantial presence. (This matches CLAN CHECK,
  which dropped its own `@s` declaration requirement in 2019.)

<!--
Deferred to a later release:
- Word-content validity: reject junk inside words (`|`, ideographic comma,
  mojibake, ...) per the curated word-segment allowlist. Pending adjudication.
- CHECK-parity endgame closes (48 illegal `|`, 76 single-letter `@l`) and the
  remaining per-rule decisions.
-->


## [0.3.5] - 2026-07-15

Emergency release restoring corpus-correct word parsing. Versions
0.3.3 and 0.3.4 have been YANKED (releases and tags removed).

### Fixed

- Reverted the whitespace-boundary overlap-custody grammar introduced
  in 0.3.3. Its GLR-arbitrated word readings fragmented words carrying
  four or more glued markers (for example multi-syllable-pause chains
  like `or^ga^ni^zi^ra`), causing spurious E252/E331/E600/E705
  validation errors across real corpora and, worse, a serialization
  mutation (a space inserted into such words on rewrite). Word parsing
  is restored to the 0.3.2 grammar, verified by an error-code
  differential and a roundtrip comparison against the 0.3.2 binary
  over a corpus sample: identical profiles.
- A regression test pins that multi-marker words parse as one word and
  validate cleanly.

### Retained from the yanked releases

- Typed `@u` phonetic word forms (UNIBET).
- The `build_chat` header emitters and @ID demographics fix.
- The shared English capitalization transform.
- The long-tier stack-overflow fix and its regression test.
- The SQLite cache concurrency-safety fix; CI runs under nextest.

## [0.3.4] - 2026-07-15 [YANKED]

### Added

- **`@u` phonetic forms are now typed phonetic content.** A `@u` word
  (a UNIBET/IPA phonetic transcription standing in a word slot, e.g.
  the spoken side of an aphasia `[: target]` replacement) now models
  its content as a dedicated `WordContent::Phonetic(WordPhonetic)`
  node instead of orthographic text, in both parsers. Orthographic
  word-hygiene rules structurally cannot apply to phonetic content;
  the phonetic string itself stays deliberately lenient (IPA, ASCII
  UNIBET, X-SAMPA), matching the `%pho` tier's stance. `to-json`
  emits `{"type": "phonetic", ...}` for these nodes (schema updated);
  `cleaned_text` remains the phonetic string verbatim; the sanitizer
  redacts phonetic forms like spoken text. Scope is `@u` only;
  sibling special forms remain orthographic words.

- **`build_chat` now emits the full standard header set.** The general
  CHAT-generation schema (`TranscriptDescription` / `ParticipantDesc`)
  gained typed optional fields for `@Date`, `@Situation`, `@Options`,
  `@Transcriber`, `@Comment`, per-speaker `@L1 of`, and `@PID`
  (preserved from a source, never minted), each emitted in canonical
  header order. `@ID` demographics (age, sex, group, SES, education,
  custom) are now carried through `ParticipantDesc` instead of being
  silently dropped, fixing empty demographic slots in generated `@ID`
  headers.
- **Shared English capitalization transform**
  (`talkbank_transform::capitalize`): capitalizes the pronoun "I"
  family and the first real word of each utterance on the typed model,
  for generators whose sources are all-lowercase (improves downstream
  `%mor` accuracy). Token-level helpers are public for generators that
  capitalize their own word representation.

### Fixed

- **`chatter validate` no longer headlines a warnings-only file as an
  error.** A file whose findings are all warnings (which is valid CHAT,
  and was already counted valid in the summary) now prints
  `⚠ Warnings in <file>` instead of the contradictory
  `✗ Errors found in <file>`, and the "fix structural errors first"
  hint fires only on hard errors. Presentation only; validation logic
  unchanged.
- **The validation cache no longer fails to initialize when opened
  concurrently.** Two `chatter` runs sharing a cache directory (or a
  multi-threaded consumer) could race the one-time SQLite setup and hit
  `UNIQUE constraint failed: _sqlx_migrations.version` or a WAL init
  collision, silently disabling caching for that run. Concurrent opens on a
  fresh cache directory now retry the transient init race and all succeed.

## [0.3.3] - 2026-07-13 [YANKED]

### Added

- **Desktop app: a "Check for Updates..." menu item and a periodic background
  update check.** The app previously checked for a new release only at launch,
  so an app that was rarely relaunched could sit far behind. It now also checks
  every six hours in the background, and the app menu has a manual "Check for
  Updates..." item that reports when you are already up to date.
- **Desktop app: a real "About Chatter" panel** with the version, a short
  description, and clickable links to the TalkBank site and the source
  repository, replacing the bare version-only default.
- **`talkbank_transform::build_chat`: assemble a validated CHAT file from a
  typed transcript description.** Given participants, optional media, and
  utterances as pre-formatted CHAT main-tier text (`TranscriptDescription`),
  it synthesizes the header block, parses each utterance through the
  tree-sitter parser, and returns a `ChatFile`. The description carries a
  `media_status`, so a transcript that names its media but has no timing
  bullets yet (pre-forced-alignment) can emit `@Media: <id>, audio, unlinked`
  and stay valid instead of falsely claiming linkage (E544).
- **`talkbank_transform::num_words::expand_number`: spell digit tokens as
  language-appropriate number words** (13 lookup-table languages, CJK, and
  English ordinals/decades), so generated CHAT satisfies E220 (numeric digits
  are not allowed in words for languages that do not permit them).

### Changed

- **Overlap custody now follows whitespace boundaries, with canonical overlap
  serialization.** Overlap markers bind to the token on the correct side of a
  whitespace boundary, and serialization emits a single canonical form.
- **tree-sitter updated to 0.26.11** across the workspace (CLI, grammar
  bindings, and the generated parser).

### Fixed

- **Long dependent-tier reconstruction is now linear-time.** A quadratic blowup
  on very long utterance tiers is eliminated; pathological inputs that
  previously stalled the parser now reconstruct in linear time.
- **Desktop app: the validation settings popover no longer opens hidden behind
  the results panel.** It was rendered below the panels in the stacking order;
  it now sits above them.
- **Desktop app: the "up to date" dialog now dismisses on the first OK.** A
  listener leak (an async menu subscription whose cleanup could run before it
  resolved) let duplicate listeners accumulate, so one menu click stacked
  several identical dialogs.

## [0.3.2] - 2026-07-10

### Added

- **`chatter rediarize`: repair speaker attribution from external
  diarization turns.** Takes a transcript whose utterance timing is
  trusted but whose speaker labels are not, plus a speaker-turns JSON
  file (`{"source": ..., "turns": [{"track", "start_ms", "end_ms"}]}`)
  from an external diarizer, and re-attributes each timed utterance to
  the dominant overlapping turn. Utterances with no turn coverage are
  flagged, never guessed. Reconciled `@ID` rows are inserted in the
  header block. `--summary-json` emits a machine-readable outcome
  summary (per-utterance reattributions and flag reasons) for
  downstream tooling.
- **Four validation rules for constructs that do not make sense**,
  each adjudicated against real CLAN CHECK behavior and the wild
  corpus: E748 leading-zero media-bullet times; E749 comma glued to
  the following word; E750 whitespace inside angle-group delimiters;
  E751 pause marker glued to a word.

### Fixed

- The re2c oracle lexer now tokenizes short-form parenthesized
  material the same way the canonical parser does (its catch-all
  previously swallowed a trailing delimiter), keeping the two
  independent parsers in cross-check agreement on the new spacing
  rules.

### Changed

- Rust toolchain pin bumped to 1.97.0 (CI workflow pins synced);
  workspace and spec lockfiles refreshed; desktop dependency bumps
  (jsonschema 0.47, TypeScript 7).
- Documentation: an architecture page on overlap-marker binding (why
  edge-adjacent overlap markers bind into words, the ideal top-level
  model, and the conversion-layer path); the grammar's empty-`extras`
  (all-whitespace-explicit) design rationale is now recorded at the
  declaration site.

## [0.3.1] - 2026-07-08

### Fixed

- **Every public fallible constructor's error type is now publicly
  nameable.** `LanguageCodeError` (from `LanguageCode::new`),
  `XphointParseError`, and `PhoalnParseError` were not re-exported, so
  downstream crates could not store them in typed `#[source]` fields and
  had to stringify at the boundary; found by the first real downstream
  consumption of the 0.3.0 API. A new API-surface guard test pins the
  contract so a constructor error type can never silently become
  unnameable again.

## [0.3.0] - 2026-07-07

### Added

- **`--llm-cache <file>` (env `CHATTER_LLM_CACHE`) for holistic speaker-id
  judgment.** A persistent, write-through JSON response cache for
  `speaker-id` / `pipeline` / `batch --judgment holistic`: an identical
  request (same endpoint, model, and rendered prompt) is served from the
  cache instead of making another LLM call, so re-running a batch after a
  crash or an unrelated code change does not re-pay completed sessions.
  Absent flag and env variable means uncached, unchanged from before.

### Fixed

- **`chatter batch` no longer reports holistic suggestions as merges.** In
  holistic-judgment mode the per-session pipeline exits 0 after writing a
  suggestion to the pending file without merging (the operator adjudicates
  first); the batch summary counted those as "merged" and reported zero
  pending work. Outcomes are now classified by whether the merged output
  actually exists, and the summary separately counts merges, suggestions
  awaiting adjudication, and low-confidence refusals awaiting adjudication.
- **E552 (`@Media` says `unlinked` but timing exists) now says where the
  timing was found and how to fix it.** When the only timing evidence is
  word-level bullets inside a `%wor` tier (invisible in normal display), the
  message names the `%wor` tier and offers both remedies (the media is in
  fact aligned: remove `unlinked`; or the `%wor` tier is stale: remove it)
  instead of asserting the media is linked and pointing at bullets the user
  cannot see. The main-tier-bullet case keeps its direct advice.
- **Chatter Desktop's single-file validation now shares the CLI's validation
  engine.** Previously, validating a single `.cha` file in the desktop app
  (as opposed to its parent folder) bypassed the on-disk cache entirely,
  skipped the `@Media`-filename check (E531), and could not honor
  `--roundtrip` / `--parser` / `--strict-linkers`. All of these now work
  identically to `chatter validate` and to the desktop's own folder
  validation, and a new **Settings** panel exposes the equivalent options.
- **Chatter Desktop no longer shows "N files, all valid" before a run has
  actually finished.** The file tree previously derived this message from
  the partial, still-streaming result set, so it could flash "all valid"
  mid-run whenever no error had streamed in yet.

## [0.2.1] - 2026-06-24

### Added

- **The `talkbank-lsp` language server now ships as a standalone release
  artifact.** Prebuilt, code-signed `talkbank-lsp` binaries for macOS (Apple
  Silicon and Intel), Linux (x86_64 and aarch64, static musl), and Windows are
  attached to the GitHub Release, each with its own `talkbank-lsp-installer.sh`
  / `talkbank-lsp-installer.ps1`. Any LSP-aware editor can now install the server
  without building it from source; it is a first-class artifact in its own right,
  not only the binary the VS Code extension bundles per platform.

## [0.2.0] - 2026-06-23

### Added

- **More of CLAN CHECK's invalidity is now enforced.** A batch of CHECK-parity
  rules was implemented so `chatter validate` rejects more invalid CHAT:
  - `E514`: an `@ID` line's corpus field is required (CHECK 63).
  - `E547`: a constant participant header must follow the `@ID` block.
  - `E548`: closes the case CHECK 126 covers.
  - `E549`: a speaker may not be declared twice (CHECK 13).
  - Duplicate `@ID` lines and out-of-order `@Options` fields (CHECK 13, 125).
  - A dependent tier used without being declared (CHECK 17).
  - An out-of-range `@Time Duration` (CHECK 35).
  - An `@Media` header marked unlinked while the transcript still carries timing
    bullets (CHECK 124), and an `@Media` filename that does not match the data
    file (CHECK 157).
  - A replacement `[: ...]` now requires a preceding space (CHECK 161).
  - Tree-sitter recovery nodes are surfaced as invalidity rather than silently
    repaired: a surviving `ERROR` node maps to `E316` and a `MISSING` node to
    `E342` (with the re2c oracle mirroring it), covering a group with no
    annotation and swallowed recovery nodes inside comma-list headers
    (CHECK 5/6/106/108).
- **Phon:** `U` (unknown) is accepted as a legal syllable-constituent code on the
  `%xmodsyl` and `%xphosyl` tiers.
- A formal behavioral CHECK-validity parity test suite that runs real CLAN CHECK
  and chatter on the same fixtures and fails if either side drifts.

### Changed

- **`chatter update` now self-updates in process.** It embeds the axoupdater
  self-updater as a library, reads the cargo-dist install receipt (keyed by the
  package name), and replaces the running binary from GitHub Releases. This
  removes the package-name coupling that previously made `chatter update` report
  "not installed" on a correctly installed binary.
- **The CLI package is renamed `talkbank-cli` to `chatter`** (the crate now lives
  at `crates/chatter/`). The generated install scripts are therefore
  `chatter-installer.sh` and `chatter-installer.ps1` (previously
  `talkbank-cli-installer.*`); update any pinned install URL accordingly. The
  binary is still `chatter`, and the library/API crates keep their `talkbank-*`
  names.
- **Validation is stricter.** Because of the new CHECK-parity rules above, some
  files that passed `chatter validate` under 0.1.1 may now report errors. This is
  intended: chatter is the CHAT-validity authority and is at least as strict as
  CLAN CHECK.

- Word-level explicit language codes (`word@s:CODE`) are now validated
  against the ISO 639-3 registry (E519), the same rule that guards
  `@Languages` and `@ID`; declaration in `@Languages` remains not
  required.

### Removed

- The standalone self-updater binary (cargo-dist `install-updater = false`). The
  `chatter update` subcommand is unchanged for users; it now updates in process
  instead of shelling out to a separate program.

### Fixed

- The recovery-node invalidity backstop is scoped to localized errors so it does
  not over-flag, and several malformed `@ID` test fixtures were corrected.
- Hardened the CHECK-parity audit and corrected a CHECK 126 verdict it had
  falsely certified; the curated CHECK error-code map is restored in place of a
  brittle keyword heuristic.

## [0.1.1] - 2026-06-22

### Fixed

- **Validation cache could serve a stale verdict across rule-set changes.**
  `chatter validate` keyed its result cache on the cache crate's package
  version, which does not change when validation rules change, so a "Valid"
  result cached before a new rule (such as a retrace-marker check) existed kept
  being served, while a fresh conversion of the same bytes correctly rejected
  them. The cache key now folds in a fingerprint over every error-code rule, so
  adding, removing, or renaming any rule invalidates stale entries; the cache
  is kept and still functions, only keyed correctly.
- CLI usage lines pin the binary name to `chatter` regardless of the invoked
  path (clap `bin_name`).
- The book renders Mermaid diagrams again (restored mdbook-mermaid assets).
- **Desktop app version is now locked to the release version.** The desktop
  bundle (`.dmg` / `.exe` / `.deb`) and the Tauri auto-updater manifest now report
  the same version as the CLI. A version-sync gate (`scripts/sync-app-version.py`,
  enforced in CI and at release time) keeps `tauri.conf.json`, `package.json`, the
  workspace version, and this changelog from drifting, so the updater can never
  again advertise a version the installed bundle does not match.

### Changed

- CI book toolchain bumped to mdBook 0.5.3 and mdbook-mermaid 0.17.0.
- Build: force `serialize-javascript >= 7.0.5` to clear advisories, and bump
  `rand` in the spec crate.
- Docs: the book intro is de-staged for the public release (download-first).

## [0.1.0] - 2026-06-15

First public release.

### Added

- **CHAT-format core.** A strict, incremental tree-sitter parser
  (`talkbank-parser`) with an independent re2c oracle parser
  (`talkbank-parser-re2c`) that cross-checks it on every file; a typed
  CHAT data model with structured validation, error codes, and tier
  alignment (`talkbank-model`); and CHAT-to-JSON / JSON-to-CHAT / XML
  conversion, normalization, transcript-merge, and redaction pipelines
  (`talkbank-transform`).
- **Phon extension tiers.** The four Phon `%x` dependent tiers
  (`%xmodsyl`, `%xphosyl`, `%xphoaln`, `%xphoint`) are parsed and
  validated as first-class CHAT tiers, on by default (pass
  `--suppress xphon` to opt out): syllabification constituent codes and
  phone-vs-source reconstruction, model-to-actual phone alignment, and
  per-phone time intervals, with dedicated error codes.
- **`chatter` CLI.** `validate`, `normalize`, `to-json` / `from-json` /
  `to-xml`, `merge`, `speaker-id`, `batch`, `pipeline`, `adjudicate`,
  `sanity-scan`, `lint`, `clean`, `watch`, `new-file`, `show-alignment`,
  `validate-utseg`, `schema`, `update`, and a content cache.
- **Language server** (`talkbank-lsp`): real-time validation, hover,
  go-to-definition, and cross-tier alignment for any LSP-aware editor.
- **Desktop app** (`Chatter`): a Tauri-based CHAT validation app, shipping
  in the coordinated release alongside the CLI.
- **Auto-update.** The `chatter` CLI self-updates with `chatter update`
  (the bundled cargo-dist / axoupdater self-updater), and the desktop app
  checks for and installs new releases on launch (Tauri updater). Both pull
  from GitHub Releases. The CLI self-updater is experimental.
- **Prebuilt binaries** for macOS (Apple Silicon and Intel), Linux, and
  Windows, plus desktop installers, attached to the GitHub Release. The
  macOS desktop `.dmg` is signed and notarized.

### Known limitations

- **The merge and adjudication surface is experimental.** `merge`,
  `adjudicate`, `speaker-id`, and `sanity-scan` work, but their
  interfaces and heuristics may change before 1.0.
- **Windows binaries are not code-signed yet**, so Windows SmartScreen
  warns on first run (choose "More info" then "Run anyway"). macOS CLI
  binaries are codesigned but not notarized; install via the release
  installer script to avoid the Gatekeeper quarantine prompt.
- **Not on crates.io yet.** crates.io publication is deferred.

[Unreleased]: https://github.com/TalkBank/chatter/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/TalkBank/chatter/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/TalkBank/chatter/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/TalkBank/chatter/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/TalkBank/chatter/releases/tag/v0.1.0
