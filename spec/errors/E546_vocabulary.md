+++
code = 'E546'
name = 'SES vocabulary and component boundaries'

[[example]]
title = 'All declared ethnicity and socioeconomic values'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother, FAT Father, INV Investigator, OBS Participant, ADU Adult, SIS Sister, BRO Brother
@ID:	eng|corpus|CHI||||White,UC|Target_Child|||
@ID:	eng|corpus|MOT||||Black,MC|Mother|||
@ID:	eng|corpus|FAT||||Asian,WC|Father|||
@ID:	eng|corpus|INV||||Latino,LI|Investigator|||
@ID:	eng|corpus|OBS||||Pacific|Participant|||
@ID:	eng|corpus|ADU||||Native|Adult|||
@ID:	eng|corpus|SIS||||Multiple|Sister|||
@ID:	eng|corpus|BRO||||Unknown|Brother|||
*CHI:	hello .
@End
'''

[[example]]
title = 'Replace only the socioeconomic component'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI||||White,ZZ|Target_Child|||
*CHI:	hello .
@End
'''

[[example]]
title = 'Replace only the ethnicity component'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI||||Unlisted,MC|Target_Child|||
*CHI:	hello .
@End
'''

[[example]]
title = 'Delete the ethnicity while retaining its separator'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI||||,MC|Target_Child|||
*CHI:	hello .
@End
'''

[[example]]
title = 'Delete the socioeconomic component while retaining its separator'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI||||White,|Target_Child|||
*CHI:	hello .
@End
'''

[[example]]
title = 'Use the supported space separator'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI||||White MC|Target_Child|||
*CHI:	hello .
@End
'''
[[example]]
title = 'Absent optional SES field'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	hello .
@End
'''

[[example]]
title = 'Insert a separator without either SES component'
level = 'header'
claim = 'violates'
notes = 'Insert only a comma into the preceding empty optional field. A separator is not evidence for either component of a combined value.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI||||,|Target_Child|||
*CHI:	hello .
@End
'''
+++

## Description

The legal control enumerates the declared ethnicity vocabulary and pairs the
four socioeconomic codes with recognized ethnicities. The invalid examples
replace or delete one component of a combined value while retaining its comma.
A separator alone does not supply a missing component. The space-separated
control is accepted but normalizes to a comma during serialization.

## CHAT Rule

The E546 vocabulary comes from `depfile.cut`: ethnicity and socioeconomic
codes may appear alone or together. Chatter accepts either comma or space
between two recognized components. Unknown or missing components in a
comma-separated pair violate E546; an entirely empty optional SES field is
a different, legal state. These are authored metadata examples, not demographic
assertions about real participants. CHECK observations are recorded separately
from the declared claims.

### CHECK's token-list boundary

CHECK's `check_ID` sends this field to `check_ses`. That scanner skips any run
of commas or whitespace before each token and stops at the field delimiter.
Each remaining token is checked against either the ethnicity or socioeconomic
vocabulary through `check_matchplate` and `check_SES_item`; unmatched tokens
produce error 144. There is no required pair structure or missing-component
check. Thus `,MC` and `White,` each reduce to one recognized token, while a
comma-only field contains no token to reject. This explains its permissiveness
without attributing the different CA scanner's double-counting defect to SES.

Chatter instead distinguishes an absent optional field, a standalone recognized
value, and a complete combined value. An explicit comma with a missing side
does not supply that complete pair. This is the existing stricter syntax policy,
not a claim that CHECK's separator-skipping code accidentally fails its own
token-list policy. Source locations in the inspected `check.cpp`:
`check_ID` 5081–5083, `check_ses` 1880–1901,
`check_matchplate` 1518–1521 and 1544–1545, and `check_SES_item` 1405–1419.

## Notes

CHECK (21-Sep-2026) accepts the vocabulary and space-separator controls and
reports 144 for either unknown component. It accepts `,MC` and `White,`;
Chatter's existing E546 check rejects both incomplete pairs. These two cases
record a stricter existing boundary, not CHECK parity. All examples parse
without diagnostics; invalid components are diagnosed during validation.
Only the space-separated control changes spelling during serialization.
The absent-field and comma-only specimens are also accepted by CHECK.
Chatter retains the former as `None` and the latter as `Unsupported(",")`,
reporting E546 only for the latter. Neither is silently converted into a
recognized ethnicity or socioeconomic value.
