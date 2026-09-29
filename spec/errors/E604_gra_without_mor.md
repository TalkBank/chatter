+++
code = 'E604'
name = '%gra Tier Without %mor Tier'

[[example]]
level = 'utterance'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello world .
%gra:	1|2|NSUBJ 2|0|ROOT
@End
'''
[[example]]
level = 'utterance'
claim = 'violates'
notes = 'A comment may intervene before GRA, and its relations may continue on a tab-indented line. A requested GRA removal must preserve the comment and remove the complete structured tier, not assume the next physical line.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello world .
%com:	keep this comment
%gra:	1|2|NSUBJ
	2|0|ROOT
@End
'''

[[example]]
level = 'utterance'
claim = 'legal'
notes = 'Independent expected output for removing the orphaned GRA tier; the main tier and comment remain unchanged.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello world .
%com:	keep this comment
@End
'''
[[example]]
level = 'utterance'
claim = 'violates'
notes = 'Multiple orphaned GRA tiers still violate E604, but the single-target removal proposal must refuse rather than guess which tier to delete.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello world .
%gra:	1|2|NSUBJ 2|0|ROOT
%gra:	1|2|NSUBJ 2|0|ROOT
@End
'''
+++

## Description

A %gra (grammatical relations) tier appears without a corresponding %mor (morphology) tier. According to CHAT rules, %gra depends on %mor and cannot exist independently.

## Expected Behavior

- **Parser**: Should succeed - syntax is valid for both tiers
- **Validator**: Should report E604 - %gra tier present but %mor tier is missing

## CHAT Rule

The %gra tier provides grammatical relations derived from the %mor tier analysis. Every utterance with a %gra tier must also have a %mor tier that precedes it.

## Notes

This is a dependency validation error. The %gra tier references morphological units from the %mor tier, so %mor must be present first. This error is detected during semantic validation after successful parsing.
