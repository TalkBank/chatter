+++
code = 'E537'
name = 'Number vocabulary boundaries'

[[example]]
title = 'Declared value 1'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Number:	1
*CHI:	hello .
@End
'''

[[example]]
title = 'Declared value 2'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Number:	2
*CHI:	hello .
@End
'''

[[example]]
title = 'Declared value 3'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Number:	3
*CHI:	hello .
@End
'''

[[example]]
title = 'Declared value 4'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Number:	4
*CHI:	hello .
@End
'''

[[example]]
title = 'Declared value 5'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Number:	5
*CHI:	hello .
@End
'''

[[example]]
title = 'Declared value more'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Number:	more
*CHI:	hello .
@End
'''

[[example]]
title = 'Declared value audience'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Number:	audience
*CHI:	hello .
@End
'''

[[example]]
title = 'Near-miss value 0'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Number:	0
*CHI:	hello .
@End
'''

[[example]]
title = 'Near-miss value 6'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Number:	6
*CHI:	hello .
@End
'''

[[example]]
title = 'Near-miss value More'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Number:	More
*CHI:	hello .
@End
'''
+++

## Description

These complete-file controls enumerate the declared `@Number` vocabulary.
The near-miss variants change only that header's value, preserving the speaker,
ID and utterance. They distinguish supported tokens from similar-looking
spellings rather than relying only on an arbitrary unknown word.

## CHAT Rule

The vocabulary is the one specified in E537.md and declared in `depfile.cut`.
Values are exact tokens, not case-insensitive names or numeric quantities to
coerce. The invalid variants must report E537; their original text remains
available for diagnostics and serialization. CHECK observations are separate
from these authored claims.

## Notes

CHECK (21-Sep-2026) accepts every declared-value control. It reports code 11
for all three near-miss variants, agreeing with Chatter's refusals.
All examples parse without diagnostics and roundtrip byte-exactly; the
near-miss refusals occur during validation.
