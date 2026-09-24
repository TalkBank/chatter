+++
code = 'E704'
name = 'Untranscribed speech constrains same-speaker timing'

[[example]]
title = 'Untranscribed material constrains following speech'
level = 'utterance'
claim = 'violates'
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|sample|CHI|||||Target_Child|||
@Media:	E704_untranscribed_timing_1, audio
*CHI:	hello . \u00151000_2000\u0015
*CHI:	www . \u00151500_9000\u0015
*CHI:	world . \u00152000_3000\u0015
@End
"""

[[example]]
title = 'Unintelligible material constrains following speech'
level = 'utterance'
claim = 'violates'
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|sample|CHI|||||Target_Child|||
@Media:	E704_untranscribed_timing_2, audio
*CHI:	hello . \u00151000_2000\u0015
*CHI:	xxx . \u00151500_9000\u0015
*CHI:	world . \u00152000_3000\u0015
@End
"""

[[example]]
title = 'Phonetic placeholder constrains following speech'
level = 'utterance'
claim = 'violates'
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|sample|CHI|||||Target_Child|||
@Media:	E704_untranscribed_timing_3, audio
*CHI:	hello . \u00151000_2000\u0015
*CHI:	yyy . \u00151500_9000\u0015
*CHI:	world . \u00152000_3000\u0015
@End
"""

[[example]]
title = 'Lexical substitution retains the same overlap violation'
level = 'utterance'
claim = 'violates'
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|sample|CHI|||||Target_Child|||
@Media:	E704_untranscribed_timing_4, audio
*CHI:	hello . \u00151000_2000\u0015
*CHI:	again . \u00151500_9000\u0015
*CHI:	world . \u00152000_3000\u0015
@End
"""
+++

**Status:** Adjudicated — timed speech occupies time regardless of transcription
**Last updated:** 2026-09-23 19:37 EDT

## Description

An authored timing matrix compares untranscribed-only turns with lexical
speech. All starts are nondecreasing. The middle turn has a broad timing span;
substituting a lexical word does not change its timing obligation. These
examples are not production attestations.

## CHAT Rule

All timed speech constrains the same speaker's following turn, with a 500 ms
tolerance. Untranscribed or unintelligible speech still occupies time. The
third turn is compared with the middle turn's endpoint, regardless of whether
its words have been transcribed.

## Expected Behavior

All four examples emit E704 on the third turn. The second turn overlaps the
first by exactly the tolerated 500 ms; the third overlaps the second by
7000 ms. This timing rule does not disable other validation rules. In
particular, `yyy` can require phonological transcription.

## CHECK observation

CHECK21-Sep-2026 observed on September 23 reports 133 on the third utterance
in all four examples. The adopted rule agrees with that observation. Chatter's
former exemption confused transcription availability with whether speech
occupies time; these cases prevent that exemption from returning.
