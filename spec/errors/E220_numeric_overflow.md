+++
code = 'E220'
name = 'Numeric overflow does not license digit words'

[[example]]
title = 'English unsupported ranges preserve bare, ordinal, decade and compound tokens'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	18446744073709551616 18446744073709551616th 18446744073709551616s 18446744073709551616-year-old 10000th 10001st 10002nd 10003rd 10004th 1000001st 18446744073709551615th 1s 21s 99s 100s 1090s 1101s 2001s 2991s 3000s .
@End
'''

[[example]]
title = 'Mandarin bare numeral beyond machine range'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	zho
@Participants:	CHI Target_Child
@ID:	zho|corpus|CHI|||||Target_Child|||
*CHI:	18446744073709551616 .
@End
'''

[[example]]
title = 'Cantonese bare numeral beyond machine range'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	yue
@Participants:	CHI Target_Child
@ID:	yue|corpus|CHI|||||Target_Child|||
*CHI:	18446744073709551616 .
@End
'''

[[example]]
title = 'Spanish bare numeral beyond machine range'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	spa
@Participants:	CHI Target_Child
@ID:	spa|corpus|CHI|||||Target_Child|||
*CHI:	18446744073709551616 .
@End
'''
+++

## Description

These authored controls apply the existing E220 digit-word policy to large
numerals. A numeral does not become valid CHAT because its magnitude exceeds
an implementation's numeric range. English ordinal, decade and compound
suffixes do not license its digits either. The overflow decimal value is one beyond
the maximum unsigned 64-bit integer; that is a generation boundary, not a
CHAT validity threshold.

English ordinal composition supports 0–9999. Larger ordinal tokens remain
unchanged, even when their numeric value fits in `u64`: no guessed `th` suffix
or partial expansion is permitted. The authored `9999th`
generation control is in the English number-spelling reference workflow.

Decade composition admits only multiples of ten: shorthand 0–90 or full-year
forms 1100–2990. Other suffix-bearing inputs remain byte-for-byte unchanged;
`21s` and `2001s` do not license guessed phrases such as “twenty-ones” or
“two thousand and ones”. Supported endpoints are paired with authored written
controls. Zero shorthand is tested at the raw-string API boundary because an
initial zero has separate omission semantics in CHAT.

## Expected Behavior

Parsing preserves the lexical forms. Validation reports E220. Explicit number
generation must leave these unsupported values unchanged, without truncation,
wrapping or partial rewriting. It cannot confer validity on the source.
Ordinary written-number controls remain in the English, Mandarin, Cantonese
and Spanish number-spelling reference fixtures. No new CHECK observation or
agreement claim is made.
