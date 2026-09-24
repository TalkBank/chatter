+++
code = 'E711'
name = 'Empty features on main and post-clitic morphology words'

[[example]]
title = 'Flat feature control'
level = 'tier'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	it's .
%mor:	pron|it-Nom~aux|be-Pres .
@End
'''

[[example]]
title = 'Keyed feature control'
level = 'tier'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	it's .
%mor:	pron|it-Case=Nom~aux|be-Tense=Pres .
@End
'''

[[example]]
title = 'Remove the main-word feature value'
level = 'tier'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	it's .
%mor:	pron|it-Case=~aux|be-Tense=Pres .
@End
'''

[[example]]
title = 'Remove the post-clitic feature value'
level = 'tier'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	it's .
%mor:	pron|it-Case=Nom~aux|be-Tense= .
@End
'''

[[example]]
title = 'Remove both feature values'
level = 'tier'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	it's .
%mor:	pron|it-Case=~aux|be-Tense= .
@End
'''
+++

**Status:** Current
**Last updated:** 2026-09-23 18:39 EDT

## Description

A post-clitic is a morphological word with its own features, not an unchecked
suffix of its host. Both the main word and every post-clitic require nonempty
feature values. Flat and keyed features retain their distinct written forms.

## Expected Behavior

Both controls parse without recovery and emit no E711. Removing either value
emits one E711; removing both emits two, without dropping either word or key.
The morphology remains one item with two word chunks and a final terminator.
JSON roundtrip preserves flat/keyed form and empty values; it does not make an
invalid feature valid or restore source-parser provenance.

## CHAT Rule

Every morphological feature must have content, including features carried by
post-clitics. A keyed feature requires a value after its equals sign. This is
the existing E711 rule applied independently to each morphological word.

## Notes

This is an authored boundary matrix, not a new production attestation. The
keyed control is the seed for the three value-deletion variants; each mutation
preserves all other CHAT bytes. Claims are independent of CHECK observations.

CHECK 21-Sep-2026 accepted all five files in a file-mode observation on
2026-09-23 at18:38 EDT. This is not parity for empty feature values: Chatter
retains its existing nonempty-value rule even though CHECK accepts these
variants. The two-error expectation for the combined mutation is additionally
checked by the corpus contract; the observation snapshot records code sets,
not diagnostic multiplicities.
