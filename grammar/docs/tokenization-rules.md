# CHAT Tokenization Rules

**Status:** Current
**Last updated:** 2026-07-11 12:49 EDT

This document explains how CHAT text is split into tokens (words, markers,
annotations). These rules are the most confusing aspect of CHAT and the
most common source of parser bugs.

## The Fundamental Rule

**Whitespace delimits content items.** Contiguous non-whitespace characters
form a single token. This applies everywhere: main tiers, dependent tiers,
headers.

## The Word Purity Invariant

**`word_segment` contains ONLY pure spoken text.** Within a word, structural
markers (overlap, CA, underline, stress, lengthening) are separate typed
children in `word_body`, never consumed by `word_segment`. This means:

- `cleaned_text()` never needs to scan for and strip markers
- Validation can find ALL overlap/CA markers by walking `WordContent` variants
- Editors get typed CST nodes for every structural element

The exclusions are built from the symbol registry (`src/generated_symbol_sets.js`)
a single source of truth shared by the grammar and Rust code.

See [Precedence Decisions](precedence-decisions.md#word-segment-purity-invariant)
for the full exclusion table.

## The Exceptions

### Exception 1: Overlap Markers (⌈⌉⌊⌋)

Overlap markers indicate simultaneous speech between speakers. Custody
follows the **whitespace-boundary rule** (2026-07-11): a marker glued
to word material stays in the word (as part of a glued run); a marker
whose outer side touches whitespace is top-level content:

```
*CHI:   Yeah⌋⌈2 hey .
        ^^^^^^^^       ← ONE word: "Yeah⌋⌈2" (glued trailing run)

*CHI:   Yeah ⌋ ⌈2 hey .
        ^^^^           ← word: "Yeah"
             ^^^       ← overlap_point: ⌋
               ^^^^    ← overlap_point: ⌈2
                   ^^^ ← word: "hey"

*CHI:   Yeah ⌋⌈2 hey .
        ^^^^           ← word: "Yeah"
             ^^^^^     ← TWO top-level overlap_points: ⌋ then ⌈2
                         (glued pair, but no spoken-text host)
                  ^^^  ← word: "hey"

*CHI:   ⌈one two⌉ .
        ^^^            ← overlap_point: ⌈ (top-level, edge)
         ^^^           ← word: "one"
                       ← word: "two"; trailing ⌉ top-level
```

**Key mechanics:** in-word custody is carried by the weighted hidden
rules `_interior_overlap` / `_final_overlap_cluster` /
`_final_overlap_form` (`prec.dynamic(2)` inside their rule bodies),
racing against the fragmented readings under declared GLR conflicts.
See `book/src/architecture/overlap-binding.md` for the full design
record and the dynamic-precedence attachment traps.

**`cleaned_text()` does NOT strip in-word overlap markers.** They are
part of the raw word content. Stripping for NLP is a downstream
concern.

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
  because tree-sitter's precedence doesn't propagate through intermediate rules

**See also:** [Precedence Decisions](precedence-decisions.md) for the full
proof and minimal grammar experiment.

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
              ^        ← SHOULD be separator(colon)
                         ACTUALLY parsed as standalone_word(lengthening)
```

**How it works:** `word_body` requires starting with `word_segment`,
`shortening`, or `stress_marker`. `lengthening` and `+` cannot be the
first element. So standalone `:` cannot form a valid `word_body`, and
falls through to `separator(colon)`. But `no::` works because
`word_segment("no")` starts the word body, then `lengthening("::")`
follows.

**Why colon is excluded from `word_segment`:** If colon were part of
`word_segment`, `no::` would be one opaque word_segment and the grammar
couldn't distinguish lengthening colons from text colons. Excluding
colon makes `lengthening` a separate CST node.

## Why These Exceptions Exist

Both exceptions exist because CHAT was designed for human readability, not
machine parsing:

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

**How it works:** All terminator and linker tokens use `prec(10)`, which
beats `word_segment` at `prec(5)`. So `+...` is always a terminator, never
part of a word. The compound `+` inside `ice+cream` works because it's
part of the longer `word_segment` match (maximal munch at prec 5).

**No valid CHAT word ends with `+`.** The `standalone_word` grammar enforces
this, the word body's last character cannot be `+`. This ensures `to+...`
is tokenized as word `to` + terminator `+...`, not word `to+` + `...`.

### Exception 6: Bracket Annotations vs Plain Brackets

Bracket annotations (`[= text]`, `[=! text]`, `[% text]`, etc.) use
`prec(8)` prefix tokens to beat generic bracket handling. Without this,
`[= explanation]` would be parsed as `[` + content + `]`.

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
