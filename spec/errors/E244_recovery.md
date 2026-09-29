+++
code = 'E244'
name = 'Clean stress tokens retain local evidence during unrelated recovery'

[[example]]
title = 'Separated stress remains nonconsecutive with a later stray bracket'
level = 'word'
claim = 'legal'
notes = 'Insert only an unmatched closing bracket after the separated-stress control from E244_stress_runs. The word itself remains structurally complete.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ˈhelˌlo ] .
@End
'''

[[example]]
title = 'Duplicate primary stress in a clean word retains its local proposal'
level = 'word'
claim = 'violates'
notes = 'Duplicate only the primary stress token in the preceding control. The source-bound word remains clean despite the unrelated bracket fault; a local catalog proposal is not admission to rewrite the tainted utterance.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ˈˈhelˌlo ] .
@End
'''
+++

## Description

Consecutive stress is established from the complete word's typed tokens.
Recovery elsewhere does not turn separated markers into duplicates or erase
a known duplicate. The catalog's local proposal and later edit admission are
separate steps; a proposal alone does not certify the whole utterance.

## CHAT Rule

Stress markers must not be consecutive within a word. Preserve the selected
stress positions and resolve the unrelated structural fault before writing
repairs to a tainted utterance.
