# Validation Errors

**Status:** Current
**Last modified:** 2026-07-16 11:14 EDT

The CHAT validator produces diagnostics at two severity levels: **errors** (must fix) and **warnings** (should fix). Each diagnostic has an error code that maps back to a documented spec and validator rule.

`chatter validate` is the binding judgment on whether a byte sequence is valid CHAT. When it reports an **error**, the file is invalid CHAT: clean the data rather than working around the check. A **warning** flags a questionable but parseable construct you should review. Where `chatter` and an older tool such as CLAN's `check` disagree on whether a file is valid, `chatter validate` is authoritative (see [CHECK Parity Audit](../../architecture/errors-and-validation/check-parity-audit.md) for how the two are reconciled).

## Reading Error Output

The validator emits rich diagnostics that include the error code, a
source-pointed snippet, and a suggested fix:

```text
  × error[E304]: Missing speaker in main tier (line 15, column 3)

15 │ *	hello world .
   ·  ╰── here
   ╰────
  help: Add a speaker code between * and : (e.g., *CHI:)
```

Each diagnostic contains:
- **File path** and **location** (line:column)
- **Severity**: `error` or `warning`
- **Error code**: `E` prefix for errors, `W` prefix for warnings, with
  a URL pointing at the per-code documentation page
- **Message**: human-readable description
- **Suggestion**: actionable fix guidance where available

## Error Code Ranges

| Range | Category | Examples |
|-------|----------|----------|
| E1xx | UTF-8 and encoding | E101: Invalid line format |
| E2xx | Word-level content | E202: Missing form type after `@`, E203: Invalid form type marker, E207: Unknown annotation |
| E3xx | Main tier (speakers, terminators, content) | E301: Empty/missing main tier, E304: Missing speaker, E305: Missing terminator, E306: Empty utterance, E307: Invalid speaker, E308: Undeclared speaker |
| E4xx | Dependent tier structure | E401: Duplicate dependent tier |
| E5xx | Headers | E501: Duplicate header, E504: Missing @Participants, E505: Invalid @ID format |
| E6xx | Dependent tier validation | E601: Invalid dependent tier, E604: %gra without %mor |
| E7xx | Alignment, Phon tiers, structure | E705: Main/%mor count mismatch, E721: %gra index error, E747: Blank line, E748: Leading zero in bullet time, E749: Comma glued to next word, E750: Space inside angle group, E751: Pause glued to word, E752: Timing bullets without @Media, E753: Word only repetition segments, E754: Multi-letter @l form, E755: Undeclared utterance language, E756: Empty user-defined tier, E757: Code glued to following word, E758: Leading space on tier (non-CA), E759: Annotation at utterance start, E760: %mor item with empty POS |
| W1xx-W6xx | Warnings | W108: Speaker not found in @Participants (non-fatal contexts) |

## Common Errors and Fixes

### E256: Curly single quote used as a word character

A curly single quotation mark (`U+2018` or `U+2019`), commonly introduced by
autocorrect or speech-to-text, is not a legal CHAT word character. CHAT words
use the ASCII apostrophe (`U+0027`, the plain `'`). For example, a contraction
typed as `don` + `U+2019` + `t` is rejected; write `don't` with the ASCII
apostrophe instead. `chatter` flags the curly form wherever it appears in word
content and points the diagnostic at the exact character. This mirrors CLAN
CHECK errors 138 and 139.

### E243: Private-use or non-standard Unicode in a word

A word may contain only standard Unicode. Characters from the Unicode Private
Use Area and the other non-standard code points in the `U+E000`-`U+FFFF` block
are rejected, including the replacement character `U+FFFD` that marks a botched
text encoding. The most common cause is a file saved in the wrong encoding:
re-save it as UTF-8 and replace any private-use or compatibility-area character
with its standard Unicode equivalent. `chatter` points the diagnostic at the
exact character. This mirrors CLAN CHECK error 86.

### E304: Missing speaker code

A main tier line must have a speaker code after the `*`:

```text
*CHI:	hello world .
```

An empty speaker code (`*:	hello .`) triggers E304.

### E308: Undeclared speaker

Every `*SPEAKER:` code must be listed in `@Participants`. Add the missing speaker to the header:

```text
@Participants:	CHI Target_Child, MOT Mother
```

### E370: Retrace marker with nothing to retrace

A retrace or repetition marker (`[/]`, `[//]`, `[///]`) must be followed by the
repeated or corrected material; per the CHAT manual the marker always refers to
the text that follows it. A marker followed only by a terminator has nothing to
retrace:

```text
*CHI:	<the> [/] .          ← invalid: [/] is not followed by repeated material
*CHI:	<the> [/] the cat .  ← valid: the repeated material follows the marker
```

This mirrors CLAN CHECK error 119 (and the related retrace checks 151 and 159).

### E505: Invalid @ID format

Check that pipe-separated fields are correct and the speaker code matches `@Participants`:

```text
@ID:	eng|corpus|CHI|2;6.||||Target_Child|||
```

### E705: Main/%mor alignment mismatch

The number of `%mor` items must match the number of alignable words on the main tier. Retraces, pauses, and events are not counted. The validator shows a columnar diff:

```text
  Main tier       %mor tier
  ──────────────  ──────────────
  I               pro|I
  want            v|want
  to              inf|to
  go              v|go
  home, ⊖
```

### E714 / E715: `%pho`, `%mod`, or `%wor` count mismatch

The same two codes are reused for "too few" / "too many" count mismatches on
`%pho`, `%mod`, and `%wor`.

For `%wor`, the main-tier side is a spoken-token inventory:

- regular words and fillers count
- fragments, nonwords, and `xxx`/`yyy`/`www` count
- retrace does not change `%wor` membership
- replacements keep the original spoken surface word for `%wor`

That context-sensitivity decides **membership**, not leniency. Once an item is
in the `%wor` set, alignment is still **strict 1:1**. So if a filler like
`&-mm` counts on the main tier and `%wor` omits it, E714 is the correct result.

So this is valid:

```chat
*CHI:	<one &+ss> [/] one play ground .
%wor:	one •321008_321148• ss •321148_321368• one •321809_321969• play •322049_322310• ground •322390_322890• .
```

But this is also valid:

```chat
*EXP:	&+ih <the what> [/] what's letter &+th is this ?
%wor:	ih •49063_49103• the •49103_49163• what •49183_50205• what's •50205_50405• letter •50405_50685• th •50886_50946• is •50946_51046• this •51086_51586• ?
```

And this is valid too:

```chat
*EXP:	what's is dis [: this] ?
%wor:	what's •37050_37471• is •37491_37631• dis •37631_38131• ?
```

### E721: %gra sequential index error

`%gra` entries must have sequential 1-based indices: `1|...|... 2|...|... 3|...|...`

### E748: Leading zero in bullet timestamp

A media bullet time component is written with a leading zero before
another digit, for example `\u{15}012_200\u{15}`. Bullet times are
plain millisecond integers; write `12`, not `012`. A bare `0` (as in
`0_200`) is legal. This mirrors CLAN CHECK error 90 ("Illegal time
representation inside a bullet."). The bullet's numeric value still
parses, so downstream tooling sees the intended times; the diagnostic
alone makes the file invalid.

### E749: Comma glued to the following word

A comma on a speaker tier must be followed by a space or end-of-line:
write `hey , you`, not `hey ,you`. Mirrors CLAN CHECK error 92. The
check looks at the word immediately after the comma in document order
(including inside `<...>` groups); constructs that place their own
character after the comma (group and overlap marks, CA symbols) are
not flagged.

### E750: Space inside angle-bracket group delimiters

Group delimiters hug their content: write `<dog> [/]`, never `< dog>`
or `<dog >`. Mirrors CLAN CHECK error 160. Each offending space gets
its own diagnostic; the group still parses, so downstream tooling sees
the intended structure.

### E751: Pause glued to the preceding word

A pause marker must be space-delimited from the word before it: write
`hello (.) there`, not `hello(.) there`. Mirrors CLAN CHECK error 57.

### E752: Timing bullets without an @Media header

The transcript carries timing evidence (utterance bullets or %wor word
timing) but no `@Media` header declares the recording those timestamps
index. Add an `@Media` header naming the media file (or remove the
timing bullets if the transcript is genuinely unlinked). Completes the
media-consistency family: E544 covers declared linkage without timing,
E552 covers a declared `unlinked` contradicted by timing. Mirrors CLAN
CHECK error 112.

### E753: Word consisting only of a repetition segment

A word whose entire material sits inside segment-repetition delimiters
(`↫hi↫` with nothing outside the arrows) marks the repetition of a word
that is not there; attach the repeated segment to its host word
(`↫p↫parents`) or transcribe a stand-alone fragment as a filler or
nonword form. Filler and other word-category prefixes (`&-`, `&~`, `0`)
count as material outside the arrows. Adopted from GUI CLAN CHECK error
151 as a chatter rule.

### E754: Letter form @l with more than one letter

The `@l` form marks a single spoken letter (`b@l`); use `@k` (letter
sequence) or `@ls` (letter plural) for multi-letter content. Stuttered
letters with repetition segments (`↫b^↫b@l`) are fine: repeated-segment
material does not count toward the stem. Mirrors CLAN CHECK error 76.

### E519 at word level: language codes must be real everywhere

The ISO 639-3 registry check that guards `@Languages` and `@ID` also
applies to explicit word-level switch codes (`word@s:CODE`, including
`+`/`&` multi-code forms) and to `@L1 of` values: the code needs no
declaration, but it must name a real language. Utterance-level `[- CODE]` precodes are covered
by E755 plus the header check.

### E755: Utterance language not declared in @Languages

A `[- CODE]` precode marks a whole utterance as being in another
language, which is substantial presence: declare that language in
`@Languages`. Deliberate contrast: a word-level `@s:CODE` insertion
needs NO declaration (`ok@s:eng` in a Cantonese transcript is valid
as-is), because `@Languages` lists the transcript's substantial
languages, not every language that appears. Mirrors CLAN CHECK error
152.

### E756: Empty user-defined tier

A user-defined `%x` tier with empty or whitespace-only content declares
an annotation that is not there; add the content or remove the line.
(Formerly W601; renumbered because it always was a hard error.)

### E757: Bracketed code glued to the following word

A bracketed code's closing `]` must be space-delimited from what
follows: write `hello [/] x`, not `hello [/]x`. The parse is
unambiguous either way, which is exactly why this is a style rule: the
corpus stays canonically spaced. Mirrors CLAN CHECK error 19.

### E758: Leading space before tier content in a non-CA file

A space between the tier's tab delimiter and the first content item
(`*CHI:<tab><space>dog .`) is invalid unless the file declares
`@Options: CA`; CA transcripts legitimately column-align content with
spaces after the tab. Mirrors CLAN CHECK error 123.

### E759: Annotation at utterance start

Postfix annotations (retraces `[/]` `[//]`, overlap markers `[<]`
`[>]`, replacements `[: text]`, the quotation code `["]`) scope over
the material BEFORE them; an utterance whose content begins with one
has nothing for the code to attach to. Mirrors CLAN CHECK error 52.

### E760: %mor item with an empty part-of-speech field

A `%mor` item beginning with the `|` separator (`|we`) declares no
part of speech; every item is `pos|stem` with a non-empty POS. The
modern reading of CLAN CHECK error 11 (the depfile mechanism is
legacy; the non-empty-symbol invariant is real).

### E243 addition: the pipe character

`|` is the %mor tier's delimiter and has no meaning in main-tier word
text; a bare or embedded pipe in a word now reports E243
(IllegalCharactersInWord). Covers the grounded shape of CLAN CHECK
error 48.

## Generated Error Documentation

The source of truth for error-code details is `spec/errors/`. Maintainers can
also regenerate a local error-reference set from those specs when working on
diagnostics:

```bash
cargo run --manifest-path spec/tools/Cargo.toml --bin gen_error_docs
```

That generated reference includes the error description, example inputs, suggested fixes, and the layer that catches the diagnostic.
