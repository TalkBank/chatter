+++
code = 'E755'
name = 'Language declaration in combined preludes'

[[example]]
title = "ordinary declared language precodes are retained"
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
title = "ordinary undeclared language before bamboo retains prelude and ending"
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
*SPK:	+< +^ [- fra] bamboo forest . [+ bch] \u0015100_200\u0015
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
title = "ordinary declared language change before bamboo is legal"
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
*SPK:	+< +^ [- eng] bamboo forest . [+ bch] \u0015100_200\u0015
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
title = "ordinary undeclared language before birch retains prelude and ending"
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
*SPK:	+, [- fra] birch grove ? [+ second] \u0015200_300\u0015
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
title = "ordinary declared language change before birch is legal"
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
*SPK:	+, [- zho] birch grove ? [+ second] \u0015200_300\u0015
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
title = "ordinary undeclared language before cedar retains prelude and ending"
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
*SPK:	[- fra] cedar tree . [+ third] \u0015300_400\u0015
%com:	language note
*SPK:	+^ +< fern garden ! [+ fourth] \u0015400_500\u0015
%com:	linkers note
*SPK:	finch closing .
%com:	closing note
@End
"""

[[example]]
title = "ordinary declared language change before cedar is legal"
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
*SPK:	[- eng] cedar tree . [+ third] \u0015300_400\u0015
%com:	language note
*SPK:	+^ +< fern garden ! [+ fourth] \u0015400_500\u0015
%com:	linkers note
*SPK:	finch closing .
%com:	closing note
@End
"""

[[example]]
title = "CA declared language precodes are retained"
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
title = "CA undeclared language before bamboo retains prelude and ending"
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
*SPK:	+< +^ [- fra] bamboo forest . [+ bch] \u0015100_200\u0015
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
title = "CA declared language change before bamboo is legal"
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
*SPK:	+< +^ [- eng] bamboo forest . [+ bch] \u0015100_200\u0015
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
title = "CA undeclared language before birch retains prelude and ending"
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
*SPK:	+, [- fra] birch grove ? [+ second] \u0015200_300\u0015
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
title = "CA declared language change before birch is legal"
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
*SPK:	+, [- zho] birch grove ? [+ second] \u0015200_300\u0015
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
title = "CA undeclared language before cedar retains prelude and ending"
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
*SPK:	[- fra] cedar tree . [+ third] \u0015300_400\u0015
%com:	language note
*SPK:	+^ +< fern garden ! [+ fourth] \u0015400_500\u0015
%com:	linkers note
*SPK:	finch closing .
%com:	closing note
@End
"""

[[example]]
title = "CA declared language change before cedar is legal"
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
*SPK:	[- eng] cedar tree . [+ third] \u0015300_400\u0015
%com:	language note
*SPK:	+^ +< fern garden ! [+ fourth] \u0015400_500\u0015
%com:	linkers note
*SPK:	finch closing .
%com:	closing note
@End
"""
+++

## Description

The header declares eng and zho. Switching an admitted whole-utterance precode to fra changes only that source-owned token and violates the existing declaration requirement. Switching between the declared languages is legal. E755 is located at the owning main-tier start, not at the precode token; word-level suffix policy is unchanged.

## Expected Behavior

Paired controls retain their established acceptance. Each invalid variant reports
its declared diagnostic without dropping healthy source-owned neighboring tiers.
File recovery does not authorize strict fragment acceptance.

## CHAT Rule

The header declares eng and zho. Switching an admitted whole-utterance precode to fra changes only that source-owned token and violates the existing declaration requirement. Switching between the declared languages is legal. E755 is located at the owning main-tier start, not at the precode token; word-level suffix policy is unchanged.
