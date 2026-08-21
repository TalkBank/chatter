+++
code = 'E372'
name = 'Nested quotation'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'utterance'
source = 'validation_gaps/nested-quotation.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
@ID:	eng|corpus|MOT|30;00.|female|||Mother|||
*MOT:	she said “I told him “go away” yesterday” .
@Comment:	ERROR: Nested quotations - "go away" is inside "I told him..."
@Comment:	A state stack detects this; Rust only counts balance
*CHI:	okay mommy .
*MOT:	he said “hello” and “goodbye” .
@Comment:	VALID: Two separate quotations, not nested
@End
'''

[[example]]
level = 'utterance'
title = 'nesting below a retrace'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|3;00.00|male|||Target_Child|||
@Comment:	ERROR: the model is quotation -> retrace -> quotation
*CHI:	“a <“b”> [/] c” .
@End
'''

[[example]]
level = 'utterance'
title = 'nesting below a phonological group'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|3;00.00|male|||Target_Child|||
@Comment:	ERROR: nesting is nesting at any depth, through any container
*CHI:	“a ‹“b”› c” .
@End
'''
+++

Detected at ANY depth, through every container. Until 2026-08-07 the predicate
recursed into annotated groups only, so a quotation inside a retrace, a
phonological group, a sign group, or another quotation was invisible: examples
2 and 3 below reported nothing while example 1 reported E372.

## Description

Nested quotation

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
