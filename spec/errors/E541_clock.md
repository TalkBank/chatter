+++
code = 'E541'
name = 'Time Start clock boundaries'

[[example]]
title = 'Clock control 00:00'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Time Start:	00:00
*CHI:	hello .
@End
'''

[[example]]
title = 'Clock control 59:59'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Time Start:	59:59
*CHI:	hello .
@End
'''

[[example]]
title = 'Clock control 23:59:59'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Time Start:	23:59:59
*CHI:	hello .
@End
'''

[[example]]
title = 'Invalid clock 25:00:00'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Time Start:	25:00:00
*CHI:	hello .
@End
'''

[[example]]
title = 'Invalid clock 00:60:00'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Time Start:	00:60:00
*CHI:	hello .
@End
'''

[[example]]
title = 'Invalid clock 00:00:60'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Time Start:	00:00:60
*CHI:	hello .
@End
'''

[[example]]
title = 'Invalid clock 60:00'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Time Start:	60:00
*CHI:	hello .
@End
'''

[[example]]
title = 'Invalid clock 00:60'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Time Start:	00:60
*CHI:	hello .
@End
'''

[[example]]
title = 'Invalid clock 45'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Time Start:	45
*CHI:	hello .
@End
'''
+++

## Description

Two- and three-component clock controls isolate numeric shape from clock
range. Each overflow changes one component while preserving the other zeros;
45 deletes the colon and other component rather than describing a clock.
The two-component form is minutes and seconds, not hours and minutes.

## CHAT Rule

A start time has the declared MM:SS or HH:MM:SS shape and clock components:
hours 0–23, minutes and seconds 0–59. Numeric parsing alone does not certify
a valid clock. Invalid values must retain their original spelling for
reporting and serialization, while E541 rejects them.

## Notes

CHECK (21-Sep-2026) accepts all three controls and rejects all six invalid
values with code 35. Before the clock-range fix, Chatter rejected only bare
45; all five numeric overflows were silently accepted. The resulting E541
diagnostic consumes model-issued refusal evidence, not arbitrary text.
