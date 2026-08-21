+++
code = 'E734'
name = '%mod alignment count mismatch, too many tokens'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'tier'
source = 'E4xx_alignment_errors/E734_mod_count_too_many.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	want cookie .
%mod:	aɪ wɑnt kʊki
@Comment:	ERROR: Main tier has 2 words but %mod has 3 tokens (extra aɪ)
@End
'''
+++

## Description

The `%mod` (model/target phonology) tier has more alignable tokens than the
main tier. Remove the extra `%mod` tokens so counts match.

This code is scoped to `%mod` only. `%pho` count mismatches use E715. `%wor`
is a timing-annotation tier and is never validated for count mismatches.

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Introduced when %pho/%mod/%wor alignment codes were separated (previously all used E714/E715)
- E734 is scoped to `%mod` only; `%pho` uses E715
