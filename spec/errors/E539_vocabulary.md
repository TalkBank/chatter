+++
code = 'E539'
name = 'Transcription vocabulary boundaries'

[[example]]
title = 'Declared value eye_dialect'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Transcription:	eye_dialect
*CHI:	hello .
@End
'''

[[example]]
title = 'Declared value partial'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Transcription:	partial
*CHI:	hello .
@End
'''

[[example]]
title = 'Declared value full'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Transcription:	full
*CHI:	hello .
@End
'''

[[example]]
title = 'Declared value detailed'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Transcription:	detailed
*CHI:	hello .
@End
'''

[[example]]
title = 'Declared value coarse'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Transcription:	coarse
*CHI:	hello .
@End
'''

[[example]]
title = 'Declared value checked'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Transcription:	checked
*CHI:	hello .
@End
'''

[[example]]
title = 'Declared value anonymized'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Transcription:	anonymized
*CHI:	hello .
@End
'''

[[example]]
title = 'Near-miss value Full'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Transcription:	Full
*CHI:	hello .
@End
'''

[[example]]
title = 'Near-miss value eye-dialect'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Transcription:	eye-dialect
*CHI:	hello .
@End
'''

[[example]]
title = 'Near-miss value checked_extra'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Transcription:	checked_extra
*CHI:	hello .
@End
'''
+++

## Description

These complete-file controls enumerate the declared `@Transcription` vocabulary.
The near-miss variants change only that header's value, preserving the speaker,
ID and utterance. They distinguish supported tokens from similar-looking
spellings rather than relying only on an arbitrary unknown word.

## CHAT Rule

The vocabulary is the one specified in E539.md and declared in `depfile.cut`.
Values are exact tokens, not case-insensitive names or numeric quantities to
coerce. The invalid variants must report E539; their original text remains
available for diagnostics and serialization. CHECK observations are separate
from these authored claims.

## Notes

CHECK (21-Sep-2026) accepts every declared-value control. It reports code 11
for `Full` and `checked_extra`, but accepts `eye-dialect`. Chatter's existing
exact-token rule rejects all three; the hyphen variant is a documented
behavior difference, not a parity gain.
All examples parse without diagnostics and roundtrip byte-exactly; the
near-miss refusals occur during validation.
