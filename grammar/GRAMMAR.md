# CHAT Grammar Architecture

A guide to the tree-sitter grammar for the [CHAT transcription format](https://talkbank.org/0info/manuals/CHAT.html).

## Overview

This grammar produces a **concrete syntax tree** (CST), not an abstract syntax tree. Every byte of the input is represented in the tree, whitespace, delimiters, and continuation lines are all explicit nodes. Downstream consumers (the Rust parser in `crates/talkbank-parser`) walk the CST to build a semantic AST.

**Design philosophy: "Parse, don't validate."** The grammar accepts all plausible CHAT input. Invalid values (wrong date formats, unknown option names, etc.) parse successfully into the CST and are flagged by the Rust validator. This means the grammar never rejects a file that a human could plausibly have written.

## Architecture

### Three tiers of rules

1. **Leaf tokens**: atomic regex or string matches (`standalone_word`, `inline_bullet`, `pause_token`, terminators). These are opaque to tree-sitter; internal structure is parsed downstream.

2. **Structural compositions**: `seq()`, `repeat()`, `choice()` rules that combine leaf tokens into CHAT constructs (`utterance`, `mor_word`, `id_contents`).

3. **Supertypes**: abstract `choice()` rules that group related node types for query convenience:

| Supertype | Groups |
|-----------|--------|
| `terminator` | `.` `?` `!` `+...` `+/.` `+//.` etc. (13 variants) |
| `linker` | `++` `+<` `+^` `+"` `+,` `+≈` `+≋` |
| `base_annotation` | `[!]` `[?]` `[//]` `[= ...]` `[* ...]` etc. (17 variants) |
| `dependent_tier` | `%mor` `%gra` `%pho` `%sin` `%com` etc. (26 variants) |
| `header` | `@Languages` `@Participants` `@ID` `@Date` etc. (30 variants) |
| `pre_begin_header` | `@PID` `@Color words` `@Window` `@Font` |

### The `extras: []` decision

Tree-sitter normally auto-skips whitespace via `extras`. This grammar sets `extras: []` and handles all whitespace explicitly through `whitespaces`, `continuation`, `space`, and `tab` tokens. This is necessary because:

- CHAT uses tab characters structurally (header separator, tier separator)
- Continuation lines (newline + tab) are meaningful; they indicate multi-line content
- Whitespace between content items must be preserved for roundtrip serialization

### Conflict declarations

Five rules require explicit `conflicts` declarations because tree-sitter's GLR parser needs guidance on ambiguous sequences:

- `contents`: a content item followed by whitespace could be the start of another item or trailing whitespace before a terminator
- `word_with_optional_annotations` / `nonword_with_optional_annotations`, bracket annotations after a word could belong to the word or be a separate content item
- `base_annotations`: multiple annotations in sequence create shift-reduce ambiguity
- `final_codes`: postcodes after terminators have similar ambiguity

## Document Structure

```
document
├── utf8_header              "@UTF8\n"
├── pre_begin_header*        "@PID", "@Font", "@Window", "@Color words"
├── begin_header             "@Begin\n"
├── line*
│   ├── header               "@Languages:\teng\n", "@Date:\t01-JAN-2000\n"
│   ├── utterance
│   │   ├── main_tier        "*CHI:\twant more cookie ."
│   │   │   ├── star, speaker, colon, tab
│   │   │   └── tier_body
│   │   │       ├── linkers?
│   │   │       ├── contents     (words, events, groups, pauses, annotations)
│   │   │       └── utterance_end (terminator, postcodes, media_url, newline)
│   │   └── dependent_tier*  "%mor:\tv|want qn|more n|cookie"
│   └── unsupported_line     catch-all for unrecognized lines
└── end_header               "@End\n"
```

## Design Patterns

### Opaque word tokens

Word-internal structure (prosody markers, CA features, shortenings, compound markers, form/language suffixes) is **not** parsed by the grammar. The `standalone_word` rule captures everything between word boundaries as a single opaque token. The Rust direct parser then analyzes word internals via `parse_word_impl()`.

**Rationale:** Word-internal syntax is too complex and context-dependent for tree-sitter's lexer. A separate parsing pass can use full backtracking and richer error recovery.

### Atomic annotation tokens

Bracket annotations like `[= text]`, `[! text]`, `[* code]` are single `token()` rules. The grammar captures the complete bracket contents; the Rust parser extracts the payload text.

**Rationale:** Annotation internals are simple enough to extract with string operations. Making them atomic avoids state explosion from nested bracket disambiguation.

### Strict + catch-all (10 rules)

For header fields with a closed set of valid values, the grammar provides both strict matches and a generic catch-all:

```javascript
option_name: $ => choice('CA', 'NoAlign', $.generic_option_name),
generic_option_name: $ => /[^\s,\r\n\t]+/,
```

Tree-sitter's DFA gives string literals priority over regexes at the same length, so known values win automatically. Unknown values fall through to the catch-all and are flagged by the Rust validator. This pattern applies to: `option_name`, `media_type`, `media_status`, `recording_quality_option`, `transcription_option`, `number_option`, `date_contents`, `time_duration_contents`, `id_sex`, `id_ses`.

Note: `id_ses` uses `token(prec(1, regex))` instead of string literals to avoid keyword conflicts, ethnicity words like White, Black, Native appear in utterance text.

### Explicit whitespace and continuation

With `extras: []`, the grammar must thread whitespace tokens through every rule that allows spaces. The `whitespaces` token matches `repeat1(choice(' ', /[\r\n]+\t/))`, either spaces or continuation sequences (newline + tab). This means a multi-line header value like:

```
@Location:	Room 5
	Building A
```

is parsed as a single `free_text` node containing `rest_of_line` + `continuation` + `rest_of_line`.

## Precedence Strategy

The grammar uses four precedence tiers to resolve tokenization ambiguity:

| Level | Used by | Beats |
|-------|---------|-------|
| **prec(10)** | Terminators (`+...`, `+/.`), linkers (`++`, `+<`), CA markers, group delimiters | Everything; these are structural tokens that must never be consumed as word content |
| **prec(8)** | Bracket annotations (`[= ...]`, `[=! ...]`), `langcode` | standalone_word, but not structural tokens |
| **prec(5)** | `standalone_word`, overlap points | Event segments, generic catch-alls |
| **prec(1)** | `event_segment` | Nothing, lowest priority, only matches when nothing else does |

Additional: `zero` uses prec(3) to beat `natural_number` (prec 2) for the character `0`.

## Downstream Integration

### CST → AST pipeline

1. **tree-sitter** parses CHAT source → CST (this grammar)
2. **`talkbank-parser`** walks CST → `ChatFile` AST (Rust data model)
3. **`talkbank-model`** validates the AST (error codes, cross-tier alignment)
4. **`talkbank-transform`** orchestrates parse + validate pipelines, CHAT↔JSON roundtrip

### `node_types.rs` generation

The grammar generates `src/node-types.json`. The Rust constants file
`node_types.rs` is generated from this grammar output:

```bash
node scripts/generate-node-types.js > crates/talkbank-parser/src/node_types.rs
```

### Editor queries

The `queries/` directory provides the minimal boilerplate query set expected by the language bindings: syntax highlighting, injections, locals, and tags.

Additional query families (for example folds/indents/textobjects) remain post-v1 TODO work.

## Adding New Constructs

### New header

1. Add a `token()` prefix rule (e.g., `my_header_prefix: $ => token('@MyHeader')`)
2. Add a header rule using `seq(prefix, header_sep, content, newline)`
3. Add it to the `header` supertype choice
4. Run `tree-sitter generate && tree-sitter test`
5. Add a downstream parser in `talkbank-parser`

### New dependent tier

1. Add a `token()` tier prefix (e.g., `my_tier_prefix: $ => token('%myt')`)
2. Add a tier rule using `seq(prefix, tier_sep, content, newline)`
3. Add it to the `dependent_tier` supertype choice
4. Existing `unsupported_dependent_tier` catch-all will handle it until the parser is updated

### New annotation type

1. Add a `token()` rule with the bracket pattern (use `prec(8)` to beat `standalone_word`)
2. Add it to the `base_annotation` supertype choice
3. Ensure the regex doesn't conflict with existing bracket patterns
