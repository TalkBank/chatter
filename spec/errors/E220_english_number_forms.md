+++
code = 'E220'
name = 'English numeric cardinal, ordinal and decade spellings'

[[example]]
title = 'Numeric cardinal, ordinal and decade tokens require written pronunciation'
level = 'word'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	1st 12th 20th 21st 100th 101st 120th 121st 1000th 2000th 1001st 1010th 1100th 1234th 9999th 10s 20s 80s 1900s 1950s 2000s 2010s 1 42 101 1234 2000 10001 20000 1000000 2000000 1000001 2000000000 3000000000000 4000000000000000 5000000000000000000 18446744073709551615 17-year-old 3-star 21-22 21-22-23 90s 1100s 2990s .
@End
'''
+++

**Status:** Current
**Last updated:** 2026-09-28 10:22 EDT

## Description

English ordinal and decade suffixes do not license digits in spoken-word
transcription. Write the pronunciation as words. Numeric forms remain invalid
even though their letter suffixes distinguish them from bare numeral tokens.

## CHAT Rule

Transcribe numbers according to their spoken pronunciation. The authored
controls in `corpus/reference/word-features/english-number-spelling.cha` supply
one explicit generation policy, including British-style ordinal conjunctions,
short-scale cardinals without conjunctions, and expanded decade names. The
ordinal controls include thousands with small and hundred-sized remainders.
Generation produces spoken word sequences, without prose commas; it is not
punctuation parity with a general-purpose number-formatting library. The
largest cardinal exercises the generation API's u64 boundary, not an attested
spoken value. These are not production attestations and do not
assert that a context-free generator can recover what a speaker actually said.
Digit-leading compounds likewise require written number words. The generation
controls preserve alphabetic compound tails (`17-year-old`, `3-star`) and
spell each group of an all-numeric dash sequence separately (`21-22`,
`21-22-23`). This is an explicit generation convention, not an inference that a
dash in an original recording was spoken as a range or an omitted word.

## Notes

The paired corpus contract calls the number-generation API and compares its
output with independently written controls. It does not edit either transcript
or use a recovered model to clean invalid CHAT.
