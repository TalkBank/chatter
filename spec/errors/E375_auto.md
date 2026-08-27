+++
code = 'E375'
name = 'Scoped annotation parse error'

[[example]]
level = 'utterance'
source = 'E3xx_main_tier_errors/E350_unexpected_annotation_node.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
@Comment:	Note: This may need adjustment after testing
*CHI:	hello [[[[ test ]]]] world .
@End
'''

[[example]]
title = 'A scoped annotation may follow a quotation'
level = 'utterance'
claim = 'legal'
notes = '''
The minimal form of the same 2026-08-26 report. `plant [//] plant` and
`<“plant”> [//] plant` were both accepted while `“plant” [//] plant` was
not, which is a gap in what may carry a scoped annotation rather than a rule
about scoped annotations. Real CLAN CHECK accepts all three.

The published corpora contain ZERO instances: every closing-quote-then-bracket
match there is a CA annotation, which reduces elsewhere and was never broken.
The construct arrives with incoming data, which is where it was reported from.
'''
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	TEA Teacher
@ID:	eng|corpus|TEA|||||Teacher|||
*TEA:	“plant” [//] plant .
@End
'''
+++

## Description

Scoped annotation parse error

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
