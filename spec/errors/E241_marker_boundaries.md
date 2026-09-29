+++
code = 'E241'
name = 'Marker spelling and source boundaries'

[[example]]
title = 'Canonical markers with unchanged comment text'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	I said xxx yyy www xxx yyy www today .
%com:	quoted markers xx YY wW XXX YyY WWw remain unchanged
@End
'''

[[example]]
title = 'Shortened and miscased markers across all three categories'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	I said xx YY wW XXX YyY WWw today .
%com:	quoted markers xx YY wW XXX YyY WWw remain unchanged
@End
'''

[[example]]
title = 'Omission and shortening retain their structural notation'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	I said 0xx (yy) today .
%com:	quoted markers xx YY wW XXX YyY WWw remain unchanged
@End
'''

[[example]]
title = 'Sound material is not marker spelling'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	I said &~xx &=YY today .
%com:	quoted markers xx YY wW XXX YyY WWw remain unchanged
@End
'''

+++

## Description

Marker spelling applies to orthographic word material, not comment text or
sound material. The invalid spelling example samples shortened and miscased
forms in each category; its repair must equal the canonical control without
rewriting the ordinary dependent comment tier.

## CHAT Rule

Use the canonical untranscribed marker spelling. A whole-word replacement must
not discard omission or shortening notation: those cases retain E241 but are
not admitted by the simple marker-spelling repair. Marker classification has
one model owner; the repair does not define another vocabulary.
