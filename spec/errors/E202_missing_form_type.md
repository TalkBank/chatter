+++
code = 'E202'
name = 'Missing form type after @'

[[example]]
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello@ .
@End
'''

[[example]]
level = 'word'
claim = { subsumed_by = 'E203' }
notes = '''
Note: `@j` is recognized as a form type syntactically, but `j` is not a valid
form type value. This triggers E203 (InvalidFormType) rather than E202
(MissingFormType).
'''
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	dog@j .
@End
'''

[[example]]
level = 'word'
title = 'a doubled @ is ONE defect, named by E203'
claim = { subsumed_by = 'E203' }
notes = '''
`word@@` ends with `@` AND carries a repeated `@` run, so it satisfies both
this file's dangling-marker rule and E203's at-most-one-suffix rule. The parser
names the repeated run as E203 with the run in hand, which is the specific
answer; adding E202 for the same word reports one defect twice, the generic
diagnostic buried under the specific one.

The suppression that stops the double already existed for E203-against-E203
and was never applied to the E202 branch four lines above it.
'''
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	word@@ .
@End
'''
[[example]]
level = 'word'
claim = 'violates'
title = 'the same dangling marker on another word (re-filed from E243_auto.md, whose example never demonstrated E243)'
notes = '''
Note: The example `hell@` triggers E202 (MissingFormType) rather than E243
(IllegalCharactersInWord) because the parser detects the bare `@` as a missing
form type marker. E243 fires at the validation layer on parsed words containing
whitespace, control characters, or bullet markers, conditions that are
difficult to reach via normal CHAT input.
'''
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: @ character only allowed for form markers
@Comment:	Invalid: 'hell@' - @ in wrong position
*CHI:	hell@ .
@End
'''
+++

## Description

A word contains `@` at a position where a form type marker is expected, but
no valid form type follows. Tree-sitter produces an ERROR node at the `@`.

The valid form types are declared in
`spec/form_markers/form_marker_registry.json` and rendered in the book's
[symbols chapter](../../book/src/chat-format/symbols.md). `@s` is the language
marker, a separate construct that is not a form type.

There used to be a copy of the list here, and it had drifted: it omitted `@u`.

## Expected Behavior

The parser should report E202 and recover by treating the word as malformed.
The raw text is preserved for downstream tools that may handle it differently.
