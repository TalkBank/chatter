+++
code = 'E316'
name = 'Main-tier recovery preserves source-owned neighboring turns'

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
title = "Delete the star only"
level = 'utterance'
claim = { subsumed_by = 'E326' }
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
SPK:	river .
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
title = "Delete the speaker only"
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
*:	river .
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
title = "Delete the colon only"
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
*SPK	river .
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
title = "Delete the tab only"
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
*SPK:river .
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
title = "Duplicate the colon"
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
*SPK::	river .
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
title = "Space inside the speaker code"
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
*SP K:	river .
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
title = "Replace the required tab with a space"
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
*SPK: river .
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
title = "A bare filler marker"
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
*SPK:	& cedar .
*SPK:	willow [: branch] .
*SPK:	<ash elm> [!] .
*SPK:	ˈbirch .
*SPK:	spruce@u .
*SPK:	finch steady ?
%com:	closing note
@End
"""

[[example]]
title = "A filler prefix without content"
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
*SPK:	&- cedar .
*SPK:	willow [: branch] .
*SPK:	<ash elm> [!] .
*SPK:	ˈbirch .
*SPK:	spruce@u .
*SPK:	finch steady ?
%com:	closing note
@End
"""

[[example]]
title = "A nonword prefix without content"
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
*SPK:	&~ cedar .
*SPK:	willow [: branch] .
*SPK:	<ash elm> [!] .
*SPK:	ˈbirch .
*SPK:	spruce@u .
*SPK:	finch steady ?
%com:	closing note
@End
"""

[[example]]
title = "Replace the annotation host with an unmatched bracket"
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
*SPK:	] [: branch] .
*SPK:	<ash elm> [!] .
*SPK:	ˈbirch .
*SPK:	spruce@u .
*SPK:	finch steady ?
%com:	closing note
@End
"""

[[example]]
title = "An incomplete marker inside an annotated group"
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
*SPK:	<& elm> [!] .
*SPK:	ˈbirch .
*SPK:	spruce@u .
*SPK:	finch steady ?
%com:	closing note
@End
"""

+++

## Description

Incomplete speaker delimiters and malformed word carriers remain invalid.
Deleting the star makes an unprefixed line: the existing E326 rule rejects it,
not E316. This does not witness a Missing/Absent star in a native main tier.
The cases place the fault after Unicode header/content and before an intact
later main tier. Native recovery is inspected separately from the diagnostic
claim: a healthy, source-owned neighboring tier must not disappear from the
model, and fragment diagnostics must name the caller bytes. The bare ampersand
inside a group is a separate recovery boundary: native `other_spoken_event`
recovery absorbs the immediately following turn into an error node. That turn
has no healthy native main-tier carrier; later healthy turns still survive.
Do not infer
Missing/Error/Absent prefix-slot reachability from an invalid source string.

## CHAT Rule

These examples extend the existing rule to a connected file and fragment
boundary. Recovery does not certify validity or permit fabricated speaker or
spoken material. The fully admitted controls remain independently required.
