+++
code = 'E353'
name = 'MissingOtherCompletionContext'
kind = 'Invalidity'
status = 'not_implemented'

[[example]]
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||male|||Target_Child|||
*CHI:	++ hello .
@End
'''
+++

## Description

An other-completion linker (`++`) was used but it is the very first
utterance in the file. The `++` linker requires a preceding utterance
(from a different speaker) to complete.

## Expected Behavior

Validation should report E353. The `++` linker signals other-completion
(completing a different speaker's utterance), but there is no preceding
utterance at all; this is the first utterance in the file.

## CHAT Rule

The `++` linker pairs with `+...` (trailing off). A preceding utterance
from a different speaker must have trailed off for another speaker to
complete it.
