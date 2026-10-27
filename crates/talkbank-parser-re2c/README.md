# talkbank-parser-re2c

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/crates/talkbank-parser-re2c/README.md)

CHAT transcript parser using a
[re2c](https://re2c.org/manual/manual_rust.html) DFA lexer and
[chumsky](https://docs.rs/chumsky/1.0.0-alpha.8) parser combinators.

This crate and `talkbank-parser` are peer implementations of the `ChatParser`
trait from `talkbank-model`. Tree-sitter was prototyped first and is the current
CLI default; the shared model belongs to neither backend. Both parsers can
drive model changes, including explicit malformed and recovery states. Prefer
shared model types to duplicate semantic ASTs; backend-specific tokens and
parser-progress carriers still have their own role. See
[backend ownership](https://github.com/TalkBank/chatter/blob/main/book/src/architecture/parser-backends.md#equal-backends-one-shared-model).

Neither backend is a specification oracle. The existing 1.0 regression contract requires
some error for every input that tree-sitter reports as erroneous; exact
diagnostics and recovered models may differ. Known silent acceptance is a
defect, not an experimental exemption.

## Compatibility contract

The book's [Backend compatibility mandate](https://github.com/TalkBank/chatter/blob/main/book/src/architecture/parser-backends.md#backend-compatibility-mandate)
is authoritative for users and developers. Read it before switching parsers or
changing a parity expectation; the [parity report](docs/parity-report.md)
contains dated measurements, not a competing policy.

Use `talkbank-parser-re2c` when you want:

- an independent parser implementation for cross-checking or fuzzing
- direct access to the re2c lexer and token stream
- independent full-file parsing where incremental reparsing is not required

Use `talkbank-parser` when you want the default tree-sitter-based parser,
incremental parsing, or the parser surface used by the LSP and the rest of the
default toolchain.

## Library use

The [compiled library recipes](https://github.com/TalkBank/chatter/blob/main/book/src/chatter/integrating/library-usage.md)
run both backends through `parse_validated_with_parser` and
`parse_source_with_parser`. These accept `ChatParser` and return the same shared
admission capabilities. For recovery-oriented parsing, use
`talkbank_model::ParseProduct::parse(&Re2cParser::new(), source)` to retain the
model alongside diagnostics. A built model is not a validity claim.

## Architecture

```
re2c DFA Lexer          Chumsky Combinators         AST-to-Model
  (lexer.re)              (parser/)                  (convert/)

source text             &[Token<'a>]                ast types         talkbank-model
    |                        |                          |                  |
    v                        v                          v                  v
  Lexer::new(input, condition) --> main_tier_parser() --> MainTier<'a> --> model::MainTier
                          gra_tier_parser(...)    model::GraTier (direct)
                          mor_tier_parser(...)    model::MorTier (direct)
                          ...                      ...                ...
```

**Lexer:** re2c generates a DFA (`lexer.re` -> `lexer.rs`) that produces
rich tokens with tagged field extraction. A single `Token::Word` carries
raw_text, prefix, body, form_marker, lang_suffix, and pos_tag -- all
zero-copy `&str` slices.

**Parser:** Chumsky 1.0-alpha combinators consume `&[Token]` and produce shared
models or parser-specific carriers. Text-tier bodies are recognized as token
slices and lowered with the existing shared `BulletContent` builder, without
a second semantic text-tier AST. MOR, GRA, PHO and SIN also construct shared model
values directly; GRA construction rejects the whole tier if a numeric field fails.
File recovery preserves the original GRA source as UnparsedGra. PHO/SIN body
APIs return explicit ParseOutcome rejection; malformed PHO no longer becomes an
empty successful tier. PHO compounds and PHO/SIN groups use shared model values,
with tier locations supplied at construction. Failed PHO/MOD/SIN file tiers
retain their exact source as shared UnparsedPho/UnparsedMod/UnparsedSin values;
JSON import does not erase their invalidity or alignment taint. WOR parsing
constructs shared WorItem/WorTier directly with its admitted tier extent;
unsupported word shapes reject instead of producing empty-word placeholders.
Rejected file WOR retains original source as UnparsedWor; it is not reconstructed
or reparsed and JSON preserves its invalidity and timing restrictions. Word/annotation
lowering and orphan-bullet policy remain separate boundaries.
Main content and other remaining families still use native
carriers under assessment. The `recursive()` combinator handles nested groups and
quotations. File-level dispatch also has an imperative loop for line ownership
and recovery; not every parsing stage is a combinator.

**Conversion:** The `convert/` module lowers intermediate AST types into the
public `talkbank-model` types, retaining diagnostics. Source-aware conversions
use the original input's owner to locate borrowed slices. Reconstructed word
text is owned by the AST. Borrowed intermediate ASTs are not borrowed public
`ChatFile` models; the `ChatParser` entry point includes conversion.

## Benchmark scope

The [divan benchmark](benches/parse_comparison.rs) now measures both file
backends through `ChatParser`, producing the public model at offset zero with
the same diagnostic collector. Main/MOR/GRA tier measurements also use shared
trait entry points. Lex-only measurements remain a separate lower-level cost.
The timed regions use preloaded inputs, not file I/O or model validation.
Constructor-inclusive and reused tree-sitter cases are separate; the batch case
constructs one tree-sitter parser per batch. Benchmark completion does not prove
semantic equivalence, error-policy coverage or end-to-end CLI throughput.

### Historical measurements, not current speed guarantees

The tables below retain earlier reported timings. They lack a complete pinned
build/hardware receipt here and have not been rerun for the current source.
More importantly, the earlier comparison timed tree-sitter's public-model
production against re2c's **intermediate AST**, omitting the latter's conversion
and using different diagnostic paths. Do not interpret these ratios as the
relative cost of equivalent public APIs or compare them directly with the
corrected shared-model benchmark. No new performance result is claimed.

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
69-85% was attributed to parser and allocation overhead in this historical
measurement. Current parsing borrows source and drops temporary token storage;
these timings have not been remeasured after that ownership change.

### Architectural differences are not timing evidence

`Re2cParser` is a unit struct and its parser consumes rich lexer tokens;
tree-sitter constructs a CST that is lowered into the model. Both have work
outside token recognition. Those differences motivate measurement but do not
establish which current operation is faster or explain a measured ratio without
a matched profile and result boundary.

### Limitations

- **No incremental parsing.** TreeSitter supports incremental reparsing
  (essential for the LSP). Re2c+chumsky does not. The LSP always uses
  TreeSitterParser.
- **Error recovery.** The backends have different recovery architectures.
  re2c has parser-owned recovery paths but does not promise tree-sitter's
  recovered output or diagnostic identity. See the compatibility contract.

## Build & Test

Run these commands from the repository root. Ordinary Cargo builds copy the
committed generated lexer into the build output; they do **not** require or
invoke `re2rust` and do not verify that generated code is current.

```bash
cargo check -p talkbank-parser-re2c
cargo test -p talkbank-parser-re2c
cargo bench -p talkbank-parser-re2c --bench parse_comparison
```

For lexer-source changes, use the exact generator version in
[`re2c-version.toml`](https://github.com/TalkBank/chatter/blob/main/re2c-version.toml), regenerate with the invocation
in [`build.rs`](build.rs), and run `just verify-vendored-lexer`. That currency
check compares generated bytes; it does not replace parser behavior tests.
Do not hand-edit `src/generated/lexer.rs`.

## CLI Integration

```bash
chatter validate --parser re2c corpus/reference/
chatter validate --parser re2c --roundtrip corpus/reference/
```

TreeSitterParser remains the default. Re2c is opt-in via `--parser re2c`.

## License

MIT OR Apache-2.0.
