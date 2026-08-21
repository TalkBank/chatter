+++
code = 'E715'
name = '%pho alignment count mismatch, too many tokens'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'tier'
source = 'E4xx_alignment_errors/E715_pho_count_too_many.cha'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.|male|||Target_Child|||
*CHI:	want cookie .
%pho:	aɪ wɑnt kʊki
@Comment:	ERROR: Main tier has 2 words but %pho has 3 tokens (extra aɪ)
@End
'''
+++

## Description

The `%pho` (actual phonology) tier has more alignable tokens than the main tier.
Remove the extra `%pho` tokens so counts match.

`%mod` count mismatches use E734. `%wor` is not an alignment tier; it is a
timing sidecar (`WorTimingSidecar`) modeled in
[`talkbank-model::alignment`](../../crates/talkbank-model/src/alignment/wor.rs),
so no E7xx error fires on a `%wor` count mismatch; drift is reported
structurally via the `Drifted` variant, not via `ParseError`.

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- E715 is scoped to `%pho` only; `%mod` uses E734. `%wor` is a timing sidecar, not an alignment, see `WorTimingSidecar`.
