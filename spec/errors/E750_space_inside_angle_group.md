+++
code = 'E750'
name = 'Space inside angle-bracket group delimiters'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	< dog> [/] dog .
@Comment:	ERROR: space directly after the opening angle bracket
@End
'''

[[example]]
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<dog > [/] dog .
@Comment:	ERROR: space directly before the closing angle bracket
@End
'''
+++

## Description

A space directly after the opening `<` or directly before the closing
`>` of an angle-bracket group (`< dog>` or `<dog >`) is invalid (CLAN
CHECK error 160, "Space character is not allowed after '<' or before
'>' character.", check.cpp 4300/4306). The grammar tolerates the
whitespace as an explicit optional `whitespaces` CST node so the parse
recovers, but the construct is invalid CHAT; before this rule the
parser silently DROPPED that whitespace, so accepted files were also
being silently rewritten on normalize.

## Expected Behavior

- **Parser**: Reports E750 at each offending `whitespaces` node (a
  file with both spaces gets two diagnostics) and keeps parsing the
  group (recovery preserved).
- **Validator**: No separate validation-layer check; the parser drops
  the whitespace from the model, so the parse is the only layer that
  sees it.

## CHAT Rule

Group delimiters hug their content. Parity entry:
`crates/talkbank-parser-tests/tests/check_parity/manifest.json`
CHECK 160.
