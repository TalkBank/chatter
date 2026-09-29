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
+++

## Description

Every angle-bracket scope requires its own following annotation. An outer
scope's annotation cannot stand in for a missing inner annotation.

## Expected Behavior

Reject the malformed nested scope. In this example the grammar cannot retain
a typed group and instead reports a whole-tier ERROR through E316; the more
specific E342 slot diagnostic is subsumed. Do not invent a retrace or reconstruct
group structure from that ERROR text. Recovered output is not a repaired control.

## CHAT Rule

Restore the intended annotation after the inner group, or remove its brackets
if no inner scope was intended. The legal control uses uncertainty for the
inner scope and emphasis for the outer scope; those meanings are independent.
