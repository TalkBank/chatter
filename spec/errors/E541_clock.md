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
[[example]]
title = 'An empty start time is empty header content, not a start time'
level = 'header'
claim = { subsumed_by = 'E553' }
notes = 'Delete the complete value from the legal 23:59:59 control, retaining its tab separator and following speech. A header with nothing after its colon is Header::EmptyContent, refused by E553; no start-time value is built, so E541 has nothing to assess.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Time Start:	
*CHI:	hello .
@End
'''

[[example]]
title = 'A suffix after a complete start time is not discarded'
level = 'header'
claim = 'violates'
notes = 'Append x to the legal 23:59:59 control; retain the whole unsupported value.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Time Start:	23:59:59x
*CHI:	hello .
@End
'''
+++

## Description

Two- and three-component clock controls isolate numeric shape from clock
range. Each overflow changes one component while preserving the other zeros;
45 deletes the colon and other component rather than describing a clock.
The two-component form is minutes and seconds, not hours and minutes.
The empty-value and suffix mutations preserve the header and following speech;
neither an invented midnight value nor a truncated valid prefix is admissible.

## CHAT Rule

A start time has the declared MM:SS or HH:MM:SS shape and clock components:
hours 0–23, minutes and seconds 0–59. Numeric parsing alone does not certify
a valid clock. Invalid values must retain their original spelling for
reporting and serialization, while E541 rejects them.

## Notes

CHECK (21-Sep-2026) accepted the original three controls and rejected the
original six invalid values with code 35. That observation does not cover the
later empty-value and suffix mutations. Before the clock-range fix, Chatter rejected only bare
45; all five numeric overflows were silently accepted. The resulting E541
diagnostic consumes model-issued refusal evidence, not arbitrary text.
