+++
code = 'E762'
name = 'prefix marker stands alone or opens a word'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	the # dog .
@Comment:	ERROR: the prefix marker is not a word
@End
'''

[[example]]
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	the #dog .
@Comment:	ERROR: the marker attaches to the end of a prefix, not the start of a stem
@End
'''

[[example]]
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	heb
@Participants:	CHI Target_Child
@ID:	heb|corpus|CHI|||||Target_Child|||
*CHI:	ha# # kelev .
@Comment:	ERROR: position is illegal in every language, Hebrew included
@End
'''
+++

## Description

The prefix marker `#` separates a bound prefix from its stem in languages
whose orthography glues the two together, letting a transcriber keep the
morphology visible on the main tier. The marker attaches to the END of the
prefix, and the prefix is a word of its own:

```
*CHI:	ha# kelev .
```

Two shapes therefore cannot be that construct in any language: a word that is
nothing but the marker (`#`), and a word that opens with it (`#dog`). A bare
`#` is not a word at all, and a leading marker would attach a prefix to
nothing on its left.

This rule is about POSITION and is language-independent. Whether a word's
language uses the marker at all is a separate rule (`E763`); a word rejected
here is not additionally reported there, because "change the language" is not
the fix for either shape.

## Expected Behavior

- **Parser**: unaffected. The marker is ordinary word text; no grammar,
  serialization, or roundtrip behavior changes.
- **Validator**: reports E762 at the offending word. Runs without language
  context, deliberately: requiring a resolved language would let a file with
  no `@Languages` header carry these shapes silently.

## CHAT Rule

The prefix marker attaches to the end of the prefix it marks, and the prefix
is a separate main-tier word. A word consisting only of markers, or beginning
with one, is invalid regardless of language.

Wild-data impact at adoption (typed survey over every `#`-bearing corpus file,
2026-07-26): standalone 0, word-initial 0. Zero attestations, so no kept file
is affected. Legal positions are word-final (34,866 attestations) and, in
Hebrew only, word-internal (35,802).

Parity note: this subsumes CLAN CHECK 71 (a scoped code following `#` must
precede it) and the `#`-undeclared facet of CHECK 11, both of which concern
shapes this rule rejects outright.

The marker is legitimate notation in other places, and none of them is a
main-tier word: `%xpho` phonetic tiers, and free-text annotation content
inside `[= ]`, `[% ]`, and `[^ ]`. This rule covers main-tier words only.
