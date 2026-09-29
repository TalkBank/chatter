+++
code = 'E501'
name = 'Duplicate headers remain diagnosable during unrelated recovery'

[[example]]
title = 'A single language declaration with a stray closing bracket'
level = 'file'
claim = 'legal'
notes = 'Keep the single Languages control from E501 and insert only an unmatched closing bracket into its main tier. Legal here asserts absence of E501, not acceptance of the malformed main tier.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello ] .
@End
'''

[[example]]
title = 'An identical duplicate does not authorize editing a recovered document'
level = 'file'
claim = 'violates'
notes = 'Duplicate only the Languages declaration in the preceding control. Header occurrence evidence survives the unrelated main-tier fault, but the catalog requires a recovery-free document before proposing duplicate deletion.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello ] .
@End
'''
+++

## Description

A malformed utterance does not make duplicate structured headers permissible.
The diagnostic and a safe repair are different obligations: reporting a known
duplicate does not certify that a document requiring grammar recovery can be
rewritten automatically.

## CHAT Rule

Use one `@Languages` declaration and balanced main-tier syntax. Repair the
unmatched bracket using the intended utterance; do not guess its missing mate.
