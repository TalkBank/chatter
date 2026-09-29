+++
code = 'E220'
name = 'Currency symbols do not license digits in English lexical words'

[[example]]
level = 'utterance'
claim = 'violates'
notes = 'Authored numeric currency forms corresponding to the written reference controls in currency-number-spelling.cha. Currency interpretation belongs to an explicit generation operation, not validation or parser recovery.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
*CHI:	£12 ¥12 ₹12 ₩12 ₽12 €12 12€ 12₹ .
@End
'''
+++

## Description

English lexical words containing numeric currency amounts still contain illegal
digits. A currency symbol is not an exemption from the lexical digit rule.

## CHAT Rule

Write the spoken words in the main tier. Number expansion is an explicit
generation aid; neither parsing nor validation silently rewrites a transcript.

