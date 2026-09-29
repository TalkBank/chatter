+++
code = 'E540'
name = 'Duration numeric and separator admission boundaries'

[[example]]
title = 'Clock boundary control'
level = 'header'
claim = 'legal'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Time Duration:	23:59:59
*CHI:	hello .
@End
'''

[[example]]
title = 'Oversized single hour cannot wrap'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Time Duration:	4294967296:59:59
*CHI:	hello .
@End
'''

[[example]]
title = 'Oversized single second cannot wrap'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Time Duration:	23:59:4294967296
*CHI:	hello .
@End
'''

[[example]]
title = 'Oversized millisecond suffix is not admitted'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Time Duration:	23:59:59.4294967296
*CHI:	hello .
@End
'''

[[example]]
title = 'Oversized range start hour cannot wrap'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Time Duration:	4294967296:00-23:59
*CHI:	hello .
@End
'''

[[example]]
title = 'Oversized range end minute cannot wrap'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Time Duration:	00:00-23:4294967296
*CHI:	hello .
@End
'''

[[example]]
title = 'Malformed semicolon endpoint retains its text'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Time Duration:	00:00:00;23:59:4294967296
*CHI:	hello .
@End
'''

[[example]]
title = 'A signed field is not an unsigned clock field'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Time Duration:	+0:00:00
*CHI:	hello .
@End
'''

[[example]]
title = 'A trailing empty segment does not disappear'
level = 'header'
claim = 'violates'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Time Duration:	00:00:00,
*CHI:	hello .
@End
'''

[[example]]
title = 'Malformed first semicolon endpoint retains its text'
level = 'header'
claim = 'violates'
notes = 'Counterpart of the malformed second endpoint: overflow in the first endpoint must not be skipped while retaining the second.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Time Duration:	4294967296:00:00;23:59:59
*CHI:	hello .
@End
'''

[[example]]
title = 'A surplus clock component cannot be silently discarded'
level = 'header'
claim = 'violates'
notes = 'Append :00 to the legal clock boundary control; the result is not an HH:MM:SS value.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Time Duration:	23:59:59:00
*CHI:	hello .
@End
'''
[[example]]
title = 'An empty duration retains the optional-empty policy'
level = 'header'
claim = 'legal'
notes = 'Delete the complete value from the legal 23:59:59 control, keeping its tab separator and following speech. TimeDurationValue::invalidity explicitly exempts empty optional values; the omission is preserved rather than serialized as zero.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Time Duration:	
*CHI:	hello .
@End
'''

[[example]]
title = 'A suffix after a complete duration is not discarded'
level = 'header'
claim = 'violates'
notes = 'Append x to the legal 23:59:59 control; retain the whole unsupported value.'
chat = '''
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|sample|CHI|||||Child|||
@Time Duration:	23:59:59x
*CHI:	hello .
@End
'''
+++

## Description

These authored controls apply the existing duration policy in [E540](E540.md):
unsigned decimal clock components, hours at most 23, minutes and seconds at
most 59, and one supported single-time or hyphen-range shape. The legal control
anchors the upper clock boundary. Oversized values must not wrap or truncate;
signs, millisecond suffixes, semicolon ranges and trailing empty segments do not
become valid by normalization. Numeric admission failure must preserve the
original header text for diagnostics and roundtrip, not manufacture a time.
Both semicolon endpoints are independently subject to numeric admission, and
extra clock fields must not be dropped to recover a valid prefix. These cases
also retain their rejection and original spelling through JSON roundtrip.
An empty value is not evidence of zero elapsed time, and a valid prefix does
not permit a trailing letter. Preserve both fields for correction without
inventing or truncating a duration.

The large decimal value also exceeds a 32-bit unsigned component. That is an
implementation boundary exercised by these cases, not the definition of CHAT's
clock limit. No new CHECK observation or diagnostic-equivalence claim is made.
