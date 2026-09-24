+++
code = 'E209'
name = 'CA omission versus adjacent shortenings'

[[example]]
title = 'Single parenthesized CA word'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@Options:	CA
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	(ab) .
@End
'''

[[example]]
title = 'Two shortenings do not form a CA omission'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@Options:	CA
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	(a)(b) .
@End
'''

[[example]]
title = 'Single parenthesized CA replacement'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@Options:	CA
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	word [: (ab)] .
@End
'''

[[example]]
title = 'Two shortenings in replacement text'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@Options:	CA
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	word [: (a)(b)] .
@End
'''
+++

## Description

In CA mode a single standalone parenthesized word is interpreted as a CA
omission, with spoken text in the model. Inserting `)(` between its letters
produces two adjacent shortenings instead. Neither contributes spoken material,
and the combination is not a single CA omission. The same distinction applies
to words inside replacement annotations; enclosing syntax does not supply
missing spoken content.

## CHAT Rule

The CA-omission interpretation applies to a single parenthesized word, not an
arbitrary sequence of shortening elements. Ordinary shortenings need spoken
material elsewhere in the word. These pairs vary only the internal boundary;
the file's CA option and surrounding annotation remain fixed.

## Observations

All four cases parse without diagnostics and serialize byte-exactly. Chatter
validates the single-parenthesis controls without diagnostics and emits E209
for both double-shortening variants. CHECK (21-Sep-2026) accepts all four.
This records Chatter's existing single-shortening normalization boundary; it
does not claim CHECK parity for these shapes or introduce a new normalization
policy based on CHECK's silence.
