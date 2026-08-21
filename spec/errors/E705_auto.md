+++
code = 'E705'
name = 'Mor count mismatch - too few items'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'tier'
source = 'E4xx_alignment_errors/tag_marker_alignment.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@Comment:	Note: Tag markers are alignable content
*CHI:	I want ± cookie .
%mor:	pro|I v|want n|cookie .
@Comment:	ERROR: Tag marker ± should have a mor item
@Comment:	Main tier alignable: I, want, ±, cookie = 4 words
@Comment:	Mor tier: Should have 4 items (missing item for ±)
@End
'''

[[example]]
level = 'tier'
source = 'E4xx_alignment_errors/E705_mor_count_too_few.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	I want cookie .
%mor:	pro|I v|want .
@Comment:	ERROR: Main tier has 3 words but %mor only has 2 items (missing n|cookie)
@End
'''
+++

## Description

Mor count mismatch - too few items

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
