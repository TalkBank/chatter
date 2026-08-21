+++
code = 'E375'
name = 'Replacement [: ...] glued to a word without a preceding space'
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
*CHI:	word[: foo] .
@End
'''
+++

## Description

A replacement annotation `[: text]` must be preceded by whitespace, exactly
like every other bracketed annotation (the scope codes `[?]`, `[!]`, `[/]`,
`[//]`, etc., which `base_annotations` already requires a space before). A
replacement written with no space, glued directly to the word it replaces
(`word[: foo]`), is invalid CHAT.

The grammar enforces this: `word_with_optional_annotations` requires
`$.whitespaces` before the `replacement`, so a glued replacement produces an
ERROR node and the parser reports E375. The spaced form `word [: foo]` parses
and validates cleanly.

This corresponds to CLAN CHECK error 161 ("Space character is required before
'[' code item"). Before this rule the grammar made the whitespace optional only
for the replacement branch (an inconsistency with every sibling annotation),
silently accepting the glued form; this spec locks in the rejection so the
parser cannot regress to accepting it.
