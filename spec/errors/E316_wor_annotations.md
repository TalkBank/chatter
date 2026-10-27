+++
code = 'E316'
name = 'Main-tier annotations on word timing'

[[example]]
title = "Standalone word markers control"
level = "tier"
claim = "legal"
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Media:	wor-annotations, audio
*CHI:	hello world .
%wor:	(he)llo@s:eng ice+cream 0dog \u0015100_200\u0015 world .
@End
"""

[[example]]
title = "Word error annotation is not timing content"
level = "tier"
claim = "violates"
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Media:	wor-annotations, audio
*CHI:	hello world .
%wor:	hello [*] \u0015100_200\u0015 world .
@End
"""

[[example]]
title = "Word explanation is not timing content"
level = "tier"
claim = "violates"
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Media:	wor-annotations, audio
*CHI:	hello world .
%wor:	hello [= greeting] \u0015100_200\u0015 world .
@End
"""

[[example]]
title = "Word replacement is not timing content"
level = "tier"
claim = "violates"
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Media:	wor-annotations, audio
*CHI:	hello world .
%wor:	hello [: hi] \u0015100_200\u0015 world .
@End
"""

[[example]]
title = "Word retrace is not timing content"
level = "tier"
claim = "violates"
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Media:	wor-annotations, audio
*CHI:	hello world .
%wor:	hello [/] \u0015100_200\u0015 world .
@End
"""

[[example]]
title = "Word correction is not timing content"
level = "tier"
claim = "violates"
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Media:	wor-annotations, audio
*CHI:	hello world .
%wor:	hello [//] \u0015100_200\u0015 world .
@End
"""

[[example]]
title = "Action explanation is not timing content"
level = "tier"
claim = "violates"
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Media:	wor-annotations, audio
*CHI:	hello world .
%wor:	0 [=! laughs] \u0015100_200\u0015 world .
@End
"""

[[example]]
title = "Bare action is not a display word"
level = "tier"
claim = "violates"
chat = """
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Media:	wor-annotations, audio
*CHI:	hello world .
%wor:	0 \u0015100_200\u0015 world .
@End
"""
+++

## Description

WOR echoes words and their timing, not main-tier scoped annotations. Error,
explanation, replacement and retracing annotations must not disappear during
lowering. File recovery retains the complete original tier as UnparsedWor;
fragment APIs requiring WorTier reject. A bare action marker `0` is not an omission word such as `0dog`; it must not
become a word with an empty body. Word-internal shortening, compounds,
language markers and omission prefixes remain supported.

## CHAT Rule

The [CHAT manual, Word-level diarization](https://talkbank.org/0info/manuals/CHAT.html#Wor_Tier)
excludes special codes such as comments and retracing marks from the echoed
words. The older XML schema's broader WOR content model does not justify
silently discarding those codes. This contract concerns parsed source, not
normalization of annotations from a main-tier timing projection.
