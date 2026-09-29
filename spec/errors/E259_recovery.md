+++
code = 'E259'
name = 'Comma licensing evidence during tier recovery'

[[example]]
title = 'Spoken content still licenses a comma before an unrelated stray bracket'
level = 'utterance'
claim = 'legal'
notes = 'The spoken-word comma control retains its licensing word; insert only an unmatched closing bracket later in the tier. Legal concerns E259 only.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello , there ] .
@End
'''

[[example]]
title = 'Replacing the licensing word with an event retains the finding but refuses a repair'
level = 'utterance'
claim = 'violates'
notes = 'Replace only hello with &=laughs in the preceding control. Later spoken content cannot license the earlier comma; the recovered tier body cannot authorize a deletion.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	&=laughs , there ] .
@End
'''
+++

## Description

Typed content before the comma determines whether it is licensed, even when
another part of the tier requires recovery. A finding does not establish that
the recovered tier is safe to rewrite.

## CHAT Rule

A comma requires preceding comma-licensing content. Later speech does not
license it retroactively. Establish the intended bracket structure before
applying punctuation repairs to the recovered tier.
