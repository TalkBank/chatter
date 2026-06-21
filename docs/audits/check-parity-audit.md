# CHECK Parity Audit (CLAN CHECK vs TalkBank)

Reference: `clan-check-reference/check-error-codes.json`, generated from `check.cpp` by `scripts/extract_check_codes.py` (every code CHECK actually emits, not the stale `CHECK-rules.md` subset).

## Executive Summary

- CHECK rules parsed: `153`
- Overlap with TalkBank codes: `90`
- CHECK rules missing direct TalkBank mapping: `63`
- Semantic parity `full`: `90`
- Behavioral parity `full`: `77`
- Intentional divergence (semantic full + behavioral partial due to CHECK anomalies): `13`
- TalkBank enhancements beyond CHECK (no mapped CHECK rule): `133`

## Method

- Loaded every emitted CHECK code (n_call_sites > 0) from the generated `check-error-codes.json`.
- Mapped CHECK rules to TalkBank codes via explicit ID mapping plus keyword fallback.
- Reported two parity dimensions:
  - `semantic`: intended rule meaning parity.
  - `behavioral`: literal CHECK runtime behavior parity (including documented anomalies).
- Strictness policy: TalkBank should be at least as strict semantically.

## Master Mapping (CHECK -> TalkBank)

| CHECK # | CHECK Message | Category | TalkBank Codes | Semantic | Behavioral | Strictness | Divergence | Action | Priority |
|---:|---|---|---|---|---|---|---|---|---|
| 1 | Expected characters are: @ or %% or *. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 2 | Missing ':' character and argument. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 3 | Missing either TAB or SPACE character. | check.cpp (generated reference) | `E243`, `W210`, `W211` | full | full | equal | none | no action | P3 |
| 4 | Found a space character instead of TAB character after Tier name / Found a space character...... / Please run "chstring +q +1" command on this file to fix this error. | check.cpp (generated reference) | `E243`, `W210`, `W211` | full | full | equal | none | no action | P3 |
| 5 | Colon (:) character is illegal. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 6 | "@Begin" is missing at the beginning of the file. | check.cpp (generated reference) | `E501` | full | full | equal | none | no action | P3 |
| 7 | "@End" is missing at the end of the file. | check.cpp (generated reference) | `E502` | full | full | equal | none | no action | P3 |
| 8 | Expected characters are: @ %% * TAB. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 9 | Tier name is longer than %d. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 10 | Tier text is longer than %ld. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 11 | Symbol is not declared in the depfile. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 12 | Missing speaker name and/or role. | check.cpp (generated reference) | `E308`, `E522`, `E532` | full | full | equal | none | no action | P3 |
| 13 | Duplicate speaker declaration. | check.cpp (generated reference) | `E308`, `E522`, `E532` | full | full | equal | none | no action | P3 |
| 14 | Spaces before tier code. | check.cpp (generated reference) | `E243`, `W210`, `W211` | full | full | equal | none | no action | P3 |
| 15 | Illegal role. Please see "depfile.cut" for list of roles. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 17 | Tier is not declared in depfile file. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 18 | Speaker / is not specified in a participants list. | check.cpp (generated reference) | `E308`, `E522` | full | full | equal | none | no action | P3 |
| 19 | Illegal use of delimiter in a word. / Or a SPACE should be added after it. | check.cpp (generated reference) | `E243`, `E304`, `E305`, `E360`, `W210`, `W211` | full | full | equal | none | no action | P3 |
| 20 | Undeclared suffix in depfile. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 21 | Utterance delimiter expected. | check.cpp (generated reference) | `E304` | full | full | equal | none | no action | P3 |
| 22 | Unmatched [ found on the tier. | check.cpp (generated reference) | `E375` | full | partial | TalkBank stricter | intentional | no action | P2 |
| 23 | Unmatched ] found on the tier. | check.cpp (generated reference) | `E346` | full | partial | TalkBank stricter | intentional | no action | P2 |
| 24 | Unmatched < found on the tier. | check.cpp (generated reference) | `E347` | full | partial | TalkBank stricter | intentional | no action | P2 |
| 25 | Unmatched > found on the tier. | check.cpp (generated reference) | `E348` | full | partial | TalkBank stricter | intentional | no action | P2 |
| 26 | Unmatched { found on the tier. | check.cpp (generated reference) | `E230`, `E231`, `E242`, `E346`, `E356`, `E357` | full | partial | TalkBank stricter | intentional | no action | P2 |
| 27 | Unmatched } found on the tier. | check.cpp (generated reference) | `E230`, `E231`, `E242`, `E346`, `E356`, `E357` | full | partial | TalkBank stricter | intentional | no action | P2 |
| 28 | Unmatched ( found on the tier. | check.cpp (generated reference) | `E230`, `E231`, `E242`, `E346`, `E356`, `E357` | full | full | equal | none | no action | P3 |
| 29 | Unmatched ) found on the tier. | check.cpp (generated reference) | `E230`, `E231`, `E242`, `E346`, `E356`, `E357` | full | full | equal | none | no action | P3 |
| 30 | Text is illegal. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 31 | Missing text after the colon. | check.cpp (generated reference) | `E305` | full | full | equal | none | no action | P3 |
| 32 | Code is not declared in depfile. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 33 | Either illegal date or time or symbol is not declared in depfile. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 34 | Illegal date representation. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 35 | Illegal time representation. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 36 | Utterance delimiter must be at the end of the utterance. / Use "fixit" program to break up this tier. | check.cpp (generated reference) | `E305` | full | full | equal | none | no action | P3 |
| 37 | Undeclared prefix. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 38 | Numbers should be written out in words. | check.cpp (generated reference) | `E220` | full | full | equal | none | no action | P3 |
| 42 | Use either "&" or "()", but not both. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 43 | The file must start with "@Begin" tier. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 44 | The file must end with "@End" tier. / Possibly there are some blank lines at the end of the file. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 45 | There were more @Bg than @Eg tiers found. | check.cpp (generated reference) | `E702`, `E705`, `E706`, `E720` | full | full | equal | none | no action | P3 |
| 46 | This @Eg does not have matching @Bg. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 47 | Numbers are not allowed inside words. | check.cpp (generated reference) | `E220` | full | full | equal | none | no action | P3 |
| 48 | Illegal character(s) found. / Illegal character(s) '%s' found. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 49 | Upper case letters are not allowed inside a word. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 50 | Redundant utterance delimiter. | check.cpp (generated reference) | `E305` | full | full | equal | none | no action | P3 |
| 51 | expected [ ]; < > should be followed by [ ] | check.cpp (generated reference) | `E347`, `E348` | full | full | equal | none | no action | P3 |
| 52 | This item must be preceded by text. / Item '%s' must be preceded by text. | check.cpp (generated reference) | `E370` | full | full | equal | none | no action | P3 |
| 53 | Only one "@Begin" can be in a file. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 54 | Only one "@End" can be in a file. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 55 | Unmatched ( found in the word. | check.cpp (generated reference) | `E231` | full | full | equal | none | no action | P3 |
| 56 | Unmatched ) found in the word. | check.cpp (generated reference) | `E231` | full | full | equal | none | no action | P3 |
| 57 | Please add space between word and pause symbol. / Please add space between word and pause symbol: '%s'. | check.cpp (generated reference) | `E243` | full | full | equal | none | no action | P3 |
| 59 | Expected second %c character. / Expected second %s character. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 60 | "@ID:" tier is missing in the file. Please run "insert" in Commands window on this data file. | check.cpp (generated reference) | `E522` | full | full | equal | none | no action | P3 |
| 61 | "@Participants:" tier is expected here. | check.cpp (generated reference) | `E522`, `E523`, `E524` | full | full | equal | none | no action | P3 |
| 62 | Missing language information. | check.cpp (generated reference) | `E248`, `E249`, `E519` | full | full | equal | none | no action | P3 |
| 63 | Missing Corpus name. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 64 | Wrong gender information (Choose: female or male). | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 65 | This item can not be followed by the next symbol. / Item '%s' can not be followed by the next symbol. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 66 | Illegal character in a word. / Illegal character '%s' in a word. / Or a SPACE should be added before it. | check.cpp (generated reference) | `E243`, `W210`, `W211` | full | full | equal | none | no action | P3 |
| 67 | This item must be followed by text, / Item '%s' must be followed by text, / preceded by SPACE or be removed. | check.cpp (generated reference) | `E243`, `W210`, `W211` | full | full | equal | none | no action | P3 |
| 68 | PARTICIPANTS TIER IS MISSING "CHI Target_Child". | check.cpp (generated reference) | `E522`, `E523`, `E524` | full | full | equal | none | no action | P3 |
| 69 | The UTF8 header is missing. If you edit and save the file, it will be inserted. | check.cpp (generated reference) | `E507` | full | full | equal | none | no action | P3 |
| 70 | Expected either text or "0" on this tier. | check.cpp (generated reference) | `E253` | full | full | equal | none | no action | P3 |
| 71 | This item must be before pause (#). / Item '%s' must be before pause (#). | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 72 | This item must precede the utterance delimiter or CA delimiter. / Item '%s' must precede the utterance delimiter or CA delimiter. | check.cpp (generated reference) | `E304`, `E305`, `E360` | full | full | equal | none | no action | P3 |
| 73 | This item must be preceded by text or '0'. / Item '%s' must be preceded by text or '0'. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 75 | This item must follow after utterance delimiter. / Item '%s' must follow after utterance delimiter. | check.cpp (generated reference) | `E304`, `E305`, `E360` | full | full | equal | none | no action | P3 |
| 76 | Only one letter is allowed with '@l'. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 77 | "@Languages:" tier is expected here. | check.cpp (generated reference) | `E248`, `E249`, `E519` | full | full | equal | none | no action | P3 |
| 78 | This item must be used at the beginning of tier. / Item '%s' must be used at the beginning of tier. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 79 | Only one occurrence of \| symbol per word is allowed. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 80 | There must be at least one occurrence of '\|'. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 81 | Bullet must follow utterance delimiter or be followed by end-of-line. | check.cpp (generated reference) | `E360` | full | full | equal | none | no action | P3 |
| 82 | BEG mark of bullet must be smaller than END mark. | check.cpp (generated reference) | `E361` | full | full | equal | none | no action | P3 |
| 83 | Current BEG time is smaller than previous' tier BEG time | check.cpp (generated reference) | `E362`, `E701` | full | full | equal | none | no action | P3 |
| 84 | Current BEG time is smaller than previous' tier END time by %ld msec. | check.cpp (generated reference) | `E704` | full | full | equal | none | no action | P3 |
| 85 | Gap found between current BEG time and previous' tier END time. | check.cpp (generated reference) | `E700` | full | full | equal | none | no action | P3 |
| 86 | Illegal character. Please re-enter it using Unicode standard. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 87 | Malformed structure. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 88 | Illegal use of compounds and special form markers. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 89 | Missing or extra or wrong characters found in bullet. | check.cpp (generated reference) | `E360`, `E361` | full | full | equal | none | no action | P3 |
| 90 | Illegal time representation inside a bullet. | check.cpp (generated reference) | `E360`, `E361` | full | full | equal | none | no action | P3 |
| 91 | Blank lines are not allowed. | check.cpp (generated reference) | `E303` | full | full | equal | none | no action | P3 |
| 92 | This item must be followed by space or end-of-line. / Item '%s' must be followed by space or end-of-line. | check.cpp (generated reference) | `E243`, `W210`, `W211` | full | full | equal | none | no action | P3 |
| 93 | This item must be preceded by SPACE. / Item '%s' must be preceded by SPACE. | check.cpp (generated reference) | `E243`, `W210`, `W211` | full | full | equal | none | no action | P3 |
| 94 | Mismatch of speaker and %%mor: utterance delimiters. | check.cpp (generated reference) | `E705`, `E706`, `E714`, `E715`, `E718`, `E719`, `E720` | full | full | equal | none | no action | P3 |
| 95 | Illegal use of capitalized words in compounds. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 96 | Word color is now illegal. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 97 | Illegal character inside parentheses. | check.cpp (generated reference) | `E212`, `E231` | full | full | equal | none | no action | P3 |
| 98 | Space is not allow in media file name inside bullets. | check.cpp (generated reference) | `E243`, `E360`, `E361`, `E362`, `E701`, `E704`, `W210`, `W211` | full | full | equal | none | no action | P3 |
| 99 | Extension is not allow at the end of media file name. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 100 | Commas at the end of PARTICIPANTS tier are not allowed. | check.cpp (generated reference) | `E522`, `E523`, `E524` | full | full | equal | none | no action | P3 |
| 101 | This item must be followed or preceded by text. / Item '%s' must be followed or preceded by text. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 102 | Italic markers are no longer legal in CHAT. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 103 | Illegal use of both CA and IPA on "@Options:" tier. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 104 | Please select "CAfont" or "Ascender Uni Duo" font for CA file as per "@Options:" tier. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 105 | Please select "Charis SIL" font for IPA file as per "@Options:" tier. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 106 | The whole code must be on one line. Please run chstring +q on this file. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 107 | Only single commas are allowed in tier. | check.cpp (generated reference) | `E258` | full | full | equal | none | no action | P3 |
| 108 | All postcodes must precede final bullet. | check.cpp (generated reference) | `E360`, `E361`, `E362`, `E701`, `E704` | full | full | equal | none | no action | P3 |
| 109 | Postcodes are not allowed on dependent tiers. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 110 | No bullet found on this tier. | check.cpp (generated reference) | `E360` | full | full | equal | none | no action | P3 |
| 111 | Illegal pause format. Pause has to have '.' / Pause needs '.' in '%s' or this item is in wrong location. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 112 | Missing %s tier with media file name in headers section at the top of the file. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 113 | Illegal keyword, use "audio", "video" or look in depfile.cut. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 114 | Add "audio", "video" or look in depfile.cut for more keywords after the media file name on %s tier. | check.cpp (generated reference) | `E702`, `E705`, `E706`, `E720` | full | full | equal | none | no action | P3 |
| 115 | Old bullets format found. Please run "fixbullets" program to fix this data. | check.cpp (generated reference) | `E360`, `E361`, `E362`, `E701`, `E704`, `E708`, `E709`, `E710`, `E712`, `E713`, `E720` | full | full | equal | none | no action | P3 |
| 116 | Specifying Font for individual lines is illegal. Please open this file and save it again. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 117 | This character must be used in pairs. See if any are unmatched. / Character %s must be used in pairs. See if any are unmatched. | check.cpp (generated reference) | `E230`, `E356`, `E357` | full | partial | TalkBank stricter | intentional | no action | P2 |
| 118 | Utterance delimiter must precede final bullet. | check.cpp (generated reference) | `E360` | full | full | equal | none | no action | P3 |
| 119 | Missing word after code / Missing word after code "%s" | check.cpp (generated reference) | `E370` | full | full | equal | none | no action | P3 |
| 120 | Please use three letter language code. / Please use "%s" language code instead. / Or see if "fixlang" CLAN command in commands window can fix codes automaticaly. | check.cpp (generated reference) | `E248` | full | full | equal | none | no action | P3 |
| 121 | Language code not found in CLAN/lib/fixes/ISO-639.cut file. / Language code "%s" not found in "CLAN/lib/fixes/ISO-639.cut" file. / If it is a legal code, then please add it to "CLAN/lib/fixes/ISO-639.cut" file. | check.cpp (generated reference) | `E519` | full | full | equal | none | no action | P3 |
| 123 | Illegal character found in tier text. If it CA, then add "@Options: CA" / Illegal character '%s' found in tier text. If it CA, then add "@Options: CA" | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 124 | Please remove "unlinked" from @Media header. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 125 | "@Options" header must immediately follow "@Participants:" header. | check.cpp (generated reference) | `E522`, `E523`, `E524` | full | full | equal | none | no action | P3 |
| 126 | "@ID" header must immediately follow "@Participants:" or "@Options" header. | check.cpp (generated reference) | `E505`, `E517`, `E519`, `E522`, `E523`, `E524` | full | full | equal | none | no action | P3 |
| 127 | Header must follow "@ID:" or "@Birth of" or "@Birthplace of" or "@L1 of" header. | check.cpp (generated reference) | `E505`, `E517`, `E519`, `E522`, `E523`, `E524` | full | full | equal | none | no action | P3 |
| 128 | Unmatched ‹ found on the tier. | check.cpp (generated reference) | `E316` | full | partial | TalkBank stricter | intentional | no action | P2 |
| 129 | Unmatched › found on the tier. | check.cpp (generated reference) | `E346` | full | partial | TalkBank stricter | intentional | no action | P2 |
| 130 | Unmatched 〔 found on the tier. | check.cpp (generated reference) | `E316` | full | partial | TalkBank stricter | intentional | no action | P2 |
| 131 | Unmatched 〕 found on the tier. | check.cpp (generated reference) | `E346` | full | partial | TalkBank stricter | intentional | no action | P2 |
| 132 | Tabs should only be used to mark the beginning of lines. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 133 | BEG time is smaller than same speaker's previous END time by %ld msec. | check.cpp (generated reference) | `E308`, `E522`, `E532` | full | full | equal | none | no action | P3 |
| 134 | This item is illegal. Please run "mor" command on this data. / Item '%s' is illegal. Please run "mor" command on this data. | check.cpp (generated reference) | `E702`, `E705`, `E706`, `E720` | full | full | equal | none | no action | P3 |
| 135 | This item is illegal. / Item '%s' is illegal. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 136 | Unmatched “ found on the tier. | check.cpp (generated reference) | `E242` | full | partial | TalkBank stricter | intentional | no action | P2 |
| 137 | Unmatched ” found on the tier. | check.cpp (generated reference) | `E242` | full | partial | TalkBank stricter | intentional | no action | P2 |
| 138 | Special quote U2019 must be replaced by single quote ('). | check.cpp (generated reference) | `E256` | full | full | equal | none | no action | P3 |
| 139 | Special quote U2018 must be replaced by single quote ('). | check.cpp (generated reference) | `E256` | full | full | equal | none | no action | P3 |
| 140 | Tier "%%MOR:" does not link in size to its speaker tier. | check.cpp (generated reference) | `E401`, `E705`, `E706`, `E720` | full | full | equal | none | no action | P3 |
| 141 | [: ...] has to be preceded by only one word and nothing else. / '%s' must be preceded by only one word and nothing else. | check.cpp (generated reference) | `E387`, `E388`, `E389` | full | full | equal | none | no action | P3 |
| 142 | Speaker's role on @ID tier does not match role on @Participants: tier. | check.cpp (generated reference) | `E532` | full | full | equal | none | no action | P3 |
| 143 | The @ID line needs 10 fields. | check.cpp (generated reference) | `E505` | full | full | equal | none | no action | P3 |
| 144 | Either illegal SES field value or symbol is not declared in depfile. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 145 | This intonational marker should be outside paired markers. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 146 | The &= symbol must include some code after '=' character. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 147 | Undeclared special form marker in depfile. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 148 | Space character is not allowed before comma(,) character on "@Media:" header. | check.cpp (generated reference) | `E243`, `W210`, `W211` | full | full | equal | none | no action | P3 |
| 149 | Illegal character located between a word and [...] code. / Illegal character '%s' located between a word and [...] code. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 150 | Illegal item located between a word and [...] code. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 151 | This word has only repetition segments. | check.cpp (generated reference) | `E370` | full | full | equal | none | no action | P3 |
| 153 | Age's month or day are missing initial zero. Please run "chstring +q +1" command on this file to fix this error. | check.cpp (generated reference) | `E517` | full | full | equal | none | no action | P3 |
| 154 | Please add "unlinked" to @Media header. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 155 | Please use "0word" instead of "(word)". / Please use "0%s" instead of "(%s)". | check.cpp (generated reference) | `E212` | full | full | equal | none | no action | P3 |
| 156 | Please replace ,, with F2-t („) character. | check.cpp (generated reference) | `E243` | full | full | equal | none | no action | P3 |
| 157 | Media file name has to match datafile name. | check.cpp (generated reference) | None | none | none | TalkBank looser | bug-risk | add rule | P1 |
| 158 | [: ...] has to have real word, not 0... or &... or xxx. | check.cpp (generated reference) | `E391` | full | full | equal | none | no action | P3 |
| 159 | Pause markers should appear after retrace markers. | check.cpp (generated reference) | `E370` | full | full | equal | none | no action | P3 |
| 160 | Space character is not allowed after '<' or before '>' character. | check.cpp (generated reference) | `E243`, `W210`, `W211` | full | full | equal | none | no action | P3 |
| 161 | Space character is required before '[' code item. | check.cpp (generated reference) | `E243`, `W210`, `W211` | full | full | equal | none | no action | P3 |

## Gaps: CHECK Rules Missing in TalkBank

- CHECK `1`: Expected characters are: @ or %% or *. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `2`: Missing ':' character and argument. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `5`: Colon (:) character is illegal. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `8`: Expected characters are: @ %% * TAB. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `9`: Tier name is longer than %d. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `10`: Tier text is longer than %ld. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `11`: Symbol is not declared in the depfile. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `15`: Illegal role. Please see "depfile.cut" for list of roles. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `17`: Tier is not declared in depfile file. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `20`: Undeclared suffix in depfile. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `30`: Text is illegal. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `32`: Code is not declared in depfile. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `33`: Either illegal date or time or symbol is not declared in depfile. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `34`: Illegal date representation. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `35`: Illegal time representation. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `37`: Undeclared prefix. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `42`: Use either "&" or "()", but not both. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `43`: The file must start with "@Begin" tier. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `44`: The file must end with "@End" tier. / Possibly there are some blank lines at the end of the file. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `46`: This @Eg does not have matching @Bg. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `48`: Illegal character(s) found. / Illegal character(s) '%s' found. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `49`: Upper case letters are not allowed inside a word. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `53`: Only one "@Begin" can be in a file. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `54`: Only one "@End" can be in a file. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `59`: Expected second %c character. / Expected second %s character. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `63`: Missing Corpus name. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `64`: Wrong gender information (Choose: female or male). (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `65`: This item can not be followed by the next symbol. / Item '%s' can not be followed by the next symbol. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `71`: This item must be before pause (#). / Item '%s' must be before pause (#). (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `73`: This item must be preceded by text or '0'. / Item '%s' must be preceded by text or '0'. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `76`: Only one letter is allowed with '@l'. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `78`: This item must be used at the beginning of tier. / Item '%s' must be used at the beginning of tier. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `79`: Only one occurrence of | symbol per word is allowed. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `80`: There must be at least one occurrence of '|'. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `86`: Illegal character. Please re-enter it using Unicode standard. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `87`: Malformed structure. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `88`: Illegal use of compounds and special form markers. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `95`: Illegal use of capitalized words in compounds. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `96`: Word color is now illegal. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `99`: Extension is not allow at the end of media file name. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `101`: This item must be followed or preceded by text. / Item '%s' must be followed or preceded by text. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `102`: Italic markers are no longer legal in CHAT. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `103`: Illegal use of both CA and IPA on "@Options:" tier. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `104`: Please select "CAfont" or "Ascender Uni Duo" font for CA file as per "@Options:" tier. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `105`: Please select "Charis SIL" font for IPA file as per "@Options:" tier. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `106`: The whole code must be on one line. Please run chstring +q on this file. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `109`: Postcodes are not allowed on dependent tiers. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `111`: Illegal pause format. Pause has to have '.' / Pause needs '.' in '%s' or this item is in wrong location. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `112`: Missing %s tier with media file name in headers section at the top of the file. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `113`: Illegal keyword, use "audio", "video" or look in depfile.cut. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `116`: Specifying Font for individual lines is illegal. Please open this file and save it again. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `123`: Illegal character found in tier text. If it CA, then add "@Options: CA" / Illegal character '%s' found in tier text. If it CA, then add "@Options: CA" (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `124`: Please remove "unlinked" from @Media header. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `132`: Tabs should only be used to mark the beginning of lines. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `135`: This item is illegal. / Item '%s' is illegal. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `144`: Either illegal SES field value or symbol is not declared in depfile. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `145`: This intonational marker should be outside paired markers. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `146`: The &= symbol must include some code after '=' character. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `147`: Undeclared special form marker in depfile. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `149`: Illegal character located between a word and [...] code. / Illegal character '%s' located between a word and [...] code. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `150`: Illegal item located between a word and [...] code. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `154`: Please add "unlinked" to @Media header. (`check.cpp (generated reference)`) -> action: `add rule` (P1)
- CHECK `157`: Media file name has to match datafile name. (`check.cpp (generated reference)`) -> action: `add rule` (P1)

## Intentional Divergences (Behavioral Mismatch, Semantic Match)

- CHECK `22` Unmatched [ found on the tier. -> TalkBank E375. Rationale: CHECK rule is known to have counter/toggle anomaly; TalkBank should match semantic intent, not flawed literal behavior.
- CHECK `23` Unmatched ] found on the tier. -> TalkBank E346. Rationale: CHECK rule is known to have counter/toggle anomaly; TalkBank should match semantic intent, not flawed literal behavior.
- CHECK `24` Unmatched < found on the tier. -> TalkBank E347. Rationale: CHECK rule is known to have counter/toggle anomaly; TalkBank should match semantic intent, not flawed literal behavior.
- CHECK `25` Unmatched > found on the tier. -> TalkBank E348. Rationale: CHECK rule is known to have counter/toggle anomaly; TalkBank should match semantic intent, not flawed literal behavior.
- CHECK `26` Unmatched { found on the tier. -> TalkBank E230, E231, E242, E346, E356, E357. Rationale: CHECK rule is known to have counter/toggle anomaly; TalkBank should match semantic intent, not flawed literal behavior.
- CHECK `27` Unmatched } found on the tier. -> TalkBank E230, E231, E242, E346, E356, E357. Rationale: CHECK rule is known to have counter/toggle anomaly; TalkBank should match semantic intent, not flawed literal behavior.
- CHECK `117` This character must be used in pairs. See if any are unmatched. / Character %s must be used in pairs. See if any are unmatched. -> TalkBank E230, E356, E357. Rationale: CHECK rule is known to have counter/toggle anomaly; TalkBank should match semantic intent, not flawed literal behavior.
- CHECK `128` Unmatched ‹ found on the tier. -> TalkBank E316. Rationale: CHECK rule is known to have counter/toggle anomaly; TalkBank should match semantic intent, not flawed literal behavior.
- CHECK `129` Unmatched › found on the tier. -> TalkBank E346. Rationale: CHECK rule is known to have counter/toggle anomaly; TalkBank should match semantic intent, not flawed literal behavior.
- CHECK `130` Unmatched 〔 found on the tier. -> TalkBank E316. Rationale: CHECK rule is known to have counter/toggle anomaly; TalkBank should match semantic intent, not flawed literal behavior.
- CHECK `131` Unmatched 〕 found on the tier. -> TalkBank E346. Rationale: CHECK rule is known to have counter/toggle anomaly; TalkBank should match semantic intent, not flawed literal behavior.
- CHECK `136` Unmatched “ found on the tier. -> TalkBank E242. Rationale: CHECK rule is known to have counter/toggle anomaly; TalkBank should match semantic intent, not flawed literal behavior.
- CHECK `137` Unmatched ” found on the tier. -> TalkBank E242. Rationale: CHECK rule is known to have counter/toggle anomaly; TalkBank should match semantic intent, not flawed literal behavior.

## TalkBank Enhancements Beyond CHECK

- `E001` `InternalError`
- `E002` `TestError`
- `E003` `EmptyString`
- `E101` `InvalidLineFormat`
- `E301` `MissingMainTier`
- `E302` `MissingNode`
- `E306` `EmptyUtterance`
- `E307` `InvalidSpeaker`
- `E309` `UnexpectedSyntax`
- `E310` `ParseFailed`
- `E311` `UnexpectedNode`
- `E312` `UnclosedBracket`
- `E313` `UnclosedParenthesis`
- `E314` `IncompleteAnnotation`
- `E315` `InvalidControlCharacter`
- `E319` `UnparsableLine`
- `E320` `UnparsableHeader`
- `E321` `UnparsableUtterance`
- `E322` `EmptyColon`
- `E323` `MissingColonAfterSpeaker`
- `E324` `UnrecognizedUtteranceError`
- `E325` `UnexpectedUtteranceChild`
- `E326` `UnexpectedLineType`
- `E330` `TreeParsingError`
- `E331` `UnexpectedNodeInContext`
- `E340` `UnknownBaseContent`
- `E341` `UnbalancedQuotationCrossUtterance`
- `E342` `MissingRequiredElement`
- `E344` `InvalidContentAnnotationNesting`
- `E351` `MissingQuoteBegin`
- `E352` `MissingQuoteEnd`
- `E353` `MissingOtherCompletionContext`
- `E354` `MissingTrailingOffTerminator`
- `E355` `InterleavedContentAnnotations`
- `E358` `UnmatchedLongFeatureBegin`
- `E359` `UnmatchedLongFeatureEnd`
- `E363` `InvalidPostcode`
- `E364` `MalformedWordContent`
- `E365` `MalformedTierContent`
- `E366` `LongFeatureLabelMismatch`
- `E367` `UnmatchedNonvocalBegin`
- `E368` `UnmatchedNonvocalEnd`
- `E369` `NonvocalLabelMismatch`
- `E371` `PauseInPhoGroup`
- `E372` `NestedQuotation`
- `E373` `InvalidOverlapIndex`
- `E376` `ReplacementParseError`
- `E382` `MorParseError`
- `E390` `ReplacementContainsOmission`
- `E202` `MissingFormType`
- `E203` `InvalidFormType`
- `E207` `UnknownAnnotation`
- `E208` `EmptyReplacement`
- `E209` `EmptySpokenContent`
- `E210` `IllegalReplacementForFragment`
- `E213` `UntranscribedInReplacement`
- `E214` `EmptyAnnotatedContentAnnotations`
- `E232` `InvalidCompoundMarkerPosition`
- `E233` `EmptyCompoundPart`
- `E241` `IllegalUntranscribed`
- `E244` `ConsecutiveStressMarkers`
- `E245` `StressNotBeforeSpokenMaterial`
- `E246` `LengtheningNotAfterSpokenMaterial`
- `E247` `MultiplePrimaryStress`
- `E250` `SecondaryStressWithoutPrimary`
- `E251` `EmptyWordContentText`
- `E252` `SyllablePauseNotBetweenSpokenMaterial`
- `E254` `UndeclaredExplicitWordLanguage`
- `E255` `WholeUtteranceLanguageSwitchShouldUsePrecode`
- `E259` `CommaAfterNonSpokenContent`
- `E404` `OrphanedDependentTier`
- `E503` `MissingUTF8Header`
- `E504` `MissingRequiredHeader`
- `E506` `EmptyParticipantsHeader`
- `E508` `EmptyDateHeader`
- `E509` `EmptyMediaHeader`
- `E510` `EmptyIDLanguage`
- `E511` `EmptyIDSpeaker`
- `E512` `EmptyParticipantCode`
- `E513` `EmptyParticipantRole`
- `E515` `EmptyIDRole`
- `E516` `EmptyDate`
- `E518` `InvalidDateFormat`
- `E525` `UnknownHeader`
- `E526` `UnmatchedBeginGem`
- `E527` `UnmatchedEndGem`
- `E528` `GemLabelMismatch`
- `E529` `NestedBeginGem`
- `E530` `LazyGemInsideScope`
- `E531` `MediaFilenameMismatch`
- `E533` `EmptyOptionsHeader`
- `E534` `UnsupportedOption`
- `E535` `UnsupportedMediaType`
- `E536` `UnsupportedMediaStatus`
- `E537` `UnsupportedNumber`
- `E538` `UnsupportedRecordingQuality`
- `E539` `UnsupportedTranscription`
- `E540` `InvalidTimeDuration`
- `E541` `InvalidTimeStart`
- `E542` `UnsupportedSex`
- `E543` `HeaderOutOfOrder`
- `E544` `MediaLinkageWithoutTiming`
- `E545` `InvalidBirthDateFormat`
- `E546` `UnsupportedSesValue`
- `E600` `TierValidationError`
- `E601` `InvalidDependentTier`
- `E602` `MalformedTierHeader`
- `E603` `InvalidTimTierFormat`
- `E604` `GraWithoutMor`
- `E605` `UnsupportedDependentTier`
- `E703` `UnexpectedMorphologyNode`
- `E707` `MorTerminatorPresenceMismatch`
- `E711` `MorEmptyContent`
- `E716` `MorTerminatorValueMismatch`
- `E721` `GraNonSequentialIndex`
- `E722` `GraNoRoot`
- `E723` `GraMultipleRoots`
- `E724` `GraCircularDependency`
- `E725` `ModsylModCountMismatch`
- `E726` `PhosylPhoCountMismatch`
- `E727` `PhoalnModCountMismatch`
- `E728` `PhoalnPhoCountMismatch`
- `E729` `BulletOverlap`
- `E730` `BulletGap`
- `E731` `SpeakerBulletSelfOverlap`
- `E732` `MissingBullet`
- `E733` `ModCountMismatchTooFew`
- `E734` `ModCountMismatchTooMany`
- `W108` `SpeakerNotFoundInParticipants`
- `W601` `EmptyUserDefinedTier`
- `W602` `UnknownUserDefinedTier`
- `W999` `LegacyWarning`
- `E999` `UnknownError`

## Reverse Mapping (TalkBank -> CHECK)

| TalkBank Code | Variant | CHECK Rules |
|---|---|---|
| `E001` | `InternalError` | None |
| `E002` | `TestError` | None |
| `E003` | `EmptyString` | None |
| `E101` | `InvalidLineFormat` | None |
| `E301` | `MissingMainTier` | None |
| `E302` | `MissingNode` | None |
| `E303` | `SyntaxError` | 91 |
| `E304` | `MissingSpeaker` | 19, 21, 72, 75 |
| `E305` | `MissingTerminator` | 19, 31, 36, 50, 72, 75 |
| `E306` | `EmptyUtterance` | None |
| `E307` | `InvalidSpeaker` | None |
| `E308` | `UndeclaredSpeaker` | 12, 13, 18, 133 |
| `E309` | `UnexpectedSyntax` | None |
| `E310` | `ParseFailed` | None |
| `E311` | `UnexpectedNode` | None |
| `E312` | `UnclosedBracket` | None |
| `E313` | `UnclosedParenthesis` | None |
| `E314` | `IncompleteAnnotation` | None |
| `E315` | `InvalidControlCharacter` | None |
| `E316` | `UnparsableContent` | 128, 130 |
| `E319` | `UnparsableLine` | None |
| `E320` | `UnparsableHeader` | None |
| `E321` | `UnparsableUtterance` | None |
| `E322` | `EmptyColon` | None |
| `E323` | `MissingColonAfterSpeaker` | None |
| `E324` | `UnrecognizedUtteranceError` | None |
| `E325` | `UnexpectedUtteranceChild` | None |
| `E326` | `UnexpectedLineType` | None |
| `E330` | `TreeParsingError` | None |
| `E331` | `UnexpectedNodeInContext` | None |
| `E340` | `UnknownBaseContent` | None |
| `E341` | `UnbalancedQuotationCrossUtterance` | None |
| `E342` | `MissingRequiredElement` | None |
| `E344` | `InvalidContentAnnotationNesting` | None |
| `E346` | `UnmatchedContentAnnotationEnd` | 23, 26, 27, 28, 29, 129, 131 |
| `E347` | `UnbalancedOverlap` | 24, 51 |
| `E348` | `MissingOverlapEnd` | 25, 51 |
| `E351` | `MissingQuoteBegin` | None |
| `E352` | `MissingQuoteEnd` | None |
| `E353` | `MissingOtherCompletionContext` | None |
| `E354` | `MissingTrailingOffTerminator` | None |
| `E355` | `InterleavedContentAnnotations` | None |
| `E356` | `UnmatchedUnderlineBegin` | 26, 27, 28, 29, 117 |
| `E357` | `UnmatchedUnderlineEnd` | 26, 27, 28, 29, 117 |
| `E358` | `UnmatchedLongFeatureBegin` | None |
| `E359` | `UnmatchedLongFeatureEnd` | None |
| `E360` | `InvalidMediaBullet` | 19, 72, 75, 81, 89, 90, 98, 108, 110, 115, 118 |
| `E361` | `InvalidTimestamp` | 82, 89, 90, 98, 108, 115 |
| `E362` | `TimestampBackwards` | 83, 98, 108, 115 |
| `E363` | `InvalidPostcode` | None |
| `E364` | `MalformedWordContent` | None |
| `E365` | `MalformedTierContent` | None |
| `E366` | `LongFeatureLabelMismatch` | None |
| `E367` | `UnmatchedNonvocalBegin` | None |
| `E368` | `UnmatchedNonvocalEnd` | None |
| `E369` | `NonvocalLabelMismatch` | None |
| `E370` | `StructuralOrderError` | 52, 119, 151, 159 |
| `E371` | `PauseInPhoGroup` | None |
| `E372` | `NestedQuotation` | None |
| `E373` | `InvalidOverlapIndex` | None |
| `E375` | `ContentAnnotationParseError` | 22 |
| `E376` | `ReplacementParseError` | None |
| `E382` | `MorParseError` | None |
| `E387` | `ReplacementOnFragment` | 141 |
| `E388` | `ReplacementOnNonword` | 141 |
| `E389` | `ReplacementOnFiller` | 141 |
| `E390` | `ReplacementContainsOmission` | None |
| `E391` | `ReplacementContainsUntranscribed` | 158 |
| `E202` | `MissingFormType` | None |
| `E203` | `InvalidFormType` | None |
| `E207` | `UnknownAnnotation` | None |
| `E208` | `EmptyReplacement` | None |
| `E209` | `EmptySpokenContent` | None |
| `E210` | `IllegalReplacementForFragment` | None |
| `E212` | `InvalidWordFormat` | 97, 155 |
| `E213` | `UntranscribedInReplacement` | None |
| `E214` | `EmptyAnnotatedContentAnnotations` | None |
| `E220` | `IllegalDigits` | 38, 47 |
| `E230` | `UnbalancedCADelimiter` | 26, 27, 28, 29, 117 |
| `E231` | `UnbalancedShortening` | 26, 27, 28, 29, 55, 56, 97 |
| `E232` | `InvalidCompoundMarkerPosition` | None |
| `E233` | `EmptyCompoundPart` | None |
| `E241` | `IllegalUntranscribed` | None |
| `E242` | `UnbalancedQuotation` | 26, 27, 28, 29, 136, 137 |
| `E243` | `IllegalCharactersInWord` | 3, 4, 14, 19, 57, 66, 67, 92, 93, 98, 148, 156, 160, 161 |
| `E244` | `ConsecutiveStressMarkers` | None |
| `E245` | `StressNotBeforeSpokenMaterial` | None |
| `E246` | `LengtheningNotAfterSpokenMaterial` | None |
| `E247` | `MultiplePrimaryStress` | None |
| `E248` | `TertiaryLanguageNeedsExplicitCode` | 62, 77, 120 |
| `E249` | `MissingLanguageContext` | 62, 77 |
| `E250` | `SecondaryStressWithoutPrimary` | None |
| `E251` | `EmptyWordContentText` | None |
| `E252` | `SyllablePauseNotBetweenSpokenMaterial` | None |
| `E253` | `EmptyWordContent` | 70 |
| `E254` | `UndeclaredExplicitWordLanguage` | None |
| `E255` | `WholeUtteranceLanguageSwitchShouldUsePrecode` | None |
| `E256` | `IllegalCurlyQuote` | 138, 139 |
| `E258` | `ConsecutiveCommas` | 107 |
| `E259` | `CommaAfterNonSpokenContent` | None |
| `E401` | `DuplicateDependentTier` | 140 |
| `E404` | `OrphanedDependentTier` | None |
| `E501` | `DuplicateHeader` | 6 |
| `E502` | `MissingEndHeader` | 7 |
| `E503` | `MissingUTF8Header` | None |
| `E504` | `MissingRequiredHeader` | None |
| `E505` | `InvalidIDFormat` | 126, 127, 143 |
| `E506` | `EmptyParticipantsHeader` | None |
| `E507` | `EmptyLanguagesHeader` | 69 |
| `E508` | `EmptyDateHeader` | None |
| `E509` | `EmptyMediaHeader` | None |
| `E510` | `EmptyIDLanguage` | None |
| `E511` | `EmptyIDSpeaker` | None |
| `E512` | `EmptyParticipantCode` | None |
| `E513` | `EmptyParticipantRole` | None |
| `E515` | `EmptyIDRole` | None |
| `E516` | `EmptyDate` | None |
| `E517` | `InvalidAgeFormat` | 126, 127, 153 |
| `E518` | `InvalidDateFormat` | None |
| `E519` | `InvalidLanguageCode` | 62, 77, 121, 126, 127 |
| `E522` | `SpeakerNotDefined` | 12, 13, 18, 60, 61, 68, 100, 125, 126, 127, 133 |
| `E523` | `OrphanIDHeader` | 61, 68, 100, 125, 126, 127 |
| `E524` | `BirthUnknownParticipant` | 61, 68, 100, 125, 126, 127 |
| `E525` | `UnknownHeader` | None |
| `E526` | `UnmatchedBeginGem` | None |
| `E527` | `UnmatchedEndGem` | None |
| `E528` | `GemLabelMismatch` | None |
| `E529` | `NestedBeginGem` | None |
| `E530` | `LazyGemInsideScope` | None |
| `E531` | `MediaFilenameMismatch` | None |
| `E532` | `InvalidParticipantRole` | 12, 13, 133, 142 |
| `E533` | `EmptyOptionsHeader` | None |
| `E534` | `UnsupportedOption` | None |
| `E535` | `UnsupportedMediaType` | None |
| `E536` | `UnsupportedMediaStatus` | None |
| `E537` | `UnsupportedNumber` | None |
| `E538` | `UnsupportedRecordingQuality` | None |
| `E539` | `UnsupportedTranscription` | None |
| `E540` | `InvalidTimeDuration` | None |
| `E541` | `InvalidTimeStart` | None |
| `E542` | `UnsupportedSex` | None |
| `E543` | `HeaderOutOfOrder` | None |
| `E544` | `MediaLinkageWithoutTiming` | None |
| `E545` | `InvalidBirthDateFormat` | None |
| `E546` | `UnsupportedSesValue` | None |
| `E600` | `TierValidationError` | None |
| `E601` | `InvalidDependentTier` | None |
| `E602` | `MalformedTierHeader` | None |
| `E603` | `InvalidTimTierFormat` | None |
| `E604` | `GraWithoutMor` | None |
| `E605` | `UnsupportedDependentTier` | None |
| `E700` | `UnexpectedTierNode` | 85 |
| `E701` | `TierBeginTimeNotMonotonic` | 83, 98, 108, 115 |
| `E702` | `InvalidMorphologyFormat` | 45, 114, 134 |
| `E703` | `UnexpectedMorphologyNode` | None |
| `E704` | `SpeakerSelfOverlap` | 84, 98, 108, 115 |
| `E705` | `MorCountMismatchTooFew` | 45, 94, 114, 134, 140 |
| `E706` | `MorCountMismatchTooMany` | 45, 94, 114, 134, 140 |
| `E707` | `MorTerminatorPresenceMismatch` | None |
| `E708` | `MalformedGrammarRelation` | 115 |
| `E709` | `InvalidGrammarIndex` | 115 |
| `E710` | `UnexpectedGrammarNode` | 115 |
| `E711` | `MorEmptyContent` | None |
| `E712` | `GraInvalidWordIndex` | 115 |
| `E713` | `GraInvalidHeadIndex` | 115 |
| `E714` | `PhoCountMismatchTooFew` | 94 |
| `E715` | `PhoCountMismatchTooMany` | 94 |
| `E716` | `MorTerminatorValueMismatch` | None |
| `E718` | `SinCountMismatchTooFew` | 94 |
| `E719` | `SinCountMismatchTooMany` | 94 |
| `E720` | `MorGraCountMismatch` | 45, 94, 114, 115, 134, 140 |
| `E721` | `GraNonSequentialIndex` | None |
| `E722` | `GraNoRoot` | None |
| `E723` | `GraMultipleRoots` | None |
| `E724` | `GraCircularDependency` | None |
| `E725` | `ModsylModCountMismatch` | None |
| `E726` | `PhosylPhoCountMismatch` | None |
| `E727` | `PhoalnModCountMismatch` | None |
| `E728` | `PhoalnPhoCountMismatch` | None |
| `E729` | `BulletOverlap` | None |
| `E730` | `BulletGap` | None |
| `E731` | `SpeakerBulletSelfOverlap` | None |
| `E732` | `MissingBullet` | None |
| `E733` | `ModCountMismatchTooFew` | None |
| `E734` | `ModCountMismatchTooMany` | None |
| `W108` | `SpeakerNotFoundInParticipants` | None |
| `W210` | `MissingWhitespaceBeforeContent` | 3, 4, 14, 19, 66, 67, 92, 93, 98, 148, 160, 161 |
| `W211` | `MissingWhitespaceAfterOverlap` | 3, 4, 14, 19, 66, 67, 92, 93, 98, 148, 160, 161 |
| `W601` | `EmptyUserDefinedTier` | None |
| `W602` | `UnknownUserDefinedTier` | None |
| `W999` | `LegacyWarning` | None |
| `E999` | `UnknownError` | None |

## Priority Action Plan

### P0

- None

### P1

- CHECK `1` `Expected characters are: @ or %% or *.` -> add rule (TalkBank looser; none parity)
- CHECK `2` `Missing ':' character and argument.` -> add rule (TalkBank looser; none parity)
- CHECK `5` `Colon (:) character is illegal.` -> add rule (TalkBank looser; none parity)
- CHECK `8` `Expected characters are: @ %% * TAB.` -> add rule (TalkBank looser; none parity)
- CHECK `9` `Tier name is longer than %d.` -> add rule (TalkBank looser; none parity)
- CHECK `10` `Tier text is longer than %ld.` -> add rule (TalkBank looser; none parity)
- CHECK `11` `Symbol is not declared in the depfile.` -> add rule (TalkBank looser; none parity)
- CHECK `15` `Illegal role. Please see "depfile.cut" for list of roles.` -> add rule (TalkBank looser; none parity)
- CHECK `17` `Tier is not declared in depfile file.` -> add rule (TalkBank looser; none parity)
- CHECK `20` `Undeclared suffix in depfile.` -> add rule (TalkBank looser; none parity)
- CHECK `30` `Text is illegal.` -> add rule (TalkBank looser; none parity)
- CHECK `32` `Code is not declared in depfile.` -> add rule (TalkBank looser; none parity)
- CHECK `33` `Either illegal date or time or symbol is not declared in depfile.` -> add rule (TalkBank looser; none parity)
- CHECK `34` `Illegal date representation.` -> add rule (TalkBank looser; none parity)
- CHECK `35` `Illegal time representation.` -> add rule (TalkBank looser; none parity)
- CHECK `37` `Undeclared prefix.` -> add rule (TalkBank looser; none parity)
- CHECK `42` `Use either "&" or "()", but not both.` -> add rule (TalkBank looser; none parity)
- CHECK `43` `The file must start with "@Begin" tier.` -> add rule (TalkBank looser; none parity)
- CHECK `44` `The file must end with "@End" tier. / Possibly there are some blank lines at the end of the file.` -> add rule (TalkBank looser; none parity)
- CHECK `46` `This @Eg does not have matching @Bg.` -> add rule (TalkBank looser; none parity)
- CHECK `48` `Illegal character(s) found. / Illegal character(s) '%s' found.` -> add rule (TalkBank looser; none parity)
- CHECK `49` `Upper case letters are not allowed inside a word.` -> add rule (TalkBank looser; none parity)
- CHECK `53` `Only one "@Begin" can be in a file.` -> add rule (TalkBank looser; none parity)
- CHECK `54` `Only one "@End" can be in a file.` -> add rule (TalkBank looser; none parity)
- CHECK `59` `Expected second %c character. / Expected second %s character.` -> add rule (TalkBank looser; none parity)
- CHECK `63` `Missing Corpus name.` -> add rule (TalkBank looser; none parity)
- CHECK `64` `Wrong gender information (Choose: female or male).` -> add rule (TalkBank looser; none parity)
- CHECK `65` `This item can not be followed by the next symbol. / Item '%s' can not be followed by the next symbol.` -> add rule (TalkBank looser; none parity)
- CHECK `71` `This item must be before pause (#). / Item '%s' must be before pause (#).` -> add rule (TalkBank looser; none parity)
- CHECK `73` `This item must be preceded by text or '0'. / Item '%s' must be preceded by text or '0'.` -> add rule (TalkBank looser; none parity)
- CHECK `76` `Only one letter is allowed with '@l'.` -> add rule (TalkBank looser; none parity)
- CHECK `78` `This item must be used at the beginning of tier. / Item '%s' must be used at the beginning of tier.` -> add rule (TalkBank looser; none parity)
- CHECK `79` `Only one occurrence of | symbol per word is allowed.` -> add rule (TalkBank looser; none parity)
- CHECK `80` `There must be at least one occurrence of '|'.` -> add rule (TalkBank looser; none parity)
- CHECK `86` `Illegal character. Please re-enter it using Unicode standard.` -> add rule (TalkBank looser; none parity)
- CHECK `87` `Malformed structure.` -> add rule (TalkBank looser; none parity)
- CHECK `88` `Illegal use of compounds and special form markers.` -> add rule (TalkBank looser; none parity)
- CHECK `95` `Illegal use of capitalized words in compounds.` -> add rule (TalkBank looser; none parity)
- CHECK `96` `Word color is now illegal.` -> add rule (TalkBank looser; none parity)
- CHECK `99` `Extension is not allow at the end of media file name.` -> add rule (TalkBank looser; none parity)
- CHECK `101` `This item must be followed or preceded by text. / Item '%s' must be followed or preceded by text.` -> add rule (TalkBank looser; none parity)
- CHECK `102` `Italic markers are no longer legal in CHAT.` -> add rule (TalkBank looser; none parity)
- CHECK `103` `Illegal use of both CA and IPA on "@Options:" tier.` -> add rule (TalkBank looser; none parity)
- CHECK `104` `Please select "CAfont" or "Ascender Uni Duo" font for CA file as per "@Options:" tier.` -> add rule (TalkBank looser; none parity)
- CHECK `105` `Please select "Charis SIL" font for IPA file as per "@Options:" tier.` -> add rule (TalkBank looser; none parity)
- CHECK `106` `The whole code must be on one line. Please run chstring +q on this file.` -> add rule (TalkBank looser; none parity)
- CHECK `109` `Postcodes are not allowed on dependent tiers.` -> add rule (TalkBank looser; none parity)
- CHECK `111` `Illegal pause format. Pause has to have '.' / Pause needs '.' in '%s' or this item is in wrong location.` -> add rule (TalkBank looser; none parity)
- CHECK `112` `Missing %s tier with media file name in headers section at the top of the file.` -> add rule (TalkBank looser; none parity)
- CHECK `113` `Illegal keyword, use "audio", "video" or look in depfile.cut.` -> add rule (TalkBank looser; none parity)
- CHECK `116` `Specifying Font for individual lines is illegal. Please open this file and save it again.` -> add rule (TalkBank looser; none parity)
- CHECK `123` `Illegal character found in tier text. If it CA, then add "@Options: CA" / Illegal character '%s' found in tier text. If it CA, then add "@Options: CA"` -> add rule (TalkBank looser; none parity)
- CHECK `124` `Please remove "unlinked" from @Media header.` -> add rule (TalkBank looser; none parity)
- CHECK `132` `Tabs should only be used to mark the beginning of lines.` -> add rule (TalkBank looser; none parity)
- CHECK `135` `This item is illegal. / Item '%s' is illegal.` -> add rule (TalkBank looser; none parity)
- CHECK `144` `Either illegal SES field value or symbol is not declared in depfile.` -> add rule (TalkBank looser; none parity)
- CHECK `145` `This intonational marker should be outside paired markers.` -> add rule (TalkBank looser; none parity)
- CHECK `146` `The &= symbol must include some code after '=' character.` -> add rule (TalkBank looser; none parity)
- CHECK `147` `Undeclared special form marker in depfile.` -> add rule (TalkBank looser; none parity)
- CHECK `149` `Illegal character located between a word and [...] code. / Illegal character '%s' located between a word and [...] code.` -> add rule (TalkBank looser; none parity)
- CHECK `150` `Illegal item located between a word and [...] code.` -> add rule (TalkBank looser; none parity)
- CHECK `154` `Please add "unlinked" to @Media header.` -> add rule (TalkBank looser; none parity)
- CHECK `157` `Media file name has to match datafile name.` -> add rule (TalkBank looser; none parity)

### P2

- None

### P3

- None

## Notes and Caveats

- This mapping is comprehensive but heuristic for rules with broad/generic wording.
- CHECK rule anomalies from the reference doc are explicitly modeled as intentional behavioral divergences when TalkBank enforces stricter semantics.
- Remaining `None` mappings should be triaged manually for true coverage gaps vs non-equivalent CHECK legacy behavior.
