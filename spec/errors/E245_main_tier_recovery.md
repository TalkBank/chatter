+++
code = 'E245'
name = 'Marker-only word rejection retains surrounding turns'

[[example]]
title = 'Admitted whole-file control'
level = 'utterance'
claim = 'legal'
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@ID:	eng|sample|SPK|||||Speaker|||
@Comment:	Préface café
*SPK:	café ready .
%com:	opening note
*SPK:	river .
*SPK:	&-hm cedar .
*SPK:	willow [: branch] .
*SPK:	<ash elm> [!] .
*SPK:	ˈbirch .
*SPK:	spruce@u .
*SPK:	finch steady ?
%com:	closing note
@End
"""

[[example]]
title = "Primary stress alone"
level = 'utterance'
claim = 'violates'
notes = 'Change only the source-bound prefix token or complete word described in the title; all other source bytes stay fixed.'
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@ID:	eng|sample|SPK|||||Speaker|||
@Comment:	Préface café
*SPK:	café ready .
%com:	opening note
*SPK:	river .
*SPK:	&-hm cedar .
*SPK:	willow [: branch] .
*SPK:	<ash elm> [!] .
*SPK:	ˈ .
*SPK:	spruce@u .
*SPK:	finch steady ?
%com:	closing note
@End
"""

[[example]]
title = "Both stress levels without spoken material"
level = 'utterance'
claim = 'violates'
notes = 'Change only the source-bound prefix token or complete word described in the title; all other source bytes stay fixed.'
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	SPK Speaker
@ID:	eng|sample|SPK|||||Speaker|||
@Comment:	Préface café
*SPK:	café ready .
%com:	opening note
*SPK:	river .
*SPK:	&-hm cedar .
*SPK:	willow [: branch] .
*SPK:	<ash elm> [!] .
*SPK:	ˈˌ .
*SPK:	spruce@u .
*SPK:	finch steady ?
%com:	closing note
@End
"""

+++

## Description

A word containing stress markers but no spoken material is rejected by the
existing E245 parser rule. The same file still contains its native neighboring
main/dependent tiers and the target terminator. No empty or invented Word may
replace the rejected carrier. Primary and secondary stress policy is unchanged.

## CHAT Rule

These examples extend the existing rule to a connected file and fragment
boundary. Recovery does not certify validity or permit fabricated speaker or
spoken material. The fully admitted controls remain independently required.
