+++
code = 'E767'
name = 'whitespace before the comma in @Media'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Media:	SD02_recording , audio
*CHI:	hello .
@Comment:	ERROR: the filename ends where the comma begins
@End
'''

[[example]]
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Media:	SD02_recording  , video, missing
*CHI:	hello .
@Comment:	ERROR: the filename ends where the comma begins
@End
'''
+++

## Description

In `@Media` the comma separates the filename from the media type, and the
filename ends where the comma begins:

```
@Media:	SD02_recording, audio
```

Whitespace between the filename and that comma is rejected. Real CLAN CHECK
rejects it too (CHECK 148), and the house rule is that a style violation which
is unambiguous under our grammar still follows CHECK as an error.

The construct is unambiguous, so the grammar deliberately PARSES it rather than
failing, which is the only way to name the rule and point at the exact space.
That is a change of diagnostic, not of verdict: the file was already invalid.
What it produced before was fallout rather than a judgment, and misleading in
two ways at once. The `@Media` line failed to match, so the whole header fell
back to `Unknown`, raising E525 "unknown or unrecognized header type" about a
header chatter had recognised perfectly well, alongside E330 "Missing
media_type node" on a line that visibly ends in `, audio`. Neither message gave
a transcriber anything to act on.

The space is not part of the filename under any reading, so this rule reports it
and the parsed filename excludes it. A transcript fixed by deleting one space
then validates clean.

Related: the same 2026-08-05 change widened `media_filename` from an ASCII
allowlist to a delimiter-based token, so dots, parentheses, interior spaces and
non-ASCII characters are now legal in a media filename. This rule is the one
piece of the old behaviour deliberately kept as a rejection.

## Expected Behavior

- **Parser (tree-sitter)**: the grammar accepts the whitespace as
  `optional($.whitespaces)` before the comma, and the lowering records its span
  on `MediaHeader::whitespace_before_comma`. No diagnostic is emitted here.
- **Parser (re2c)**: the lexer keeps the whitespace as its own token, and the
  header converter records the same fact. It has no byte offsets to supply, so
  the span it records is the dummy one, which is a known gap in that front end
  rather than something specific to this rule.
- **Validator**: `check_media_whitespace_before_comma` reads the recorded span
  and reports E767. Putting the rule HERE rather than in a parser is what makes
  both front ends report it from one implementation. E767 was briefly emitted
  from the tree-sitter lowering, where the re2c parser could not reach it and
  silently disagreed; the equivalence oracle cannot catch that, because it
  compares parsed models and never the two parsers' diagnostics.

## CHAT Rule

`@Media` names a file and then, after a comma, its media type. The filename
therefore ends where the comma begins, and whitespace between the two belongs
to neither. Real CLAN rejects it (CHECK 148). The construct is unambiguous, so
it is a style violation rather than a parse failure, and this project treats an
unambiguous style violation that CLAN rejects as an error.
