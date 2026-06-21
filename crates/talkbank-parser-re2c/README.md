# talkbank-parser-re2c

**Last modified:** 2026-05-30 20:04 EDT

Alternate CHAT transcript parser using a
[re2c](https://re2c.org/manual/manual_rust.html) DFA lexer and
[chumsky](https://docs.rs/chumsky/1.0.0-alpha.8) parser combinators.

This crate implements the `ChatParser` trait from `talkbank-model`, but it is
not the default parser surface for the TalkBank toolchain. Its primary role is
to provide an independently implemented parser that can act as an equivalence
oracle against `talkbank-parser`, while still remaining usable as a specialist
parser surface in its own right.

Use `talkbank-parser-re2c` when you want:

- a second parser implementation for cross-checking or fuzzing
- direct access to the re2c lexer and token stream
- fast batch parsing where incremental reparsing is not required

Use `talkbank-parser` when you want the default tree-sitter-based parser,
incremental parsing, or the parser surface used by the LSP and the rest of the
default toolchain.

## Architecture

```
re2c DFA Lexer          Chumsky Combinators         AST-to-Model
  (lexer.re)              (parser/)                  (convert.rs)

source text             &[Token<'a>]                ast types         talkbank-model
    |                        |                          |                  |
    v                        v                          v                  v
  Lexer::new(input)  -->  main_tier_parser()  -->  MainTier<'a>  -->  model::MainTier
                          gra_tier_parser()        GraTier<'a>        model::GraTier
                          mor_tier_parser()        MorTier<'a>        model::MorTier
                          ...                      ...                ...
```

**Lexer:** re2c generates a DFA (`lexer.re` -> `lexer.rs`) that produces
rich tokens with tagged field extraction. A single `Token::Word` carries
raw_text, prefix, body, form_marker, lang_suffix, and pos_tag -- all
zero-copy `&str` slices.

**Parser:** Chumsky 1.0-alpha combinators consume `&[Token]` and produce
AST types. The `recursive()` combinator handles nested groups and
quotations. Parser combinators replace the original 1,923-line
hand-written recursive descent parser.

**Conversion:** `From` impls convert AST types to `talkbank-model` types.
All conversions are source-free -- the AST is self-contained via
`raw_text` fields.

## Performance

Benchmarked on the reference corpus (87 CHAT files) using
[divan](https://docs.rs/divan). All content pre-loaded; zero I/O during
measurement.

### File-level parse (median, parser reuse)

| File | TreeSitter | Re2c+Chumsky | Speedup |
|------|-----------|-------------|---------|
| basic-conversation (13 lines) | 44 us | 9.6 us | 4.6x |
| mor-gra (with dependent tiers) | 69 us | 9.4 us | 7.3x |
| intonation (CA notation) | 78 us | 19 us | 4.1x |
| zho-conversation (CJK) | 128 us | 19 us | 6.6x |
| impdenis (complex, large) | 7,734 us | 970 us | 8.0x |

### Batch parse (35 representative files)

| Parser | Time | Files/sec |
|--------|------|-----------|
| TreeSitter | 21.7 ms | 1,613 |
| Re2c+Chumsky | 3.0 ms | 11,667 |
| **Speedup** | | **7.2x** |

### Tier-level parse (median)

| Tier | TreeSitter | Re2c+Chumsky | Speedup |
|------|-----------|-------------|---------|
| main_tier | 10.4 us | 2.6 us | 4.0x |

### Lex-only (re2c DFA floor)

| Input | Lex time | Full parse | Lex share |
|-------|----------|-----------|-----------|
| main tier line | 401 ns | 2,624 ns | 15% |
| %mor tier body | 270 ns | 885 ns | 31% |
| full file (mor-gra.cha) | 2,603 ns | 9,374 ns | 28% |

The re2c DFA accounts for 15-31% of total parse time. The remaining
69-85% is chumsky combinator overhead (backtracking, AST construction,
`Box::leak` for lifetime management).

### Why re2c is faster

1. **Zero constructor cost.** `Re2cParser` is a unit struct.
   `TreeSitterParser` loads the tree-sitter grammar on construction.
2. **DFA lexing.** re2c compiles regex patterns to a deterministic finite
   automaton at build time. Tree-sitter's GLR lexer is more general but
   slower for the fixed CHAT grammar.
3. **Rich tokens.** The re2c lexer extracts word fields during lexing
   (one DFA pass). TreeSitter produces a flat CST that requires a second
   traversal pass to extract the same information.
4. **No CST intermediate.** Re2c produces tokens directly consumed by
   chumsky. TreeSitter produces a full concrete syntax tree, then the
   Rust parser traverses it to build the model.

### Limitations

- **Long-running service posture:** This crate is designed first as an
  independently implemented parser oracle and batch parser, not as the
  toolchain's long-lived incremental parser service. The LSP and default
  editor/runtime path continue to use `talkbank-parser`.
- **No incremental parsing.** TreeSitter supports incremental reparsing
  (essential for the LSP). Re2c+chumsky does not. The LSP always uses
  TreeSitterParser.
- **Error recovery.** TreeSitter has built-in error recovery producing
  partial CSTs. The chumsky parser reports unhandled tokens via
  `ErrorSink` but does not attempt structural recovery.

## Build & Test

Requires `re2rust` (part of re2c) on PATH: `brew install re2c`.

```bash
cargo check -p talkbank-parser-re2c
cargo nextest run -p talkbank-parser-re2c
cargo bench -p talkbank-parser-re2c --bench parse_comparison
```

## CLI Integration

```bash
chatter validate --parser re2c corpus/reference/
chatter validate --parser re2c --roundtrip corpus/reference/
```

TreeSitterParser remains the default. Re2c is opt-in via `--parser re2c`.

## License

MIT OR Apache-2.0.
