+++
code = 'E342'
name = 'Missing required element'
kind = 'Invalidity'
status = 'implemented'

[[example]]
level = 'utterance'
source = 'E2xx_word_errors/E211_replacement_missing_corrected.cha'
claim = 'violates'
notes = '''
Note: `helo [: 0] world .` is malformed at BOTH stages. The grammar has no
word_segment for the `0`, so tree-sitter recovers with a MISSING node and the
parser reports E342 ("recovery is not validity"). Validation then separately
reports E390 (ReplacementContainsOmission), the more informative diagnosis,
declared by E390's own spec. (Until R4 this spec was authored parser-layer and
its generated test inspected parse diagnostics only, which is the failure the
book cites as R4's motivation; the total runner sees both stages now, and the
snapshot records the split.)

Updated 2026-08-11: this declared E390 alone, and had been unreachable since
whenever E342_auto's status became `implemented` without the generated tests
being regenerated, so four E342 tests sat `#[ignore]`d and nothing noticed the
expectation was for a layer this test does not run.
'''
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	helo [: 0] world .
@End
'''

[[example]]
level = 'utterance'
source = 'E7xx_tier_parsing/E704_empty_mor_pos.cha'
claim = { subsumed_by = 'E760' }
notes = '''
Updated 2026-08-11. This declared E316 (unparsable content) and E702 (invalid
morphology format), and emits neither: it now emits E760, which names the
actual defect ("MOR item '|hello' has an empty part-of-speech field") and
carries the rule in its help text. A generic "unparsable content" standing in
for a specific rule is the documented tell of a validator that has not yet been
taught the rule, so replacing it was the improvement; the expectation here was
simply left behind. It also emits E600 as a WARNING, saying why main-to-%mor
alignment was skipped, which is a consequence rather than a second defect and
so is not declared.
'''
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%mor:	|hello n|world .
@End
'''

[[example]]
level = 'utterance'
source = 'E7xx_tier_parsing/E703_empty_mor_stem.cha'
claim = { subsumed_by = 'E316' }
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%mor:	v| n|world .
@End
'''

[[example]]
level = 'utterance'
source = 'E7xx_tier_parsing/E711_gra_missing_role.cha'
claim = { subsumed_by = 'E316' }
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello world .
%gra:	1|2| 2|0|ROOT
@End
'''
+++

## Description

Missing required element

## Expected Behavior

The appropriate error should be reported; which stage catches it is observed in the snapshot, not declared.

## CHAT Rule

[Add link to relevant CHAT manual section]

## Notes

- Auto-generated from error corpus
- Review and enhance this specification as needed
