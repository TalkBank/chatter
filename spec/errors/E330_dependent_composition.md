+++
code = 'E330'
name = 'Dependent-tier composition and traversal recovery'

[[example]]
title = 'Complete dependent-tier composition control'
level = 'tier'
claim = 'legal'
notes = 'The reference control admits three utterances, each with MOR, GRA, comment, translation and user-defined content. A legal claim excludes E330; the integration contract separately requires diagnostic-free parsing and alignment-aware validation.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
@Comment:	Unicode café recovery control
*CHI:	before .
%mor:	co|before .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	before comment
%eng:	before translation
%xnote:	before café
*MOT:	middle .
%mor:	co|middle .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	middle comment
%eng:	middle translation
%xnote:	middle café
*CHI:	after .
%mor:	co|after .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	after comment
%eng:	after translation
%xnote:	after café
@End
'''

[[example]]
title = 'A MOR item without its POS separator between healthy tiers'
level = 'tier'
claim = { subsumed_by = 'E702' }
notes = 'Replace only the middle MOR body. Retain the same speaker, main tier, other dependent tiers and neighboring utterances; a morphology-context recovery node reports E702, not internal traversal failure.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
@Comment:	Unicode café recovery control
*CHI:	before .
%mor:	co|before .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	before comment
%eng:	before translation
%xnote:	before café
*MOT:	middle .
%mor:	middle .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	middle comment
%eng:	middle translation
%xnote:	middle café
*CHI:	after .
%mor:	co|after .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	after comment
%eng:	after translation
%xnote:	after café
@End
'''

[[example]]
title = 'A MOR item without its stem between healthy tiers'
level = 'tier'
claim = { subsumed_by = 'E702' }
notes = 'Delete the authored stem only within the middle MOR body. A pipe does not establish a complete item; recovery preserves the tier slot without admitting morphology alignment.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
@Comment:	Unicode café recovery control
*CHI:	before .
%mor:	co|before .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	before comment
%eng:	before translation
%xnote:	before café
*MOT:	middle .
%mor:	co| .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	middle comment
%eng:	middle translation
%xnote:	middle café
*CHI:	after .
%mor:	co|after .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	after comment
%eng:	after translation
%xnote:	after café
@End
'''

[[example]]
title = 'A missing GRA index beside healthy MOR'
level = 'tier'
claim = { subsumed_by = 'E316' }
notes = 'Delete the first grammatical index. Native traversal inserts a zero-width index token, not an individually reportable ERROR/MISSING node. The recovered tier still reports a source-local E316 summary and retains an Unknown empty placeholder with GRA parse taint. Native lowering rejects the declared malformed relation but retains PUNCT; document recovery does not claim that native retention receipt.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
@Comment:	Unicode café recovery control
*CHI:	before .
%mor:	co|before .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	before comment
%eng:	before translation
%xnote:	before café
*MOT:	middle .
%mor:	co|middle .
%gra:	|0|ROOT 2|1|PUNCT
%com:	middle comment
%eng:	middle translation
%xnote:	middle café
*CHI:	after .
%mor:	co|after .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	after comment
%eng:	after translation
%xnote:	after café
@End
'''

[[example]]
title = 'A zero GRA index followed by an intact relation'
level = 'tier'
claim = { subsumed_by = 'E709' }
notes = 'Change only the first index to zero. The native tier is structurally intact: rejecting one Present relation cannot be undone by the later retained PUNCT relation.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
@Comment:	Unicode café recovery control
*CHI:	before .
%mor:	co|before .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	before comment
%eng:	before translation
%xnote:	before café
*MOT:	middle .
%mor:	co|middle .
%gra:	0|0|ROOT 2|1|PUNCT
%com:	middle comment
%eng:	middle translation
%xnote:	middle café
*CHI:	after .
%mor:	co|after .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	after comment
%eng:	after translation
%xnote:	after café
@End
'''

[[example]]
title = 'An overflowing GRA index followed by an intact relation'
level = 'tier'
claim = { subsumed_by = 'E708' }
notes = 'Replace the first index with a numeric value too large for the model. Grammar acceptance is distinct from numeric admission; retain the later relation and healthy MOR.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
@Comment:	Unicode café recovery control
*CHI:	before .
%mor:	co|before .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	before comment
%eng:	before translation
%xnote:	before café
*MOT:	middle .
%mor:	co|middle .
%gra:	99999999999999999999999|0|ROOT 2|1|PUNCT
%com:	middle comment
%eng:	middle translation
%xnote:	middle café
*CHI:	after .
%mor:	co|after .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	after comment
%eng:	after translation
%xnote:	after café
@End
'''

[[example]]
title = 'A comment body beginning with a bare bullet opener'
level = 'tier'
claim = 'violates'
notes = 'Insert a bare U+0015 only at the start of the middle comment body. Reuse the E330 body-recovery rule, keeping the MOR/GRA and other text sentinels unchanged.'
chat = """
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child, MOT Mother
@ID:\teng|corpus|CHI|||||Target_Child|||
@ID:\teng|corpus|MOT|||||Mother|||
@Comment:\tUnicode café recovery control
*CHI:\tbefore .
%mor:\tco|before .
%gra:\t1|0|ROOT 2|1|PUNCT
%com:\tbefore comment
%eng:\tbefore translation
%xnote:\tbefore café
*MOT:\tmiddle .
%mor:\tco|middle .
%gra:\t1|0|ROOT 2|1|PUNCT
%com:\t\u0015middle comment
%eng:\tmiddle translation
%xnote:\tmiddle café
*CHI:\tafter .
%mor:\tco|after .
%gra:\t1|0|ROOT 2|1|PUNCT
%com:\tafter comment
%eng:\tafter translation
%xnote:\tafter café
@End
"""

[[example]]
title = 'A translation body beginning with a bare bullet opener'
level = 'tier'
claim = 'violates'
notes = 'Insert a bare U+0015 only at the start of the middle translation body. Raw text dispatch must retain independent healthy tiers, while an unidentified alignment domain conservatively blocks all dependent alignments.'
chat = """
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child, MOT Mother
@ID:\teng|corpus|CHI|||||Target_Child|||
@ID:\teng|corpus|MOT|||||Mother|||
@Comment:\tUnicode café recovery control
*CHI:\tbefore .
%mor:\tco|before .
%gra:\t1|0|ROOT 2|1|PUNCT
%com:\tbefore comment
%eng:\tbefore translation
%xnote:\tbefore café
*MOT:\tmiddle .
%mor:\tco|middle .
%gra:\t1|0|ROOT 2|1|PUNCT
%com:\tmiddle comment
%eng:\t\u0015middle translation
%xnote:\tmiddle café
*CHI:\tafter .
%mor:\tco|after .
%gra:\t1|0|ROOT 2|1|PUNCT
%com:\tafter comment
%eng:\tafter translation
%xnote:\tafter café
@End
"""

[[example]]
title = 'A user-defined body beginning with a bare bullet opener'
level = 'tier'
claim = 'violates'
notes = 'Insert a bare U+0015 only at the start of the middle user-defined body. Preserve its native prefix identity; do not confuse body recovery with absence or label recovery.'
chat = """
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child, MOT Mother
@ID:\teng|corpus|CHI|||||Target_Child|||
@ID:\teng|corpus|MOT|||||Mother|||
@Comment:\tUnicode café recovery control
*CHI:\tbefore .
%mor:\tco|before .
%gra:\t1|0|ROOT 2|1|PUNCT
%com:\tbefore comment
%eng:\tbefore translation
%xnote:\tbefore café
*MOT:\tmiddle .
%mor:\tco|middle .
%gra:\t1|0|ROOT 2|1|PUNCT
%com:\tmiddle comment
%eng:\tmiddle translation
%xnote:\t\u0015middle café
*CHI:\tafter .
%mor:\tco|after .
%gra:\t1|0|ROOT 2|1|PUNCT
%com:\tafter comment
%eng:\tafter translation
%xnote:\tafter café
@End
"""

[[example]]
title = 'An empty comment keeps parsed provenance until validation'
level = 'tier'
claim = { subsumed_by = 'E756' }
notes = 'Delete the middle comment payload, retaining label, separator and newline. An optional empty text body remains modeled and parser-clean; E756 belongs to validation, not recovery taint.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
@Comment:	Unicode café recovery control
*CHI:	before .
%mor:	co|before .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	before comment
%eng:	before translation
%xnote:	before café
*MOT:	middle .
%mor:	co|middle .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	
%eng:	middle translation
%xnote:	middle café
*CHI:	after .
%mor:	co|after .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	after comment
%eng:	after translation
%xnote:	after café
@End
'''

[[example]]
title = 'An empty user-defined tier keeps its label until validation'
level = 'tier'
claim = { subsumed_by = 'E756' }
notes = 'Delete only the middle user-defined payload. Preserve the x namespace, tier slot and separator; validation reports E756 without manufacturing parse recovery.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
@Comment:	Unicode café recovery control
*CHI:	before .
%mor:	co|before .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	before comment
%eng:	before translation
%xnote:	before café
*MOT:	middle .
%mor:	co|middle .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	middle comment
%eng:	middle translation
%xnote:	
*CHI:	after .
%mor:	co|after .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	after comment
%eng:	after translation
%xnote:	after café
@End
'''

[[example]]
title = 'A nonnumeric GRA index creates native ERROR recovery'
level = 'tier'
claim = { subsumed_by = 'E316' }
notes = 'Replace the first grammatical index with alphabetic text, leaving the pipe, head and relation label intact. Unlike deleting the index, this creates an ERROR carrier. Document placeholders, native retained relation slots and strict fragment refusal remain different contracts.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother
@ID:	eng|corpus|CHI|||||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
@Comment:	Unicode café recovery control
*CHI:	before .
%mor:	co|before .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	before comment
%eng:	before translation
%xnote:	before café
*MOT:	middle .
%mor:	co|middle .
%gra:	bad|0|ROOT 2|1|PUNCT
%com:	middle comment
%eng:	middle translation
%xnote:	middle café
*CHI:	after .
%mor:	co|after .
%gra:	1|0|ROOT 2|1|PUNCT
%com:	after comment
%eng:	after translation
%xnote:	after café
@End
'''

+++

## Description

Dependent-tier recovery preserves source-owned attachment and unrelated content.
Cross-tier parse health is not a tier's own retention receipt or a CHAT-validity
claim. Structured MOR rejection, GRA structural placeholders, partial GRA
lowering, free-text body recovery and validation-only empty content have distinct
contracts. The complete control and eleven body-only variants keep those contracts
composed with intact dependent tiers and earlier/later utterances.

## CHAT Rule

MOR items require a POS, pipe and stem; GRA relations require admitted positive
indices and numeric heads. A bare media-bullet opener is not free text. Empty
optional text content remains represented for E756 validation. Healthy native
carriers must remain attached to their own main tier; recovery does not license
invented identities or certainty.

