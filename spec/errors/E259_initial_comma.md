+++
code = 'E259'
name = 'Initial comma has no preceding spoken content'

[[example]]
title = 'Spoken control'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello .
@End
'''

[[example]]
title = 'Initial comma before one separating space'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	, hello .
@End
'''

[[example]]
title = 'Initial comma before several separating spaces'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	,   hello .
@End
'''
+++

## Description

A tier-initial comma precedes all spoken content. These examples insert a
comma and its separator before the legal control's first word.

## CHAT Rule

A comma requires preceding comma-licensing content; speech later in the tier
does not license it. Deleting an initial comma is a semantic, user-reviewed
choice. A deletion proposal must consume its structural separator as well,
otherwise it introduces a leading-space error into the main tier.
