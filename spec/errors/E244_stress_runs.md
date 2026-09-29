+++
code = 'E244'
name = 'Stress-run repair boundaries'

[[example]]
title = 'Separated primary and secondary stress'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ˈhelˌlo .
@End
'''

[[example]]
title = 'Three primary markers before a later secondary marker'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ˈˈˈhelˌlo .
@End
'''

[[example]]
title = 'Two separated primary positions remain a separate policy question'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ˈhelˈlo .
@End
'''

[[example]]
title = 'Duplicate run ends before a separated later primary marker'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ˈˈhelˈlo .
@End
'''

[[example]]
title = 'Secondary marker precedes a separated primary marker'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ˌhelˈlo .
@End
'''

[[example]]
title = 'An earlier secondary marker does not hide a primary duplicate'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ˌhelˈˈlo .
@End
'''

[[example]]
title = 'Adjacent mixed stress requires author judgment'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ˈˌhello .
@End
'''

[[example]]
title = 'Secondary-only duplicates are not primary-stress repairs'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ˌˌhello .
@End
'''
[[example]]
title = 'Two separated duplicate runs belong to one word-level finding'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ˈˈhelˈˈlo .
@End
'''
+++

## Description

These pairs vary only stress-token repetition and position. The primary-only
repair collapses adjacent primary runs and retains all other tokens.
E244 is reported once per affected word, not once per adjacent pair: a longer
run must not generate identical, overlapping repairs for the same source span.
Mixed and secondary-only duplicates still violate E244 but have no proposal
under this repair policy.

## CHAT Rule

Adjacent stress markers cannot each describe distinct following syllabic
material. Repeated primary tokens can be collapsed without choosing another
stress position. A mixed pair does not establish whether primary or secondary
stress was intended. Distinct primary positions remain subject to E247, and
secondary-only words remain subject to E250; an E244-legal control is not a
claim that these other rules accept it.
