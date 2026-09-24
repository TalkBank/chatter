+++
code = 'E375'
name = 'Retired repetition counts without a terminator'

[[example]]
title = 'Explicit spoken repetitions with a terminator'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hey [/] hey .
@End
'''

[[example]]
title = 'Retired count and omitted terminator'
level = 'utterance'
claim = 'violates'
notes = 'This deliberately combines retired repetition notation with a missing terminator; it is not claimed to be a single-fault mutation.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hey [x 2]
@End
'''

[[example]]
title = 'Retired count without spoken material or a terminator'
level = 'utterance'
claim = 'violates'
notes = 'Delete the remaining spoken word from example 2; the count alone does not establish spoken content.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	[x 2]
@End
'''
+++

## Description

Missing utterance structure does not make retired repetition-count notation
valid. These malformed combinations complement the terminated `[x N]` cases
in `E375.md`. The control writes the spoken repetitions explicitly.

## CHAT Rule

The CHAT manual retired `[x N]` in September 2022 in favor of explicit
repetition with `[/]`; see its
[Disfluency Transcription section](https://talkbank.org/0info/manuals/CHAT.html).
An utterance also requires a terminator. These examples deliberately retain
both faults to exercise recovery, not to redefine either rule.

## Expected Behavior

The invalid examples report E375 for retired repetition notation. Other
missing-structure diagnostics may coexist; recovered output is not a valid
replacement for the original transcript.
