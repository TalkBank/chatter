# AGENTS.md

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/talkbank-parser-re2c/AGENTS.md)

Read the [root guidance](../../AGENTS.md) first. This file adds re2c-specific
ownership and compatibility requirements, not a separate verification policy.
Tree-sitter and re2c are peer backends. The shared model can evolve for either
parser; Tree-sitter's earlier prototype does not give it model ownership.
Follow the shared model's
[no-defaults/no-sentinels principle](../../book/src/architecture/chat-model/chat-model.md#no-silent-defaults-or-sentinels):
explicit recovery or rejection, never fabricated successful content. Silent
placeholders cause spurious diagnostic cascades and corrupt downstream data.

**Mandatory first read for parity, recovery or diagnostic work:**
[Backend compatibility mandate](../../book/src/architecture/parser-backends.md#backend-compatibility-mandate).
That book section is the single policy authority for architecture-native
implementation and user-visible backend differences. Do not revive earlier
requirements for byte-identical recovered ASTs or matching diagnostics.
Tree-sitter's generated CST traversal requirement is specific to that backend;
it does not prescribe re2c's lexer/parser architecture.

## What This Is

A CHAT transcript parser using **re2rust** (re2c's Rust backend) for lexing and **chumsky** parser combinators for parsing. Lives in the chatter workspace as `talkbank-parser-re2c`.

**Status:** Implements `ChatParser` from `talkbank-model` through `Re2cParser`, selected by `chatter validate --parser re2c`. Diagnostic and recovery parity remain incomplete. Current measurements, their limits, and reproducible commands belong in `docs/parity-report.md`; do not infer correctness or current speed from historical corpus results.

## Architecture

```
re2c DFA Lexer  -->  Chumsky Combinators  -->  talkbank-model
  (lexer.re)          (parser/*.rs)          (convert/*.rs)
```

**Pipeline stages:**
1. **Lexer** (`lexer.re`), re2c DFA produces rich tokens with tagged field extraction.
2. **Parser** (`parser/`), chumsky combinators consume `&[Token]`. MOR parsing
   constructs shared words, items and tiers directly. Participant lists construct
   shared `ParticipantEntries`, including explicit missing-role recovery, with no
   native participant AST or conversion. ID and media bodies also construct shared
   headers directly. Text-tier parsing retains a token view for direct shared-model construction; other
   families still use native intermediate types in `ast.rs`.
3. **Conversion** (`convert/`), conversion functions build talkbank-model
   values from admitted input, each through its model constructor with its `Location` as the
   first argument. Source-aware conversions use `SourceText` to place original
   input slices; see "Locations" below for what is and is not located.

### Parser Module Structure

```
src/parser/
  mod.rs              Module declarations, lex_to_tokens helper
  main_tier.rs        Chumsky: contents, words, groups, tier_body, main_tier
  dependent_tiers.rs  Chumsky: %mor, %gra, %pho, %sin, %wor, text tiers
  classify.rs         Token classification: is_terminator, is_annotation, etc.
  word_body.rs        Char-level word body scanner (not chumsky)
  file.rs             Imperative file-level parser with error reporting
  entry_points.rs     Public API: parse_chat_file, parse_main_tier, etc.
  headers.rs          Chumsky: @ID, @Languages, @Participants
```

### Key Design Decisions

**Chumsky** (pinned in `Cargo.toml`). Token-stream input via `&[Token<'a>]`. The `select!` macro matches token variants by value. `recursive()` handles nested groups/quotations.

**Borrowed source, temporary tokens.** Parser signatures separate `'tokens` from `'source`. The lexer borrows the caller's input and distinguishes end-of-buffer from NUL without padding or unchecked reads. Local token and recovery vectors are dropped after parsing. `WordWithAnnotations::raw_text` is a slice of the source from its rich token. Never leak allocations to extend a lifetime.

**Imperative file parser.** The file-level parser (`file.rs`) uses an imperative loop for line ownership and recovery. Exact lexer rules emit `DependentPrefixToken` with a `DependentBodyKind`; the dependent-tier parser exhaustively dispatches that admitted kind. Never reclassify labels with prefix-string tests: `%modsyl` and `%mod` select different body grammars.

**CA marker categories.** CA arrows and TCU markers remain separators, including
at the end of an utterance; linker forms remain linkers. No conversion pass
promotes them to terminators. See
[CA Terminator Resolution](../../book/src/architecture/parser-and-grammar/ca-terminator-resolution.md).

**Word admission.** Rich `Token::Word` values are the only lexical word production. Marker-only subtokens reject. Standalone zero actions have a separate source-bound production and never own lexical suffix fields.

## Locations

Every model constructor takes its `Location` first, and this backend must say
honestly which it has:

- **Located:** tiers, headers and main-tier words (`SourceText::span_of` on a
  borrowed `raw_text`). Also located: every item whose token keeps
  its WHOLE lexeme (`emit!` rules: terminators, linkers, overlap and
  underline markers, `%wor` separators), through
  `convert::locate(source, slice)`, and every tier value, whose extent the
  line machine owns (`NonEmptyRegion::extent`, `NonEmptyRegion::main_tier_locations`;
  `LocatedMainTier`, `DependentTierEntryParsed::extent`). A line includes its
  ending and main-tier content starts after the separator's colon, the same
  convention as the tree-sitter grammar; it is written on
  `MainTierLocations` and nowhere else. Word-body markers (`WordBodyItem`)
  carry their lexeme from the character scanner, and the lengthening run is a `LengtheningRun` that
  owns the nonempty proof once. To locate a new item kind, make its token
  retain its whole lexeme and admit source ownership before lowering; never
  recompute an offset from payload lengths or from a neighbouring token.
  Actions and events retain that admission in `ZeroAction` and `ParsedEvent`;
  conversion consumes their spans without another source lookup.
- **Located through `Extracted`:** a tag-extracting rule (`emit_t1!`,
  `emit_t1t2!`) emits `token::Extracted { payload, lexeme }`. `text()` is
  the payload (`eng` of `[- eng]`, `laughs` of `&=laughs`) and is what
  consumers read; `lexeme()` is the item as written and is the ONLY slice to
  admit for placement. A payload slice locates the payload, not the item.
  Events retain this admission in `ParsedEvent`; freecodes, long features,
  nonvocals, the precode, the interposed-speaker prefix,
  bullets (`MediaBulletToken::text()`) and the
  whitespace token directly before the `@Media` comma are placed this way.
- **Located as hulls:** groups and quotations from their `Enclosure`
  (opening through closing delimiter), a retrace from its content (the
  enclosure it took, or the one item it wraps) through its `marker`, a
  replaced word from the word through its last annotation, and every
  `Annotated` wrapper from its construct through its last annotation
  (`ScopedAnnotationParsed::span`). The helpers live in `convert/items.rs`
  (`retrace_span`, `chain_span`, `annotations_hull`); admitted chain leaves,
  enclosures, replacements and retrace markers retain their source spans, so
  these hulls are total and do not repeat source admission. The
  model's E757 rule depends on these wrappers being placed; do not reintroduce
  a parser-side mirror of it.
- **Unlocated:** nothing this backend builds from source, by design. Every
  word's `raw_text` is a slice of the source (the rich token's, or the
  covering slice an assembled word borrows through the `SourceText` the word
  parsers take), so the only `Unlocated` arms left are the fallbacks of
  `located` and the separator slice, reached only when a slice is not from
  the source being converted (a programming error). Never write a
  placeholder span (`0..0` was the old sentinel and is gone from the
  language); write `talkbank_model::Location::Unlocated`.
- Word/event character classes and word-attached marker sets come from
  `src/generated_symbol_sets.re`, emitted by `just symbols-gen` from the
  canonical symbol registry. Rich word rules own text admission; do not add
  a fallback word token with a broader character class.
- Changing `lexer.re` means regenerating `src/generated/lexer.rs` with the
  invocation in `build.rs` under the pinned re2rust (`re2c-version.toml`) in
  the same commit; `just verify-vendored-lexer` byte-compares them.
- A fragment parsed from empty input is located at the caller's offset with
  zero width.

The fabricated-AST gate in `talkbank-parser-tests` holds this crate's
production `Location::Unlocated` count at a measured ceiling
(`PARSER_UNLOCATED_CEILING`). It only falls: when the lexer learns an offset
for an item kind, lower the ceiling in the same commit. The count is textual
over production text (the file up to its first `#[cfg(test)]`), so a
consumer's `Location::Unlocated =>` match arm counts too; spell a
located-or-not read as `location.span().map(..)` in production code. A
diagnostic with nothing to point at is `DiagnosticLocation::Unlocated`, which
the gate does not count.

Validation rules fire on a construct's presence, never on its having a span
(E752 and E767 once went silent under this backend for exactly that reason),
so an unlocated item still triggers every rule; only the diagnostic's
position differs.

## Architecture: Rich Word Token

The lexer emits a **single `Token::Word`** for each complete word, carrying tagged field boundaries:

```rust
Token::Word {
    raw_text: &str,            // full word text from source
    prefix: Option<&str>,      // "&-", "&~", "&+", or "0"
    body: &str,                // word body (parser handles internals)
    form_marker: Option<&str>, // "f", "z:grm" (content only, no @)
    lang_suffix: Option<WordLanguagePayload>, // Codes("eng+zho") or Shortcut
    pos_tag: Option<&str>,     // "n", "adj" (no $)
}
```

**Body parsing:** The body is too complex for fixed re2c tags (variable-length sequences of text segments, shortenings, compounds, CA markers, etc.). `parse_word_body(body: &str) -> Vec<WordBodyItem>` in `word_body.rs` scans the body string.

### Other Rich Tokens

| Token | Fields | Notes |
|-------|--------|-------|
| `IdFields` | pipe-delimited fields | @ID header, zero-copy |
| `TypesFields` | design, activity, group | @Types header |
| `MorWord` | pos, lemma_features | %mor word |
| `GraRelation` | index, head, relation | %gra relation |
| `MediaBullet` | start_time, end_time | Timestamp extraction |
| `OtherSpokenPrefix` | speaker, prefix lexeme | &*SPK: followed by the ordinary word producer |

## Implementing the shared CHAT language

Use the canonical specs and `grammar/grammar.js` to establish intended syntax,
not as an instruction to reproduce tree-sitter's implementation or recovery.
Apply the book's compatibility mandate. When implementing a construct:

1. Find the exact rule in grammar.js
2. Implement its language semantics using idiomatic re2c conditions and parser rules
3. Verify with the matching spec in `spec/constructs/` or `spec/errors/`
4. **Leverage re2c to produce richer tokens** than grammar.js's flat token model can express

Key grammar.js design decisions:
- Terminators are **optional** (`optional($.terminator)` in `utterance_end`), presence enforced by AST validation, not parsing
- Each tier type has its own content rules (`mor_contents`, `gra_contents`, `pho_groups`, `text_with_bullets`, etc.)

## Conditions (Start States)

re2c conditions are numbered states that change what rules are active:

- `INITIAL`: top-level line classification (@, *, %)
- `MAIN_CONTENT`: main tier body (words, annotations, terminators)
- `MOR_CONTENT`, `GRA_CONTENT`, `PHO_CONTENT`, `SIN_CONTENT`, tier-specific
- `ID_CONTENT`, `TYPES_CONTENT`, `LANGUAGES_CONTENT`, `PARTICIPANTS_CONTENT`, `MEDIA_CONTENT`, header-specific
- `HEADER_CONTENT`, `TIER_CONTENT`, generic structured headers/tiers

**Multiple entry points:** `Lexer::new(input, condition)` allows starting in any condition. This means we can lex a `%mor` tier body in isolation (start in `MOR_CONTENT`), a main tier content item (start in `MAIN_CONTENT`), etc.

**Continuation rule:** The lexer's continuation rule (`<*> [\r\n]+ [\t]`) must NOT reset the condition. Continuation content stays in the same lexer mode.

## Entry Points

| Entry point | Start condition | Input | Output |
|-------------|----------------|-------|--------|
| `parse_main_tier` | `INITIAL` | `*CHI:\thello .\n` | `MainTier` |
| `parse_chat_file` | `INITIAL` | full `.cha` file | `ChatFile` |
| `parse_word` | `MAIN_CONTENT` | `ice+cream@f` | `WordWithAnnotations` |
| `parse_mor_tier` | `MOR_CONTENT` | `pro\|I v\|want .\n` | `ParseOutcome<talkbank_model::MorTier>` |
| `parse_gra_tier` | `GRA_CONTENT` | `1\|2\|SUBJ 2\|0\|ROOT` | `ParseOutcome<talkbank_model::GraTier>` |
| `parse_pho_tier` | `PHO_CONTENT` | `wɑ+kɪŋ hɛloʊ .\n` | `ParseOutcome<talkbank_model::PhoTier>` |
| `parse_sin_tier` | `SIN_CONTENT` | gesture/sign body | `ParseOutcome<talkbank_model::SinTier>` |
| `parse_text_tier` | `TIER_CONTENT` | text with bullets | `ParseOutcome<BulletContent>` |
| `parse_id_header` | `ID_CONTENT` | `eng\|corpus\|CHI\|...` | `Option<IDHeader>` |
| `parse_languages_header` | `LANGUAGES_CONTENT` | comma-separated language-list body | `ParseOutcome<LanguageCodes>` |

## ChatParser Trait

`Re2cParser` implements `ChatParser` from `talkbank-model`, providing all parse methods:
- File-level: `parse_chat_file`
- Line-level: `parse_header`, `parse_utterance`, `parse_main_tier`
- Token-level: `parse_word`, `parse_mor_word`, `parse_gra_relation`
- Tier-level: `parse_mor_tier`, `parse_gra_tier`, `parse_pho_tier`, plus all text tiers

Conversion functions in `convert/` use typed AST data and, where spans are recovered, `SourceText` borrowed from the same input.

## Performance

Use the existing divan benchmark for the operation and reference inputs being
investigated. Retain the build, input set and constructor/parse boundary with
results. Historical speed ratios or profiles are not current guarantees.

Run benchmarks: `cargo bench -p talkbank-parser-re2c --bench parse_comparison`

## Build & Test

From the chatter repo root:

Select focused feedback under the root workflow. Ordinary builds consume the
vendored lexer and do not invoke `re2rust`. Regeneration and the vendored-lexer
currency check require the exact version pinned in `re2c-version.toml` on PATH.

The build script (`build.rs`) copies the vendored `src/generated/lexer.rs` into `OUT_DIR`. Edit `lexer.re`, regenerate with the exact `re2rust --no-unsafe` command documented in `build.rs`, and run `just verify-vendored-lexer`. Never edit generated output. Use `\x00` (not `\0`) for NUL, re2c treats `\0` as octal prefix.

## Testing

Tests are modules of one integration binary, `tests/integration/main.rs`.
Use a name filter for the boundary being changed:

```sh
cargo test -p talkbank-parser-re2c --test integration word_equivalence_lengthening
cargo test -p talkbank-parser-re2c --test integration error_parity::backends_report_every_tree_sitter_error -- --exact --nocapture
```

`lexer_tests`, `golden_parse`, `parser_fixtures`, `model_study`, and the
source-provenance modules cover focused behavior. `equivalence_tests` covers
the repository reference fixtures. The one-way comparison includes every
declared error-spec example, retaining actual severity and parse/validation
stages. A tree-sitter error requires some re2c error for 1.0; warnings, silence
and baseline exemptions do not satisfy it. Diagnostic/recovery identity is not
required. See `docs/parity-report.md` for finite scope and fragment limits.

Diagnose failures and repair demonstrated defects within the authorized task.
Ask only when CHAT semantics or a material architecture decision is unresolved.

A failure needs diagnosis and CHAT-rule adjudication, not a new baseline entry
or an invented recovery rule. Model equality, source spans, emitted diagnostics
and source-preserving serialization answer separate questions.

External corpus helpers remain ignored investigations. They are not release or
validity gates. Do not revive the obsolete standalone `--test` commands: their
sources now live inside the integration binary. Use the current test names and
explicit source inputs when an investigation is authorized, and retain dated
results without presenting old corpus counts as current guarantees.

## Error Token Design

Every re2c condition has a per-condition error fallback (`ErrorInMainContent`, `ErrorInMorContent`, etc.) that:
1. Consumes exactly one character
2. Stays in the same condition (lexing continues)
3. Carries context about WHERE the error occurred

The lexer NEVER fails; it always returns tokens, some of which may be error tokens.

## Recovery work

Follow the book's [Backend compatibility mandate](../../book/src/architecture/parser-backends.md#backend-compatibility-mandate),
not tree-sitter MISSING-node emulation. The parity report records observations
and reproduction commands; it cannot impose a stricter recovery-equality goal.

## Rust Coding Standards

- Rust **2024 edition**; use the root workflow's formatting and lint boundary.
- `thiserror` for domain errors, `miette` for rich diagnostics.
- No panics for recoverable conditions. No silent swallowing.
- `tracing` for library logging, never `println!`.
- Every `pub` type and function has a doc comment.
- File size: ≤400 lines recommended, ≤800 hard limit.

## Pipeline Integration

The re2c parser is wired into the main pipeline as an alternative to TreeSitterParser:

```bash
# Use re2c parser for validation
chatter validate --parser re2c corpus/reference/

# Use re2c parser with roundtrip testing
chatter validate --parser re2c --roundtrip corpus/reference/
```

Key integration points:
- `ParserKind::Re2c` in `talkbank-transform/src/validation_runner/config.rs`
- `ParserDispatch` enum in `worker.rs` wraps both parser backends
- `ParserBackend` CLI enum in `chatter/src/cli/args/core.rs`
- Cache keys include the parser label (`"re2c"` vs `"tree-sitter"`)
- TreeSitterParser remains the default; LSP always uses TreeSitterParser (needs incremental parsing)

## Equivalence Status

The reference comparison and one-way error comparison provide different evidence.
See `docs/parity-report.md` for measured scope and unresolved gaps; passing one
does not establish the other or complete backend equivalence.
