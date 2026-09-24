+++
code = 'E243'
name = 'High-BMP word-character boundaries'

[[example]]
title = 'Standard Unicode controls below the high-BMP block'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	aab a·b a퟿b .
@End
'''

[[example]]
title = 'Both inclusive endpoints of the two CHECK-compatible exemptions'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ab ab a！b a～b .
@End
'''

[[example]]
title = 'Rejected endpoints and the neighbors immediately outside each exemption'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	ab ab ab a＀b a｟b a￿b .
@End
'''
[[example]]
title = 'A supplementary-plane scalar is not in the rejected BMP block'
level = 'word'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	a𐀀b .
@End
'''
+++

## Description

These cases isolate the existing E243 high-BMP rule by replacing the middle
character of the same ordinary `a…b` word. They preserve surrounding lexical
material, document scaffolding and termination; no recovered or manufactured
model is the oracle.

The accepted controls are ASCII, U+00B7, U+D7FF and U+10000. The four exempt
endpoints are U+F170, U+F264, U+FF01 and U+FF5E. The six rejected characters
are U+E000, U+F16F, U+F265, U+FF00, U+FF5F and U+FFFF, in that order.
U+D7FF is the final scalar before the surrogate block; surrogates are not
Unicode scalar values and cannot appear in a UTF-8 CHAT source.

This is the CHECK-compatible policy implemented by
`is_nonstandard_unicode_word_char`: reject U+E000..=U+FFFF except the two
inclusive ranges U+F170..=U+F264 and U+FF01..=U+FF5E. The source-level basis
is CLAN CHECK's `isIllegalASCII` and its two encoded-byte exemptions. These
are implementation-policy boundaries, not a claim that all characters in
this BMP block are nonstandard Unicode or that every exempt private-use
character is recommended transcription practice. A `legal` claim here means
absence of E243, not universal admissibility under every CHAT rule.

## Expected behavior

All four documents parse without recovery. Examples 1, 2 and 4 produce no E243.
Example 3 produces one E243 per rejected word, with its original source span.
The six errors must not collapse into one presence-only witness. Byte-exact
roundtrip preserves the tested character rather than normalizing it away.

## CHECK scope

The 21-Sep-2026 CHECK build agrees on the BMP controls and exemptions, but
also reports CHECK 86 for U+10000. Its byte predicate accepts lead bytes
greater than or equal to 0xEE and checks only the next two continuation
bytes; it does not restrict that branch to three-byte UTF-8 sequences.
Chatter deliberately retains its scalar-based BMP boundary instead of
rejecting standard supplementary-plane text. The CHECK 86 parity fixture
for U+E000 witnesses that shape only, not universal agreement on Unicode.
