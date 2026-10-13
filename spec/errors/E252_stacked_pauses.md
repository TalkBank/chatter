+++
code = 'E252'
name = 'Leading and trailing pauses remain invalid among structural markers'

[[example]]
title = 'Admitted whole-file control'
level = 'utterance'
claim = 'legal'
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
@Comment:	Stacked marker controls
*SPK:	⌈⌊orchid⌋⌉ ⌈\u0002\u0001thistle\u0002\u0002⌉ ⌈clo↑ver⌉ ⌈la~ven+der⌉ ⌈peˈtal⌉ ⌈ir^is⌉ .
@End
"""

[[example]]
title = "Pause before all leading markers"
level = 'utterance'
claim = 'violates'
notes = 'Change only the source-bound prefix token or complete word described in the title; all other source bytes stay fixed.'
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
@Comment:	Stacked marker controls
*SPK:	^⌈⌊orchid⌋⌉ ⌈\u0002\u0001thistle\u0002\u0002⌉ ⌈clo↑ver⌉ ⌈la~ven+der⌉ ⌈peˈtal⌉ ⌈ir^is⌉ .
@End
"""

[[example]]
title = "Pause after the first overlap marker"
level = 'utterance'
claim = 'violates'
notes = 'Change only the source-bound prefix token or complete word described in the title; all other source bytes stay fixed.'
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
@Comment:	Stacked marker controls
*SPK:	⌈^⌊orchid⌋⌉ ⌈\u0002\u0001thistle\u0002\u0002⌉ ⌈clo↑ver⌉ ⌈la~ven+der⌉ ⌈peˈtal⌉ ⌈ir^is⌉ .
@End
"""

[[example]]
title = "Pause after all spoken and overlap content"
level = 'utterance'
claim = 'violates'
notes = 'Change only the source-bound prefix token or complete word described in the title; all other source bytes stay fixed.'
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
@Comment:	Stacked marker controls
*SPK:	⌈⌊orchid⌋⌉^ ⌈\u0002\u0001thistle\u0002\u0002⌉ ⌈clo↑ver⌉ ⌈la~ven+der⌉ ⌈peˈtal⌉ ⌈ir^is⌉ .
@End
"""

[[example]]
title = "Pause after a leading underline boundary"
level = 'utterance'
claim = 'violates'
notes = 'Change only the source-bound prefix token or complete word described in the title; all other source bytes stay fixed.'
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng|sample|SPK|||||Speaker|||
@Comment:	Stacked marker controls
*SPK:	⌈⌊orchid⌋⌉ ⌈\u0002\u0001^thistle\u0002\u0002⌉ ⌈clo↑ver⌉ ⌈la~ven+der⌉ ⌈peˈtal⌉ ⌈ir^is⌉ .
@End
"""

+++

## Description

Overlap and underline markers do not supply the spoken material required on
both sides of a syllable pause. The grammar admits the structured word so the
existing E252 validation rule can diagnose its edge pause. Parsing, source
spelling, validation and file-mode policy remain distinct. The legal stacked
controls travel the marker-initial word-body choice conversions.

## CHAT Rule

These examples extend the existing rule to a connected file and fragment
boundary. Recovery does not certify validity or permit fabricated speaker or
spoken material. The fully admitted controls remain independently required.
