+++
code = 'E316'
name = 'Malformed precode and hostless annotation positions'

[[example]]
title = "ordinary healthy structured precode"
level = 'utterance'
claim = 'legal'
chat = """
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	SPK Speaker
@ID:	eng, zho|sample|SPK|||||Speaker|||
@Media:	prelude-controls, audio
@Comment:	Préface café
*SPK:	café opening .
%com:	opening note
*SPK:	+< +^ [- zho] bamboo forest . [+ bch] \u0015100_200\u0015
%com:	combined note
*SPK:	+, [- eng] birch grove ? [+ second] \u0015200_300\u0015
%com:	completion note
*SPK:	[- zho] cedar tree . [+ third] \u0015300_400\u0015
%com:	language note
*SPK:	+^ +< fern garden ! [+ fourth] \u0015400_500\u0015
%com:	linkers note
*SPK:	finch closing .
%com:	closing note
@End
"""

[[example]]
title = "ordinary malformed or hostless precode position \"[- ]\""
level = 'utterance'
claim = 'violates'
chat = """
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	SPK Speaker
@ID:	eng, zho|sample|SPK|||||Speaker|||
@Media:	prelude-controls, audio
@Comment:	Préface café
*SPK:	café opening .
%com:	opening note
*SPK:	+< +^ [- ] bamboo forest . [+ bch] \u0015100_200\u0015
%com:	combined note
*SPK:	+, [- eng] birch grove ? [+ second] \u0015200_300\u0015
%com:	completion note
*SPK:	[- zho] cedar tree . [+ third] \u0015300_400\u0015
%com:	language note
*SPK:	+^ +< fern garden ! [+ fourth] \u0015400_500\u0015
%com:	linkers note
*SPK:	finch closing .
%com:	closing note
@End
"""

[[example]]
title = "ordinary malformed or hostless precode position \"[- ZHO]\""
level = 'utterance'
claim = 'violates'
chat = """
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	SPK Speaker
@ID:	eng, zho|sample|SPK|||||Speaker|||
@Media:	prelude-controls, audio
@Comment:	Préface café
*SPK:	café opening .
%com:	opening note
*SPK:	+< +^ [- ZHO] bamboo forest . [+ bch] \u0015100_200\u0015
%com:	combined note
*SPK:	+, [- eng] birch grove ? [+ second] \u0015200_300\u0015
%com:	completion note
*SPK:	[- zho] cedar tree . [+ third] \u0015300_400\u0015
%com:	language note
*SPK:	+^ +< fern garden ! [+ fourth] \u0015400_500\u0015
%com:	linkers note
*SPK:	finch closing .
%com:	closing note
@End
"""

[[example]]
title = "ordinary malformed or hostless precode position \"[- z ho]\""
level = 'utterance'
claim = 'violates'
chat = """
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	SPK Speaker
@ID:	eng, zho|sample|SPK|||||Speaker|||
@Media:	prelude-controls, audio
@Comment:	Préface café
*SPK:	café opening .
%com:	opening note
*SPK:	+< +^ [- z ho] bamboo forest . [+ bch] \u0015100_200\u0015
%com:	combined note
*SPK:	+, [- eng] birch grove ? [+ second] \u0015200_300\u0015
%com:	completion note
*SPK:	[- zho] cedar tree . [+ third] \u0015300_400\u0015
%com:	language note
*SPK:	+^ +< fern garden ! [+ fourth] \u0015400_500\u0015
%com:	linkers note
*SPK:	finch closing .
%com:	closing note
@End
"""

[[example]]
title = "ordinary malformed or hostless precode position \"[/]\""
level = 'utterance'
claim = 'violates'
chat = """
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	SPK Speaker
@ID:	eng, zho|sample|SPK|||||Speaker|||
@Media:	prelude-controls, audio
@Comment:	Préface café
*SPK:	café opening .
%com:	opening note
*SPK:	+< +^ [/] bamboo forest . [+ bch] \u0015100_200\u0015
%com:	combined note
*SPK:	+, [- eng] birch grove ? [+ second] \u0015200_300\u0015
%com:	completion note
*SPK:	[- zho] cedar tree . [+ third] \u0015300_400\u0015
%com:	language note
*SPK:	+^ +< fern garden ! [+ fourth] \u0015400_500\u0015
%com:	linkers note
*SPK:	finch closing .
%com:	closing note
@End
"""

[[example]]
title = "CA healthy structured precode"
level = 'utterance'
claim = 'legal'
chat = """
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng, zho|sample|SPK|||||Speaker|||
@Media:	prelude-ca-controls, audio
@Comment:	Préface café
*SPK:	café opening .
%com:	opening note
*SPK:	+< +^ [- zho] bamboo forest . [+ bch] \u0015100_200\u0015
%com:	combined note
*SPK:	+, [- eng] birch grove ? [+ second] \u0015200_300\u0015
%com:	completion note
*SPK:	[- zho] cedar tree . [+ third] \u0015300_400\u0015
%com:	language note
*SPK:	+^ +< fern garden ! [+ fourth] \u0015400_500\u0015
%com:	linkers note
*SPK:	finch closing .
%com:	closing note
@End
"""

[[example]]
title = "CA malformed or hostless precode position \"[- ]\""
level = 'utterance'
claim = 'violates'
chat = """
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng, zho|sample|SPK|||||Speaker|||
@Media:	prelude-ca-controls, audio
@Comment:	Préface café
*SPK:	café opening .
%com:	opening note
*SPK:	+< +^ [- ] bamboo forest . [+ bch] \u0015100_200\u0015
%com:	combined note
*SPK:	+, [- eng] birch grove ? [+ second] \u0015200_300\u0015
%com:	completion note
*SPK:	[- zho] cedar tree . [+ third] \u0015300_400\u0015
%com:	language note
*SPK:	+^ +< fern garden ! [+ fourth] \u0015400_500\u0015
%com:	linkers note
*SPK:	finch closing .
%com:	closing note
@End
"""

[[example]]
title = "CA malformed or hostless precode position \"[- ZHO]\""
level = 'utterance'
claim = 'violates'
chat = """
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng, zho|sample|SPK|||||Speaker|||
@Media:	prelude-ca-controls, audio
@Comment:	Préface café
*SPK:	café opening .
%com:	opening note
*SPK:	+< +^ [- ZHO] bamboo forest . [+ bch] \u0015100_200\u0015
%com:	combined note
*SPK:	+, [- eng] birch grove ? [+ second] \u0015200_300\u0015
%com:	completion note
*SPK:	[- zho] cedar tree . [+ third] \u0015300_400\u0015
%com:	language note
*SPK:	+^ +< fern garden ! [+ fourth] \u0015400_500\u0015
%com:	linkers note
*SPK:	finch closing .
%com:	closing note
@End
"""

[[example]]
title = "CA malformed or hostless precode position \"[- z ho]\""
level = 'utterance'
claim = 'violates'
chat = """
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng, zho|sample|SPK|||||Speaker|||
@Media:	prelude-ca-controls, audio
@Comment:	Préface café
*SPK:	café opening .
%com:	opening note
*SPK:	+< +^ [- z ho] bamboo forest . [+ bch] \u0015100_200\u0015
%com:	combined note
*SPK:	+, [- eng] birch grove ? [+ second] \u0015200_300\u0015
%com:	completion note
*SPK:	[- zho] cedar tree . [+ third] \u0015300_400\u0015
%com:	language note
*SPK:	+^ +< fern garden ! [+ fourth] \u0015400_500\u0015
%com:	linkers note
*SPK:	finch closing .
%com:	closing note
@End
"""

[[example]]
title = "CA malformed or hostless precode position \"[/]\""
level = 'utterance'
claim = 'violates'
chat = """
@UTF8
@Begin
@Languages:	eng, zho
@Participants:	SPK Speaker
@Options:	CA
@ID:	eng, zho|sample|SPK|||||Speaker|||
@Media:	prelude-ca-controls, audio
@Comment:	Préface café
*SPK:	café opening .
%com:	opening note
*SPK:	+< +^ [/] bamboo forest . [+ bch] \u0015100_200\u0015
%com:	combined note
*SPK:	+, [- eng] birch grove ? [+ second] \u0015200_300\u0015
%com:	completion note
*SPK:	[- zho] cedar tree . [+ third] \u0015300_400\u0015
%com:	language note
*SPK:	+^ +< fern garden ! [+ fourth] \u0015400_500\u0015
%com:	linkers note
*SPK:	finch closing .
%com:	closing note
@End
"""
+++

## Description

A legal initial linker run does not make an empty, split or uppercase precode well-formed, nor supply a spoken host for a retrace. Native recovered structure governs classification. The deck retains honest recovery rather than inferring a narrower annotation or language code from unparsed text. Neighboring healthy main and dependent tiers and genuinely healthy ending carriers must survive.

## Expected Behavior

Paired controls retain their established acceptance. Each invalid variant reports
its declared diagnostic without dropping healthy source-owned neighboring tiers.
File recovery does not authorize strict fragment acceptance.

## CHAT Rule

A legal initial linker run does not make an empty, split or uppercase precode well-formed, nor supply a spoken host for a retrace. Native recovered structure governs classification. The deck retains honest recovery rather than inferring a narrower annotation or language code from unparsed text. Neighboring healthy main and dependent tiers and genuinely healthy ending carriers must survive.
