# CHAT Grammar Architecture

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/grammar/GRAMMAR.md)

A guide to the tree-sitter grammar for the [CHAT transcription format](https://talkbank.org/0info/manuals/CHAT.html).
`grammar.js` owns the executable syntax. This page explains its overall shape
and the design patterns a contributor meets first; it is not a second copy of
the rules. When this page and `grammar.js` disagree, `grammar.js` is current.
Procedure lives in the [Grammar Workflow](../book/src/contributing/grammar-workflow.md),
precedence rationale in [precedence decisions](docs/precedence-decisions.md)
and token boundaries in [tokenization rules](docs/tokenization-rules.md).

## Overview

The grammar produces a **concrete syntax tree** (CST). Whitespace is never
skipped implicitly: separators, spaces and continuation lines are matched by
explicit rules. That does not mean every byte has its own named node; some
spans are anonymous tokens (for example the body of an unsupported dependent
tier). Downstream, `crates/talkbank-parser` consumes a generated typed traversal
of this CST and lowers it into the `talkbank-model` data model.

**Recognition is not validation.** The grammar deliberately recognizes many
shapes that CHAT forbids, such as unknown header values, a missing `@UTF8` line
or trailing spaces after a tier separator, so that the model can report a
specific diagnostic instead of a generic parse failure. It does not accept all
input: text it cannot fit produces tree-sitter `ERROR` or `MISSING` recovery
nodes, which the parser reports. A clean parse is not a validity verdict; see
the book's [grammar chapter](../book/src/architecture/grammar.md) and
[leniency policy](../book/src/architecture/leniency-policy.md).

## Architecture

### Kinds of rules

1. **Leaf tokens**: regex or literal matches such as header and tier prefixes
   (`token('@Location')`, `token('%cod')`), terminators, `zero` and
   `event_segment`.
2. **Structural compositions**: `seq()`, `repeat()` and `choice()` rules that
   build CHAT constructs (`utterance`, `main_tier`, `standalone_word`,
   `mor_word`, `id_contents`).
3. **Supertypes**: abstract `choice()` rules declared in `supertypes`, so a
   query or consumer can match a whole category:

| Supertype | Groups |
|-----------|--------|
| `terminator` | utterance-ending punctuation (`.`, `?`, `!`, `+...`, `+/.` and others) |
| `linker` | discourse linkers (`++`, `+<`, `+^`, `+"`, `+,`, `+≈`, `+≋`) |
| `base_annotation` | bracket annotations such as `[!]`, `[?]`, `[//]`, `[= ...]`, `[* ...]`, `[<]` |
| `dependent_tier` | `%mor`, `%gra`, `%pho`, `%com` and the other tiers, plus `unsupported_dependent_tier` |
| `header` | `@Languages`, `@Participants`, `@ID`, `@Date` and the other headers, plus `unsupported_header` |
| `pre_begin_header` | `@PID`, `@Color words`, `@Window`, `@Font` |

The member lists are in `grammar.js`; this page intentionally does not repeat
their counts, which change whenever a construct is added.

### The `extras: []` decision

Tree-sitter normally skips whitespace through `extras`. This grammar sets
`extras: []` and threads whitespace through rules such as `whitespaces`,
`continuation`, `space`, `tab` and the separator rules. This is necessary
because:

- CHAT uses tab characters structurally (header and tier separators).
- A continuation (line break followed by a tab) continues the same header or
  tier value.
- Content-separating whitespace is needed for faithful serialization and for
  diagnostics such as trailing separator spaces.

### Conflict declarations

The `conflicts` array lists the rule sets where the GLR parser must keep more
than one interpretation alive until a later token decides. Most concern
whitespace after a content item: it may separate the next item or begin that
item's bracket annotations (`contents`, the three `*_with_optional_annotations`
rules, `base_annotations`, `final_codes`). Others concern markers that can be
standalone content or word-internal structure (`word_body` alongside
`contents` and `base_content_item`). Each entry carries a comment in
`grammar.js`; add a conflict only with the same kind of justification.

## Document Structure

`source_file` is the root. It is a choice, so the same grammar can parse a
whole file or a fragment:

```
source_file
├── full_document              complete file (highest precedence)
├── utterance                  main tier plus dependent tiers
├── main_tier                  a single main tier
├── dependent_tier             a single dependent tier
├── header                     a single header
├── pre_begin_header           @PID, @Font, @Window, @Color words
└── standalone_word            a single word

full_document
├── utf8_header?               "@UTF8"; optional so its absence can be diagnosed
├── pre_begin_header*          any order; ordering is checked downstream
├── begin_header               "@Begin"
├── line*
│   ├── header                 "@Languages:\teng", "@Date:\t01-JAN-2000"
│   ├── utterance
│   │   ├── main_tier          "*CHI:\twant more cookie ."
│   │   │   ├── star, speaker, colon, tab, sep_trailing_space?
│   │   │   └── tier_body
│   │   │       ├── linkers?
│   │   │       ├── language code?   leading "[- lang]"
│   │   │       ├── contents         words, events, groups, pauses, annotations
│   │   │       └── utterance_end    terminator?, postcodes?, bullet?, newline
│   │   └── dependent_tier*    "%mor:\tv|want qn|more n|cookie"
│   ├── blank_line
│   └── unsupported_line       catch-all for lines not starting with *, @ or %
└── end_header                 "@End"
```

## Design Patterns

### Structured words

Words are **not** opaque tokens. `standalone_word` is a sequence of an
optional prefix (`&-`, `&~`, `&+`) or omission `zero`, a `word_body`, and
optional form marker, language suffix and part-of-speech tag. `word_body`
alternates typed children: `word_segment` text, shortenings, stress markers
and structural markers such as overlap points, CA elements and delimiters,
and underline boundaries. The symbol registry supplies the character classes
that keep those markers out of `word_segment`. The
[tokenization rules](docs/tokenization-rules.md) and the book's
[word syntax](../book/src/chat-format/word-syntax.md) describe the contract.

### Bracket annotations

Simple annotations are single tokens, for example `[!]`, `[?]` and the error
marker `[* ...]`. Annotations with free-text payloads are sequences whose
opening token wins the lexer with `prec(8)` and whose payload is a field: for
example `explanation_annotation` is `[=`, a space, `field('text', annotation_content)`
and `]`. Consumers read the field rather than slicing bracket text.

### Recognized values with a catch-all

Several closed header vocabularies offer the known values together with a
generic alternative:

```javascript
option_name: $ => choice('CA', 'NoAlign', $.generic_option_name),
generic_option_name: $ => /[^\s,\r\n\t]+/,
```

Tree-sitter's lexer prefers a string literal over a regex match of the same
length, so known values win. An unknown value is still recognized and is
reported by the model rather than becoming a parse error. Some vocabularies
use structured alternatives rather than plain literals (`id_ses`, for
example, distinguishes combined, code and ethnicity values from a generic
value); consult each rule before assuming the simple pattern.

### Explicit whitespace and continuation

The `whitespaces` token matches runs of spaces and continuation sequences (line
break followed by a tab). Free-text header values use `free_text`, a repetition
of `rest_of_line` and `continuation`, so a multi-line value such as

```
@Location:	Room 5
	Building A
```

parses as one `free_text` node. Spaces after a header or tier separator are a
distinct `sep_trailing_space` node, so they can be diagnosed (E758) and dropped
on serialization instead of becoming content.

## Precedence

Precedence resolves lexical ambiguity: structural tokens such as terminators
and linkers must win over word content, annotation openers over words, and
the event-segment catch-all only when nothing else matches. The numeric levels
and the reasoning behind each non-obvious choice are recorded in
[precedence decisions](docs/precedence-decisions.md). That record is design
rationale with historical observations, not a proof over every parser state.

## Downstream Integration

### From CST to validated model

1. **tree-sitter** parses CHAT source into a CST with this grammar.
2. **Generated typed traversal** (`crates/talkbank-parser/src/generated_traversal.rs`
   and the C metadata bridge in `bindings/rust/`) gives the parser typed access
   to each node's children. It is regenerated from the grammar by the pinned
   traversal recipe, never edited by hand.
3. **`talkbank-parser`** lowers the typed CST into a `ChatFile` and reports
   recovery nodes as diagnostics.
4. **`talkbank-model`** validates the model (error codes, cross-tier alignment).
5. **`talkbank-transform`** orchestrates parse and validation pipelines and
   CHAT/JSON conversion.

The independent re2c backend (`talkbank-parser-re2c`) does not use this grammar.

### Generated consumers

`src/node-types.json` and the Rust node-kind constants in
`crates/talkbank-parser/src/node_types.rs` are generated, as are the traversal,
metadata bridge and conformance inventory. `just regen` (from the repository
root) regenerates all of them in dependency order; the ownership table in
[grammar agent guidance](AGENTS.md#source-and-output-ownership) names the
owner of each file.

### Editor queries

`queries/` provides the minimal query set expected by the language bindings:
highlights, injections, locals and tags. `just grammar-test` checks that each
query compiles against a reference file; it does not verify highlight results.
Additional query families (folds, indents, text objects) remain post-v1 work.

## Adding New Constructs

Every addition follows the [Grammar Workflow](../book/src/contributing/grammar-workflow.md):
edit `grammar.js`, run `just regen`, review the generated changes, add typed
lowering in `talkbank-parser`, and add authored examples under `spec/constructs/`
or `spec/errors/`. The grammar-level steps for common cases are:

### New header

1. Add a prefix token with the other prefixes, for example
   `my_header_prefix: $ => token('@MyHeader')`.
2. Add a header rule in the usual shape,
   `seq($.my_header_prefix, $.header_sep, content, $.newline)`.
3. Add it to the `header` supertype choice. Until then, `unsupported_header`
   recognizes the line.

### New dependent tier

1. Add a tier prefix token, for example `my_tier_prefix: $ => token('%myt')`.
2. Add a tier rule, `seq($.my_tier_prefix, $.tier_sep, body, $.newline)`.
3. Add it to the `dependent_tier` supertype choice. Until then,
   `unsupported_dependent_tier` recognizes unknown `%label` tiers, and
   `x_dependent_tier` already covers user-defined `%x...` tiers.

### New annotation type

1. Add a token, or a sequence starting with a token, whose opener uses
   `prec(8)` so it beats word content.
2. Add it to the `base_annotation` supertype choice.
3. Check that its pattern does not overlap an existing bracket opener, and
   record any non-obvious precedence choice in
   [precedence decisions](docs/precedence-decisions.md).
