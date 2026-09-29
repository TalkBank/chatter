+++
code = 'E750'
name = 'Complete annotated-group edge whitespace'

[[example]]
title = 'Clean annotated group'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<dog> [/] dog .
@End
'''

[[example]]
title = 'Several spaces at both group edges'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<   dog   > [/] dog .
@End
'''

[[example]]
title = 'Nested groups with offending edge spaces'
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	< < dog > [!] cat > [/] dog .
@End
'''

[[example]]
title = 'Clean nested control'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<<dog> [!] cat> [/] dog .
@End
'''

+++

## Description

These controls and error variants exercise complete whitespace runs at both
edges of annotated groups, including nested groups. Each E750 finding owns
one edge run, not intervening word separators or annotation spacing.

## CHAT Rule

Angle delimiters hug their content. A repair may delete the complete offending
space token at the group's content boundary; it must preserve every spoken
word, nested delimiter, annotation and interior separator.
