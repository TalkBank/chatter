+++
code = 'E305'
name = 'Recovery does not prove a missing terminator'

[[example]]
title = 'A terminated tier with an unmatched bracket requires structural recovery'
level = 'utterance'
claim = { subsumed_by = 'E316' }
notes = 'Insert only an unmatched closing bracket into the basic-terminator control in E305.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello world ] .
@End
'''

[[example]]
title = 'Removing the terminator does not make the recovered tier complete evidence'
level = 'utterance'
claim = { subsumed_by = 'E316' }
notes = 'Delete only the final period of the preceding control. The existing policy suppresses E305 on parse-tainted main tiers because recovery may conceal a terminator.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello world ]
@End
'''
+++

## Description

A recovered main tier cannot establish that all its terminators are known.
Structural rejection remains actionable without guessing an additional
missing-terminator diagnostic or offering an insertion based on incomplete
content.

## CHAT Rule

Repair the structural fault first. Outside Conversation Analysis mode, the
resulting complete main tier must end with an appropriate utterance terminator.
