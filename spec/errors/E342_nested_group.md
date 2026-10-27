+++
code = 'E342'
name = 'Nested group requires its own annotation'

[[example]]
title = 'Each nested scope has its own annotation'
level = 'utterance'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<before <hello> [?] after> [!] .
@End
'''

[[example]]
title = 'Delete only the inner scope annotation'
level = 'utterance'
claim = { subsumed_by = 'E316' }
notes = 'The outer emphasis marker does not supply the missing inner annotation. This deletion produces a whole-tier ERROR node, not a recoverable group with a MISSING annotation slot; generic E316 reports that evidence without inventing narrower structure.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	<before <hello> after> [!] .
@End
'''

[[example]]
title = 'An annotated inner scope inside a quotation'
level = 'utterance'
claim = 'legal'
notes = 'The regular inner scope retains its own uncertainty annotation.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	“<hello> [?]” .
@End
'''

[[example]]
title = 'Delete only the inner annotation inside a quotation'
level = 'utterance'
claim = 'violates'
notes = 'Delete only [?] from the paired control, retaining its preceding space and the outer delimiters. The inner angle-bracket scope still requires its own annotation.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	“<hello> ” .
@End
'''

[[example]]
title = 'An annotated inner scope inside a phonological group'
level = 'utterance'
claim = 'legal'
notes = 'The regular inner scope retains its own uncertainty annotation.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	‹<hello> [?]› .
@End
'''

[[example]]
title = 'Delete only the inner annotation inside a phonological group'
level = 'utterance'
claim = 'violates'
notes = 'Delete only [?] from the paired control, retaining its preceding space and the outer delimiters. The inner angle-bracket scope still requires its own annotation.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	‹<hello> › .
@End
'''

[[example]]
title = 'An annotated inner scope inside a sign group'
level = 'utterance'
claim = 'legal'
notes = 'The regular inner scope retains its own uncertainty annotation.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	〔<hello> [?]〕 .
@End
'''

[[example]]
title = 'Delete only the inner annotation inside a sign group'
level = 'utterance'
claim = 'violates'
notes = 'Delete only [?] from the paired control, retaining its preceding space and the outer delimiters. The inner angle-bracket scope still requires its own annotation.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	〔<hello> 〕 .
@End
'''
+++

## Description

Every angle-bracket scope requires its own following annotation. An outer
scope's annotation cannot stand in for a missing inner annotation.

## Expected Behavior

Reject every scope whose required inner annotation is missing. In the original
angle-group deletion (example 2), the grammar cannot retain a typed inner group
and instead reports a whole-tier ERROR through E316; E342 is subsumed. Do not
reconstruct group structure from that ERROR text.

Examples 4, 6 and 8 retain a typed bare inner Group inside the quotation,
phonological group and sign group. The parser emits E342 at the missing marker
position immediately before the outer closing delimiter. It retains the inner
word and written brackets without inventing an annotation or retrace. The
paired controls are clean and roundtrip byte for byte. Invalid recovery output
omits the now-unused annotation whitespace; it is not byte-exact input or a
repaired control. Its bare group remains invalid during model validation too,
including after JSON readback without source coordinates.

## CHAT Rule

Restore the intended annotation after the inner group, or remove its brackets
if no inner scope was intended. The legal control uses uncertainty for the
inner scope and emphasis for the outer scope; those meanings are independent.
