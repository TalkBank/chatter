+++
code = 'E758'
name = 'Separator provenance before a combined prelude'

[[example]]
title = "ordinary combined prelude has a canonical separator"
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
title = "ordinary single space before both linkers and language precode"
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
*SPK:	 +< +^ [- zho] bamboo forest . [+ bch] \u0015100_200\u0015
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
title = "ordinary separator padding before both linkers and language precode"
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
*SPK:	  +< +^ [- zho] bamboo forest . [+ bch] \u0015100_200\u0015
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
title = "ordinary second tab is distinct from separator-space padding"
level = 'utterance'
claim = { subsumed_by = 'E370' }
notes = 'Only the first tab is the tier delimiter. The second tab is a structural tab, not separator-space padding; CA does not waive this independent rule.'
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
*SPK:		+< +^ [- zho] bamboo forest . [+ bch] \u0015100_200\u0015
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
title = "CA combined prelude has a canonical separator"
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
title = "CA single space before both linkers and language precode"
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
*SPK:	 +< +^ [- zho] bamboo forest . [+ bch] \u0015100_200\u0015
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
title = "CA separator padding before both linkers and language precode"
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
*SPK:	  +< +^ [- zho] bamboo forest . [+ bch] \u0015100_200\u0015
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
title = "CA second tab is distinct from separator-space padding"
level = 'utterance'
claim = { subsumed_by = 'E370' }
notes = 'Only the first tab is the tier delimiter. The second tab is a structural tab, not separator-space padding; CA does not waive this independent rule.'
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
*SPK:		+< +^ [- zho] bamboo forest . [+ bch] \u0015100_200\u0015
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

Spaces immediately after the main-tier tab remain separator provenance even when linkers and a language precode follow them. The ordinary file reports E758; CA exempts this existing rule. A second tab is instead E370 structural content in either mode, not separator-space padding. A canonical tab has no padding. Semantic serialization may normalize spaces; a legal E758 claim is not a byte-exact roundtrip claim.

## Expected Behavior

Paired controls retain their established acceptance. Each invalid variant reports
its declared diagnostic without dropping healthy source-owned neighboring tiers.
File recovery does not authorize strict fragment acceptance.

## CHAT Rule

Spaces immediately after the main-tier tab remain separator provenance even when linkers and a language precode follow them. The ordinary file reports E758; CA exempts this existing rule. A second tab is instead E370 structural content in either mode, not separator-space padding. A canonical tab has no padding. Semantic serialization may normalize spaces; a legal E758 claim is not a byte-exact roundtrip claim.
