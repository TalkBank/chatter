+++
code = 'E766'
name = 'Linker placement with language precodes'

[[example]]
title = "ordinary initial linker run precedes the language precode"
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
title = "ordinary misplaced linker after bamboo leaves the prelude intact"
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
*SPK:	+< +^ [- zho] bamboo +< forest . [+ bch] \u0015100_200\u0015
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
title = "ordinary misplaced linker after birch leaves the prelude intact"
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
*SPK:	+< +^ [- zho] bamboo forest . [+ bch] \u0015100_200\u0015
%com:	combined note
*SPK:	+, [- eng] birch +< grove ? [+ second] \u0015200_300\u0015
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
title = "ordinary misplaced linker after cedar leaves the prelude intact"
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
*SPK:	+< +^ [- zho] bamboo forest . [+ bch] \u0015100_200\u0015
%com:	combined note
*SPK:	+, [- eng] birch grove ? [+ second] \u0015200_300\u0015
%com:	completion note
*SPK:	[- zho] cedar +< tree . [+ third] \u0015300_400\u0015
%com:	language note
*SPK:	+^ +< fern garden ! [+ fourth] \u0015400_500\u0015
%com:	linkers note
*SPK:	finch closing .
%com:	closing note
@End
"""

[[example]]
title = "ordinary misplaced linker after fern leaves the prelude intact"
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
*SPK:	+< +^ [- zho] bamboo forest . [+ bch] \u0015100_200\u0015
%com:	combined note
*SPK:	+, [- eng] birch grove ? [+ second] \u0015200_300\u0015
%com:	completion note
*SPK:	[- zho] cedar tree . [+ third] \u0015300_400\u0015
%com:	language note
*SPK:	+^ +< fern +< garden ! [+ fourth] \u0015400_500\u0015
%com:	linkers note
*SPK:	finch closing .
%com:	closing note
@End
"""

[[example]]
title = "ordinary linker after the language precode is not in the initial linker field"
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
*SPK:	+< +^ [- zho] +< bamboo forest . [+ bch] \u0015100_200\u0015
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
title = "ordinary changed initial linker remains a prelude token"
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
*SPK:	+^ +^ [- zho] bamboo forest . [+ bch] \u0015100_200\u0015
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
title = "CA initial linker run precedes the language precode"
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
title = "CA misplaced linker after bamboo leaves the prelude intact"
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
*SPK:	+< +^ [- zho] bamboo +< forest . [+ bch] \u0015100_200\u0015
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
title = "CA misplaced linker after birch leaves the prelude intact"
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
*SPK:	+< +^ [- zho] bamboo forest . [+ bch] \u0015100_200\u0015
%com:	combined note
*SPK:	+, [- eng] birch +< grove ? [+ second] \u0015200_300\u0015
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
title = "CA misplaced linker after cedar leaves the prelude intact"
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
*SPK:	+< +^ [- zho] bamboo forest . [+ bch] \u0015100_200\u0015
%com:	combined note
*SPK:	+, [- eng] birch grove ? [+ second] \u0015200_300\u0015
%com:	completion note
*SPK:	[- zho] cedar +< tree . [+ third] \u0015300_400\u0015
%com:	language note
*SPK:	+^ +< fern garden ! [+ fourth] \u0015400_500\u0015
%com:	linkers note
*SPK:	finch closing .
%com:	closing note
@End
"""

[[example]]
title = "CA misplaced linker after fern leaves the prelude intact"
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
*SPK:	+< +^ [- zho] bamboo forest . [+ bch] \u0015100_200\u0015
%com:	combined note
*SPK:	+, [- eng] birch grove ? [+ second] \u0015200_300\u0015
%com:	completion note
*SPK:	[- zho] cedar tree . [+ third] \u0015300_400\u0015
%com:	language note
*SPK:	+^ +< fern +< garden ! [+ fourth] \u0015400_500\u0015
%com:	linkers note
*SPK:	finch closing .
%com:	closing note
@End
"""

[[example]]
title = "CA linker after the language precode is not in the initial linker field"
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
*SPK:	+< +^ [- zho] +< bamboo forest . [+ bch] \u0015100_200\u0015
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
title = "CA changed initial linker remains a prelude token"
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
*SPK:	+^ +^ [- zho] bamboo forest . [+ bch] \u0015100_200\u0015
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

An initial run of linkers precedes the optional language precode. A linker after spoken content or after the precode is outside that run and is rejected as a misplaced typed linker. The surrounding words, healthy ending and unaffected initial linker tokens remain source-owned. No strict-linker cross-utterance policy is enabled.

## Expected Behavior

Paired controls retain their established acceptance. Each invalid variant reports
its declared diagnostic without dropping healthy source-owned neighboring tiers.
File recovery does not authorize strict fragment acceptance.

## CHAT Rule

An initial run of linkers precedes the optional language precode. A linker after spoken content or after the precode is outside that run and is rejected as a misplaced typed linker. The surrounding words, healthy ending and unaffected initial linker tokens remain source-owned. No strict-linker cross-utterance policy is enabled.
