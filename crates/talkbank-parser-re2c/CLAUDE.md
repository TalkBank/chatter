# CLAUDE.md

**Last modified:** 2026-09-07 06:57 EDT

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

**Cross-ref (root `CLAUDE.md` "CST Traversal Rules"):** the *tree-sitter* production
parser (`talkbank-parser`) must be driven by the exhaustive generated typed
traversal module (`generated_traversal`), no hand-walk of `node.kind()` strings, no
ERROR-text-classification. That traversal rule is specific to the tree-sitter CST and
does NOT apply to this re2c parser's internals: `Re2cParser` is the **independent
equivalence oracle**, with its own lexer/parser, and must produce byte-identical
`ChatFile` ASTs. The general no-text-hacking principle (detect errors from
structure, never by scanning raw CHAT text) DOES apply here.

## What This Is

A CHAT transcript parser using **re2rust** (re2c's Rust backend) for lexing and **chumsky** parser combinators for parsing. Lives in the chatter workspace as `talkbank-parser-re2c`.

**Status:** Implements `ChatParser` from `talkbank-model` through `Re2cParser`, selected by `chatter validate --parser re2c`. Diagnostic and recovery parity remain incomplete. Current measurements, their limits, and reproducible commands belong in `docs/parity-report.md`; do not infer correctness or current speed from historical corpus results.

## Architecture

```
re2c DFA Lexer  -->  Chumsky Combinators  -->  AST  -->  talkbank-model
  (lexer.re)          (parser/*.rs)       (ast.rs)     (convert.rs)
```

**Two-stage pipeline:**
1. **Lexer** (`lexer.re`), re2c DFA produces rich tokens with tagged field extraction.
2. **Parser** (`parser/`), chumsky combinators consume `&[Token]` and produce AST types.
3. **Conversion** (`convert.rs`), `From` impls map AST to talkbank-model. Source-aware conversions use `SourceText` to place original input slices.

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

**Borrowed source, temporary tokens.** Parser signatures separate `'tokens` from `'source`. The lexer borrows the caller's input and treats end-of-buffer as NUL without padding or unchecked reads. Local token and recovery vectors are dropped after parsing. `WordWithAnnotations::raw_text` is `Cow<str>`: borrowed for rich tokens, owned for reconstructed subtoken words. Never leak allocations to extend a lifetime.

**Imperative file parser.** The file-level parser (`file.rs`) uses an imperative loop for line ownership and recovery. Exact lexer rules emit `DependentPrefixToken` with a `DependentBodyKind`; the dependent-tier parser exhaustively dispatches that admitted kind. Never reclassify labels with prefix-string tests: `%modsyl` and `%mod` select different body grammars.

**CA terminator promotion.** CA intonation arrows (⇗ ↗ → ↘ ⇘) serve dual roles: mid-content separators and utterance-final terminators. Chumsky always parses them as separators. `convert.rs` promotes trailing arrows to terminators at the AST-to-model boundary (same strategy as TreeSitterParser's `resolve_ca_terminator`).

**Subtoken word assembly.** When the lexer produces sub-tokens instead of a single rich `Token::Word` (edge cases where the `w_body` regex doesn't match), `subtoken_word()` in `main_tier.rs` assembles them. The `display_text()` helper reconstructs raw_text with structural delimiters (e.g., `Shortening("x")` -> `"(x)"`).

## Architecture: Rich Word Token

The lexer emits a **single `Token::Word`** for each complete word, carrying tagged field boundaries:

```rust
Token::Word {
    raw_text: &str,            // full word text from source
    prefix: Option<&str>,      // "&-", "&~", "&+", or "0"
    body: &str,                // word body (parser handles internals)
    form_marker: Option<&str>, // "f", "z:grm" (content only, no @)
    lang_suffix: Option<&str>, // "eng+zho" (no @s:), "" for bare @s
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
| `OtherSpokenEvent` | speaker, text | &*SPK:word |

## Strict Adherence to grammar.js

The canonical grammar is `grammar/grammar.js` at this repo's root. All lexer rules and parser logic must be **directly translated** from grammar.js, not invented, not approximated. When implementing a construct:

1. Find the exact rule in grammar.js
2. Translate it to re2c conditions/rules, leveraging re2c features
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
| `parse_mor_tier` | `MOR_CONTENT` | `pro\|I v\|want .\n` | `MorTier` |
| `parse_gra_tier` | `GRA_CONTENT` | `1\|2\|SUBJ 2\|0\|ROOT` | `GraTier` |
| `parse_pho_tier` | `PHO_CONTENT` | `wɑ+kɪŋ hɛloʊ .\n` | `PhoTier` |
| `parse_text_tier` | `TIER_CONTENT` | text with bullets | `TextTierParsed` |
| `parse_id_header` | `ID_CONTENT` | `eng\|corpus\|CHI\|...` | `IdHeaderParsed` |

## ChatParser Trait

`Re2cParser` implements `ChatParser` from `talkbank-model`, providing all parse methods:
- File-level: `parse_chat_file`
- Line-level: `parse_header`, `parse_utterance`, `parse_main_tier`
- Token-level: `parse_word`, `parse_mor_word`, `parse_gra_relation`
- Tier-level: `parse_mor_tier`, `parse_gra_tier`, `parse_pho_tier`, plus all text tiers

Conversion functions in `convert/` use typed AST data and, where spans are recovered, `SourceText` borrowed from the same input.

## Performance

Benchmarked with divan on reference corpus files. All content pre-loaded; zero I/O.
Most of the per-parse cost is in the chumsky combinators; re2c lexing is the smaller fraction.
TreeSitter constructor cost is negligible.

Run benchmarks: `cargo bench -p talkbank-parser-re2c --bench parse_comparison`

## Build & Test

From the chatter repo root:

```sh
cargo check -p talkbank-parser-re2c
cargo test -p talkbank-parser-re2c
cargo test -p talkbank-parser-re2c -j 1   # fallback
```

Requires `re2rust` (part of re2c) on PATH: `brew install re2c`.

The build script (`build.rs`) copies the vendored `src/generated/lexer.rs` into `OUT_DIR`. Edit `lexer.re`, regenerate with the exact `re2rust --no-unsafe` command documented in `build.rs`, and run `just verify-vendored-lexer`. Never edit generated output. Use `\x00` (not `\0`) for NUL, re2c treats `\0` as octal prefix.

## Testing

Tests are modules of one integration binary, `tests/integration/main.rs`.
Use a name filter for the boundary being changed:

```sh
cargo test -p talkbank-parser-re2c --test integration word_equivalence_lengthening
cargo test -p talkbank-parser-re2c --test integration error_parity::backends_diverge_only_where_recorded -- --nocapture
```

`lexer_tests`, `golden_parse`, `parser_fixtures`, `model_study`, and the
source-provenance modules cover focused behavior. `equivalence_tests` covers
the repository reference fixtures. The spec-parity ratchet compares diagnostic
sets and declared expectations separately; see `docs/parity-report.md`.

**When a test fails, STOP and ask.** CHAT semantics are domain-specific.

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

## MISSING-Token Recovery Policy

When the canonical (tree-sitter) parser encounters a malformed input
that would otherwise fail to parse, it recovers by inserting a
zero-length **MISSING** placeholder for the expected terminal and
continues, visible in `tree-sitter parse` output as
`(MISSING <kind> [row, col] - [row, col])`. The talkbank-parser
tree-sitter side handles this with a two-track strategy: silent
recovery in the model AST plus a `ParseError` diagnostic for each
MISSING node (see
`crates/talkbank-parser/src/parser/tree_parsing/parser_helpers/error_checking.rs`
and the `// CRITICAL: Check for MISSING nodes - tree-sitter error
recovery` comments at every tier-level CST entry).

**Re2cParser must mirror that strategy** when it discovers a missing
expected terminal: produce the same recovered model shape (so
`SemanticEq` agrees on the AST) AND emit a matching diagnostic via
`ErrorCollector` (so the malformed input remains visible to validators
and the CLI). "Treat as known divergence" is *not* an option, the
parity goal is whole-AST agreement, including on recovered shapes.

Full policy + concrete examples + the table of construct/recovery
pairs: `docs/parity-report.md` § MISSING-Token Recovery Policy.

## Rust Coding Standards

- Rust **2024 edition**, `cargo fmt` before committing.
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

The checked-in reference-fixture gate and the invalid-spec ratchet provide different evidence. See `docs/parity-report.md` for the measured scope and unresolved gaps; passing one does not establish complete backend equivalence.
