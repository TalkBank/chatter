+++
code = 'E220'
name = 'Interposed words retain ordinary word validation'

[[example]]
title = 'root: violates'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello &*MOT:hello3 .
@End
'''

[[example]]
title = 'root: legal'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello &*MOT:hello .
@End
'''

[[example]]
title = 'angle: violates'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello <&*MOT:hello3> [!] .
@End
'''

[[example]]
title = 'angle: legal'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello <&*MOT:hello> [!] .
@End
'''

[[example]]
title = 'quotation: violates'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello “&*MOT:hello3” .
@End
'''

[[example]]
title = 'quotation: legal'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello “&*MOT:hello” .
@End
'''

[[example]]
title = 'phonological: violates'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello ‹&*MOT:hello3› .
@End
'''

[[example]]
title = 'phonological: legal'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello ‹&*MOT:hello› .
@End
'''

[[example]]
title = 'sign: violates'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello 〔&*MOT:hello3〕 .
@End
'''

[[example]]
title = 'sign: legal'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello 〔&*MOT:hello〕 .
@End
'''

[[example]]
title = 'explicit language: violates'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello &*MOT:ma1@s:eng .
@End
'''

[[example]]
title = 'explicit language: legal'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	hello &*MOT:ma1@s:zho .
@End
'''
+++

## Description

Interposed speech (`&*SPK:word`) contains a `standalone_word`, so its payload
must receive the same word validation as an ordinary word. English does not
permit digits in orthographic word text. These paired controls cover root,
annotated angle, quotation, phonological and sign scopes. The final pair keeps
the digit-bearing payload fixed and changes only its explicit language marker:
English rejects the digit, while Mandarin permits it as a tone number.

The background word does not join the main speaker's alignment sequence merely
because it receives lexical validation. Its structured content must preserve
both validation behavior and CHAT spelling after JSON serialization.
