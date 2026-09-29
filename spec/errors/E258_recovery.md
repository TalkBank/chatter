+++
code = 'E258'
name = 'Consecutive commas in a recovered main tier'

[[example]]
title = 'A single comma does not become duplicate punctuation during recovery'
level = 'utterance'
claim = 'legal'
notes = 'Insert only an unmatched closing bracket into the single-comma E258 control. Legal asserts only absence of E258; grammar recovery must still be reported.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello , world ] .
@End
'''

[[example]]
title = 'A second comma is still invalid but its recovered carrier cannot be repaired'
level = 'utterance'
claim = 'violates'
notes = 'Duplicate only the comma in the preceding control. Typed separator evidence establishes E258, while the catalog refuses deletion from a tier body requiring grammar recovery.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello ,, world ] .
@End
'''
+++

## Description

Consecutive typed comma separators remain invalid when another part of their
main tier requires recovery. A diagnostic does not by itself admit its carrier
for a mechanical fix.

## CHAT Rule

Write a single syntactic-break comma and balanced main-tier syntax. Establish
the intended bracket structure before requesting automatic punctuation repairs.
