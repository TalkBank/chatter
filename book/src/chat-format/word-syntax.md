# Word Syntax

**Status:** Reference
**Last updated:** 2026-05-11 23:33 EDT

Words are the primary content unit on the main tier. CHAT defines several word types and annotation mechanisms.

## Standalone Words

Most words are simple tokens separated by whitespace:

```chat
*CHI:	I want a cookie .
```

Words can contain Unicode characters for any language:

```chat
*CHI:	ich möchte Kekse .
```

## Compounds

Compound words join multiple elements with `+`:

```chat
*CHI:	I want ice+cream .
```

## Special Word Forms

### Shortened Forms

Parentheses mark omitted portions of a word:

```chat
*CHI:	(be)cause I want it .
```

The full form is `because`; the child produced `cause`.

### Replacements

Square brackets with colon mark what the speaker actually meant:

```chat
*CHI:	I goed [: went] to the store .
```

The speaker said "goed" but the intended word was "went".

### Language Markers

The `@s:` suffix marks a word's language in multilingual transcripts:

```chat
*CHI:	I want a Keks@s:deu .
```

When a whole stretch switches language, annotate the group rather than
suffixing every word:

```chat
*CHI:	ik weet niet <how to do it> [@s] .
*TEA:	us samay <kyaa hotaa hai> [@s:hin] .
```

`[@s:code]` names the language; bare `[@s]` resolves the way a bare `word@s`
does. Every word in the `<>` scope takes that language, exactly as if each
carried the suffix. As with any scoped annotation, a single item needs no
angle brackets: `hallo [@s]` is well-formed and means what `hallo@s` means.

A word inside the span may carry its OWN marker, and the word wins:

```chat
*TEA:	<rocket@s:eng jaise jaataa hai> [@s:hin] .
```

That is not redundancy to avoid. It is how a borrowed word is marked inside a
switched clause, and it is what transcribers actually write: the span carries
the matrix language of the stretch, the suffix carries the donor language of
one item. Resolution is innermost-first, and each layer is recorded with its
own provenance, so a consumer can tell which mark decided a given word.

A word can also carry one special-form marker naming what kind of form it is
(`gumma@c` for a child-invented word, `b@l` for a letter). The complete set,
with meanings and examples, is the table in
[Symbols](symbols.md#-markers-word-level).

There used to be a hand-picked subset of that table here, and it had already
drifted: it glossed `@si` as "signed word", which is `@sl`. `@si` is singing.
A partial copy of a closed set is worth less than a link to the whole one.

## Annotations

Words and groups can carry post-positioned annotations in square brackets:

### Error Marking

```chat
*CHI:	he goed [*] to school .
```

`[*]` marks an error. More specific error codes can follow: `[* m:+ed]`.

### Explanations

```chat
*CHI:	that one [= the red ball] .
```

`[=  text]` provides an explanation or gloss.

### Replacements

```chat
*CHI:	I wanna [: want to] go .
```

`[: text]` marks the target/intended form.

### Best Guess

```chat
*CHI:	I want the birfer [?] .
```

`[?]` marks uncertain transcription.

## Events and Actions

### Paralinguistic Events

Events marked with `&=` describe non-speech sounds:

```chat
*CHI:	&=laughs I want cookie .
*CHI:	&=coughs .
```

### Fillers

Fillers are marked with `&-`:

```chat
*CHI:	&-um I want &-uh cookie .
```

### Interposed Speech (Other Speaker)

Brief background speech from a different speaker is marked with the
`&*SPK:text` prefix, it captures the interjection without creating
a full turn line:

```chat
*CHI:	I want &*MOT:careful a cookie .
```

This says CHI was speaking and MOT briefly said "careful" mid-turn.
If the intervention is substantial enough to constitute its own turn,
transcribe it as a separate `*MOT:` utterance instead. Model:
`crates/talkbank-model/src/model/content/other_spoken.rs`.

(Note: `[^ text]` is a *freecode*, a standalone free-form
researcher annotation that sits as its own content item on the main
tier (variant of `UtteranceContent::Freecode`, sibling of `Word` and
`Group`; it is NOT attached to any word). See `grammar/grammar.js`
rule `freecode` and
`crates/talkbank-model/src/model/content/utterance_content/`. Used
for transcriber notes that are independent of any single word; for
notes about a single word use `[% text]` or `[= text]` instead.)

## Pauses

```chat
*CHI:	I (.) want (..) a (...) cookie .
*CHI:	I (1.5) want a cookie .
```

- `(.)`: short pause
- `(..)`: medium pause
- `(...)`: long pause
- `(N.N)`: timed pause in seconds

## Overlap

Overlapping speech between speakers uses angle brackets and overlap markers:

```chat
*MOT:	do you want <a cookie> [>] ?
*CHI:	<cookie> [<] !
```

- `[>]`: follows the overlap (this speaker started first)
- `[<]`: overlaps the previous speaker

## Retrace and Repetition

Groups followed by retrace markers indicate speech disfluencies:

```chat
*CHI:	<I want> [/] I want a cookie .
*CHI:	<I want> [//] I need a cookie .
*CHI:	<I want a> [///] give me a cookie .
```

- `[/]`: partial retrace (speaker repeats the same words)
- `[//]`: full retrace (speaker restarts with different words)
- `[///]`: multiple retracing (multiple false starts)
- `[/-]`: reformulation (speaker rephrases with different structure)
