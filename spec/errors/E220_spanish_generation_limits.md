+++
code = 'E220'
name = 'Unsupported Spanish number generation preserves digit words'

[[example]]
title = 'Unsupported scale, agreement and overflow forms retain their spelling'
level = 'word'
claim = 'violates'
notes = 'These are generation-refusal boundaries, not accepted Spanish cardinal spellings. Numeric words remain invalid under the existing E220 policy.'
chat = '''
@UTF8
@Begin
@Languages:	spa
@Participants:	CHI Target_Child
@ID:	spa|corpus|CHI|||||Target_Child|||
*CHI:	1000000 21000 31001 101000 18446744073709551616 1000000€ 21-1000000 21000-year-old .
@End
'''
+++

## Description

Explicit Spanish number generation preserves the complete input when its
bounded cardinal grammar cannot safely express a scale, resolve agreement,
or represent the magnitude. A currency symbol or compound
does not make the numeral valid CHAT or authorize partial generation.

## Expected Behavior

Parsing retains the lexical values. Validation reports E220 for the digit
words. Explicit generation leaves every value unchanged, including currency
and compound forms, rather than manufacturing Spanish or rewriting only a
supported portion. Legal written controls are in
`corpus/reference/word-features/spanish-number-spelling.cha`.
