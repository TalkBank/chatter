+++
code = 'E306'
name = 'Separator-only turns and dependent-tier ownership'

[[example]]
title = 'Lexical control with an ordinary dependent tier'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello .
*CHI:	voice .
%com:	this comment belongs to the second turn
@End
'''

[[example]]
title = 'A comma alone does not supply utterance content'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello .
*CHI:	, .
@End
'''

[[example]]
title = 'Dependent text does not supply missing main-tier content'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello .
*CHI:	, .
%com:	this comment belongs to the second turn
@End
'''
+++

## Description

A separator-only main tier has no meaningful utterance content. Dependent
tiers belong to that turn; their text does not fill its missing speech.

## Expected Behavior

Report E306 for the separator-only main tier. A proposed deletion must not
silently reattach its dependent tiers to the preceding turn, nor discard them.
An ordinary `%com` tier has no special semantics in this rule: ownership is
the same for every dependent tier.

## CHAT Rule

Supply the missing main-tier content from evidence. Removing an empty turn
is a semantic choice, not an automatic mechanical repair. If dependent tiers
exist, resolving their content and ownership requires a separate decision.
