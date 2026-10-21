# CHAT Tokenization Rules

**Status:** Current
**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/grammar/docs/tokenization-rules.md)

This document explains how CHAT text is split into tokens (words, markers,
annotations), focusing on cases where adjacency and grammatical position
change the structure. The examples illustrate selected shapes, not complete
valid-transcript fixtures or a second grammar specification.

## The Fundamental Rule

Whitespace is explicit in the grammar and separates many content items.
It is not a universal tokenizer for CHAT: adjacent characters may form several
structured tokens, and headers, annotations and tiers have distinct rules.
See the book's [Word Internals](../../book/src/chat-format/word-internals.md)
for the current grammar/model boundary and lexical projection.

## The Word Purity Invariant

The grammar excludes its declared structural markers from `word_segment`.
Within a word, overlap, CA, underline, stress and lengthening markers have
separate typed children in `word_body`. This separation means:

- Lexical projection can consume typed leaves rather than reparsing marker text.
- Word-level validators can inspect the retained marker variants; standalone
  content markers still require their own traversal.
- Editors can distinguish those grammar nodes from lexical segments.

This is not proof that every recognized character is valid CHAT or represents
spoken material. Recovery, imported model values and language-specific lexical
validation retain separate contracts.

The exclusions are built from the symbol registry (`src/generated_symbol_sets.js`)
a single source of truth shared by the grammar and Rust code.

See [Precedence Decisions](precedence-decisions.md#word-segment-purity-invariant)
for the full exclusion table.

## The Exceptions

### Exception 1: Overlap Markers (⌈⌉⌊⌋)

Overlap markers indicate simultaneous speech between speakers. They can
appear either **inside** or **outside** words, depending on whitespace:

```
*CHI:   Yeah⌋⌈2 hey .
        ^^^^^^^^       ← ONE word: "Yeah⌋⌈2" (no space = part of word)

*CHI:   Yeah ⌋ ⌈2 hey .
        ^^^^           ← word: "Yeah"
             ^^^       ← overlap_point: ⌋
               ^^^^    ← overlap_point: ⌈2
                   ^^^ ← word: "hey"

```

**Key insight:** an adjacent marker can belong to `word_body`, but remains a
separate `overlap_point` child. It is never swallowed by `word_segment`.
Word ownership is not token identity. A marker-only sequence does not satisfy
the marker-initial word branch merely because its characters are contiguous.

The lexical projection excludes structural overlap leaves from `cleaned_text()`;
the typed model and serialized transcription retain them. Downstream consumers
should not implement a second character-stripping parser.

### Exception 2: Zero/Omission Prefix (0)

The `0` character as a word prefix means "omitted word." But standalone
`0` means "action marker." Whitespace determines which:

```
*CHI:   0die .
        ^^^^           ← ONE word: standalone_word(zero, word_body("die"))
                         Category: Omission

*CHI:   0 die .
        ^              ← nonword(zero), action marker
          ^^^          ← word: "die"
```

**How it works in the grammar:**
- `standalone_word` has `prec.right(6)`, `nonword` has `prec(1)`
- When tree-sitter sees `zero • word_body`, prec(6) beats prec(1) → one word
- `extras: []` (no implicit whitespace) prevents `0 die` from merging
- `zero` is inlined directly into `standalone_word` (not through `word_prefix`)
  to keep the competing zero alternatives at the intended grammar boundary

**See also:** [Precedence Decisions](precedence-decisions.md) for the scoped
design rationale and the limits of the historical experiments.

### Exception 3: CA Mode Parenthetical = Non-CA Omission

In Conversation Analysis (`@Options: CA`) mode, `(word)` means
uncertain/barely audible speech, semantically equivalent to non-CA `0word`
(omission). The parentheses are NOT the same as shortening notation:

```
@Options:  CA
*CHI:   (ja) .
        ^^^^           ← CAOmission: uncertain "ja"
                         Equivalent to non-CA: 0ja

*CHI:   hel(lo) .
        ^^^^^^^        ← Shortening: "(lo)" is the shortened part
                         NOT an omission
```

**The ambiguity:** `(word)` in non-CA mode is always shortening. In CA mode,
a fully-parenthesized word `(word)` becomes a CA omission. Partially
parenthesized `hel(lo)` is always shortening regardless of mode.

**How to distinguish:** Check `@Options` header for `CA`. The parser sets
`WordCategory::CAOmission` when the word is fully parenthesized in CA mode.
This requires file-level context (`FragmentSemanticContext`), isolated
`parse_word()` cannot determine CA mode.

### Exception 4: Colon, Lengthening vs Separator

Colon (`:`) has two meanings in CHAT:
- **Inside a word:** prosodic lengthening (`no::` = drawn out "no")
- **Between words:** separator (like comma, semicolon)

```
*CHI:   no:: .
        ^^^^           ← ONE word: word_segment("no") + lengthening("::")

*CHI:   hello , world .
              ^        ← separator(comma), space-delimited

*CHI:   hello : world .
              ^        ← separator(colon)
```

**How it works:** the ordinary `word_body` branch starts with `word_segment`,
`shortening` or `stress_marker`. A separate marker-initial branch permits
selected leading structural markers before one of those required elements.
Neither branch starts with bare lengthening or `+`. The separator and word
positions therefore have different admissible shapes; do not infer a universal
lexer winner from the colon character alone. In `no::`, the lexical segment
precedes the lengthening child.

**Why colon is excluded from `word_segment`:** If colon were part of
`word_segment`, `no::` would be one opaque word_segment and the grammar
couldn't distinguish lengthening colons from text colons. Excluding
colon makes `lengthening` a separate CST node.

## Why These Exceptions Exist

These cases reflect the distinction between visible notation and grammar
structure:

1. **Overlap markers** were originally meant to be visually aligned across
   speaker turns. Transcribers place them adjacent to text to show exactly
   where overlap begins/ends within a word. Space-separated markers indicate
   overlap at word boundaries.

2. **Zero prefix** was designed as a shorthand: `0word` reads as "the speaker
   omitted [word]." Standalone `0` means "action without speech." The
   whitespace distinction is intuitive for transcribers but requires careful
   DFA handling in the grammar.

### Exception 5: Plus (+), Compound Marker vs Terminator vs Linker

The `+` character has three meanings:
- **Inside a word:** compound marker (`ice+cream`)
- **At line end:** terminator prefix (`+...`, `+/.`, `+!?`, etc.)
- **At line start:** linker prefix (`+<`, `++`, `+^`, `+"`, `+,`)

```
*CHI:   ice+cream .
        ^^^^^^^^^      ← ONE word with compound marker

*CHI:   and then +...
                 ^^^^  ← terminator: trailing_off

+< but I was +/.
^^             ^^^     ← linker: lazy_overlap, terminator: interruption
```

**How it works:** the grammar distinguishes terminator and linker alternatives
from the compound marker inside `word_body`. The `+` in `ice+cream` is its own
child, not part of a longer `word_segment` token. Recognition alone does not
prove valid compound placement; typed-model validation remains relevant.

### Exception 6: Bracket Annotations vs Plain Brackets

Structured bracket annotations such as `[= text]` and `[=! text]` use
distinct prefix tokens, content and closing delimiters. Their prefix
precedence does not by itself establish correct attachment, recovery or
valid annotation content; those contracts belong to the enclosing rules and
typed-model checks.

The `[*]` error marker is the trickiest; it has optional content:
`[*]` (bare) vs `[* s:r]` (with code). This remains an opaque token.

## Stacked CA Markers

CA markers commonly stack, multiple structural markers appear before the
word text. This is standard Conversation Analysis notation:

```
*SPK:   °↑hello° .
        ^^^^^^^^           ← ONE word: ca_delimiter(°) + ca_element(↑) +
                              word_segment("hello") + ca_delimiter(°)
                              Meaning: soft voice + pitch up on "hello"

*SPK:   °°pianissimo°° .
        ^^^^^^^^^^^^^^     ← ONE word: ca_delimiter(°) + ca_delimiter(°) +
                              word_segment("pianissimo") + ca_delimiter(°) + ca_delimiter(°)
                              Meaning: very soft (double piano) on "pianissimo"
```

The `word_body` marker-initial path uses `repeat1` to accept any number
of leading structural markers before the first text element. Without this,
`°↑hello°` would produce an ERROR on the first `°` because the grammar
would require text immediately after one marker.

**Overlap markers also stack with CA markers:** `⌈°hello⌉°` is overlap
begin + soft voice around "hello".

## Summary Table

| Input | Tokens | Rule |
|-------|--------|------|
| `hello world` | `hello`, `world` | Whitespace delimits |
| `Yeah⌋⌈2 hey` | `Yeah⌋⌈2`, `hey` | Adjacent overlap = part of word |
| `Yeah ⌋ ⌈2 hey` | `Yeah`, `⌋`, `⌈2`, `hey` | Space-separated = overlap_point |
| `0die` | `standalone_word(zero, "die")` | Adjacent 0 = omission prefix |
| `0 die` | `nonword(zero)`, `die` | Space = action marker + word |
| `ice+cream` | `ice+cream` | Compound = one word |
| `&-uh` | `standalone_word(word_prefix("&-"), "uh")` | Filler prefix = one word |
| `hello@s:eng` | `standalone_word("hello", word_lang_suffix)` | Form/lang suffix = one word |
