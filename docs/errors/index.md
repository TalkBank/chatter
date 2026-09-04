# CHAT Error Reference

Every error and warning code, in code order. Follow a code for its
description, its examples, and the CHAT rule it enforces.

Status: ✅ = active in the validator, ⏳ = documented but not yet enforced, ? = deprecated.

| Code | Name | Kind | Level | Status |
|------|------|------|-------|--------|
| [E001](E001.md) | InternalError | Invalidity |  | ✅ |
| [E002](E002.md) | TestError | Invalidity |  | ✅ |
| [E003](E003.md) | Empty string input | Invalidity | file | ⏳ |
| [E101](E101.md) | Invalid line format | Invalidity | file | ⏳ |
| [E202](E202.md) | Missing form type after @ | Invalidity | word | ✅ |
| [E203](E203.md) | Invalid form type marker | Invalidity | word | ✅ |
| [E207](E207.md) | Unknown scoped annotation marker | Invalidity | word | ✅ |
| [E208](E208.md) | Empty replacement | Invalidity | word | ✅ |
| [E209](E209.md) | Word has no spoken content | Invalidity | word | ✅ |
| [E210](E210.md) | Deprecated, replaced by E387 | Invalidity | word | ? |
| [E212](E212.md) | Invalid word format | Invalidity | word | ⏳ |
| [E213](E213.md) | Deprecated, replaced by E391 | Invalidity | word | ? |
| [E220](E220.md) | Illegal digits in word content | Invalidity | word | ✅ |
| [E230](E230.md) | Unbalanced CA delimiter | Invalidity | word | ✅ |
| [E231](E231.md) | Unbalanced shortening parenthesis | Invalidity | word | ✅ |
| [E232](E232.md) | Compound marker at word start | Invalidity | word | ✅ |
| [E233](E233.md) | Empty compound part | Invalidity | word | ✅ |
| [E241](E241.md) | Illegal Untranscribed Marker 'xx' | Invalidity | word | ✅ |
| [E242](E242.md) | Unbalanced quotation delimiters | Invalidity | utterance | ✅ |
| [E243](E243.md) | Pipe character in main-tier word text | Invalidity | word | ✅ |
| [E244](E244.md) | Consecutive stress markers in word | Invalidity | word | ✅ |
| [E245](E245.md) | Stress marker without following spoken material | Invalidity | word | ✅ |
| [E246](E246.md) | Lengthening marker not after spoken material | Invalidity | word | ⏳ |
| [E247](E247.md) | Multiple primary stress markers in one word | Invalidity | word | ✅ |
| [E248](E248.md) | Bare @s shortcut in tertiary language context | Invalidity | word | ✅ |
| [E249](E249.md) | Bare @s shortcut with no secondary language | Invalidity | word | ✅ |
| [E250](E250.md) | Secondary stress without primary stress | Invalidity | word | ✅ |
| [E251](E251.md) | Empty word content text | Invalidity | word | ⏳ |
| [E252](E252.md) | Syntax error - caret at word start | Invalidity | word | ✅ |
| [E253](E253.md) | Empty word content | Invalidity | word | ✅ |
| [E255](E255.md) | Whole-utterance language switch should use precode | Invalidity | utterance | ✅ |
| [E256](E256.md) | Illegal curly single quote | Invalidity | word | ✅ |
| [E258](E258.md) | Consecutive commas | Invalidity | utterance | ✅ |
| [E259](E259.md) | Comma after non-spoken content | Invalidity | word | ✅ |
| [E301](E301.md) | Empty speaker code | Invalidity | utterance | ✅ |
| [E302](E302.md) | Missing required node | Invalidity | utterance | ⏳ |
| [E303](E303.md) | Header colon not followed by a TAB | Invalidity | file | ⏳ |
| [E304](E304.md) | Missing speaker code | Invalidity | utterance | ⏳ |
| [E305](E305.md) | Missing terminator | Invalidity | utterance | ✅ |
| [E306](E306.md) | Utterance has no content | Invalidity | utterance | ✅ |
| [E307](E307.md) | Invalid speaker code | Invalidity | utterance | ✅ |
| [E308](E308.md) | Undeclared speaker | Invalidity | utterance | ✅ |
| [E309](E309.md) | Unexpected syntax | Invalidity | utterance | ⏳ |
| [E310](E310.md) | Parser failed to produce valid parse tree | Invalidity | utterance | ⏳ |
| [E311](E311.md) | Unclosed replacement bracket | Invalidity | utterance | ✅ |
| [E312](E312.md) | Unclosed bracket | Invalidity | utterance | ⏳ |
| [E313](E313.md) | Unclosed parenthesis | Invalidity | utterance | ✅ |
| [E314](E314.md) | Incomplete annotation | Invalidity | utterance | ✅ |
| [E315](E315.md) | Invalid control character | Invalidity | utterance, file | ✅ |
| [E316](E316.md) | Unparsable content | Invalidity | utterance, tier | ✅ |
| [E319](E319.md) | UnparsableLine | Invalidity | utterance | ⏳ |
| [E320](E320.md) | UnparsableHeader | Invalidity | utterance | ⏳ |
| [E321](E321.md) | UnparsableUtterance | Invalidity | utterance | ⏳ |
| [E322](E322.md) | EmptyColon | Invalidity | utterance | ⏳ |
| [E323](E323.md) | Missing colon after speaker code | Invalidity | utterance | ⏳ |
| [E324](E324.md) | Unrecognized utterance-level parse failure | Invalidity | utterance | ✅ |
| [E325](E325.md) | UnexpectedUtteranceChild | Invalidity | utterance | ⏳ |
| [E326](E326.md) | UnexpectedLineType | Invalidity | utterance | ✅ |
| [E330](E330.md) | Internal CST traversal failure | Invalidity | utterance | ✅ |
| [E331](E331.md) | UnexpectedNodeInContext | Invalidity | utterance | ⏳ |
| [E340](E340.md) | UnknownBaseContent | Invalidity |  | ✅ |
| [E341](E341.md) | UnbalancedQuotationCrossUtterance | Invalidity | utterance | ⏳ |
| [E342](E342.md) | Missing required element (recovery placeholder) | Invalidity | utterance | ✅ |
| [E344](E344.md) | Quotation-precedes terminator without a quoted linker | Invalidity | utterance | ⏳ |
| [E346](E346.md) | Quoted-utterance linker outside a quotation sequence | Invalidity | utterance | ⏳ |
| [E347](E347.md) | Unbalanced cross-speaker overlap (indexed markers) | Invalidity | utterance | ✅ |
| [E348](E348.md) | Unpaired overlap marker within utterance | Invalidity | utterance | ⏳ |
| [E351](E351.md) | MissingQuoteBegin | Invalidity | utterance | ⏳ |
| [E352](E352.md) | MissingQuoteEnd | Invalidity | utterance | ⏳ |
| [E353](E353.md) | MissingOtherCompletionContext | Invalidity | utterance | ⏳ |
| [E354](E354.md) | MissingTrailingOffTerminator | Invalidity | utterance | ⏳ |
| [E355](E355.md) | InterleavedScopedAnnotations | Invalidity | utterance | ⏳ |
| [E356](E356.md) | UnmatchedUnderlineBegin | Invalidity | utterance | ✅ |
| [E357](E357.md) | UnmatchedUnderlineEnd | Invalidity | utterance | ✅ |
| [E358](E358.md) | Unmatched long-feature begin | Invalidity | utterance | ✅ |
| [E359](E359.md) | Unmatched long-feature end | Invalidity | utterance | ✅ |
| [E360](E360.md) | Invalid media bullet | Invalidity | utterance | ⏳ |
| [E361](E361.md) | Invalid bullet timestamp | Invalidity | utterance | ⏳ |
| [E362](E362.md) | Bullet times backwards | Invalidity | utterance | ✅ |
| [E363](E363.md) | Postcode without content | Invalidity | utterance | ✅ |
| [E364](E364.md) | Malformed word content | Invalidity | utterance | ⏳ |
| [E365](E365.md) | Malformed tier content | Invalidity | utterance | ⏳ |
| [E367](E367.md) | Unmatched nonvocal begin | Invalidity | utterance | ✅ |
| [E368](E368.md) | Unmatched nonvocal end | Invalidity | utterance | ✅ |
| [E370](E370.md) | Structural order error | Invalidity | utterance | ✅ |
| [E371](E371.md) | Pause inside a phonological group | Invalidity | utterance | ✅ |
| [E372](E372.md) | Nested quotation | Invalidity | utterance | ✅ |
| [E373](E373.md) | InvalidOverlapIndex | Invalidity | utterance | ✅ |
| [E375](E375.md) | Scoped annotation parse error | Invalidity | word, utterance | ✅ |
| [E376](E376.md) | Replacement parse error | Invalidity | utterance | ✅ |
| [E377](E377.md) | A retracing marker with no material of its own | Invalidity | utterance | ✅ |
| [E378](E378.md) | A retracing marker over material with no words | Invalidity | utterance | ✅ |
| [E382](E382.md) | %mor tier parse failure | Invalidity | tier | ⏳ |
| [E387](E387.md) | Replacement on a phonological fragment | Invalidity | utterance | ✅ |
| [E388](E388.md) | Replacement on a nonword | Invalidity | utterance | ✅ |
| [E389](E389.md) | Replacement on a filler | Invalidity | utterance | ✅ |
| [E390](E390.md) | Replacement containing an omission | Invalidity | utterance | ✅ |
| [E391](E391.md) | Replacement containing untranscribed material | Invalidity | utterance | ✅ |
| [E401](E401.md) | Duplicate dependent tier | Invalidity | tier | ✅ |
| [E404](E404.md) | Orphaned dependent tier | Invalidity | tier | ✅ |
| [E501](E501.md) | Duplicate single-occurrence header | Invalidity | header | ✅ |
| [E502](E502.md) | Missing @End | Invalidity | header | ✅ |
| [E503](E503.md) | Missing required @UTF8 header | Invalidity | header | ✅ |
| [E504](E504.md) | Missing required header | Invalidity | header | ✅ |
| [E505](E505.md) | Invalid @ID format | Invalidity | header | ✅ |
| [E506](E506.md) | Empty @Participants header | Invalidity | header | ✅ |
| [E507](E507.md) | Empty @Languages header | Invalidity | header | ✅ |
| [E508](E508.md) | Empty @Date header (parser) | Invalidity | header | ✅ |
| [E509](E509.md) | Empty @Media header | Invalidity | header | ✅ |
| [E510](E510.md) | Empty language field in @ID | Invalidity | header | ✅ |
| [E511](E511.md) | Empty speaker field in @ID | Invalidity | header | ✅ |
| [E512](E512.md) | Empty participant code | Invalidity | header | ✅ |
| [E513](E513.md) | Participant entry without a role | Invalidity | header | ✅ |
| [E514](E514.md) | Empty corpus field in @ID | Invalidity | header | ✅ |
| [E515](E515.md) | Empty role field in @ID | Invalidity | header | ✅ |
| [E516](E516.md) | Empty @Date | Invalidity | header | ✅ |
| [E517](E517.md) | @ID age field does not match a legal CHAT date pattern | Invalidity | header | ✅ |
| [E518](E518.md) | Invalid @Date format | Invalidity | header | ✅ |
| [E519](E519.md) | Language code not in the ISO 639-3 registry | Invalidity | utterance, header | ✅ |
| [E522](E522.md) | Speaker not declared, or declared without an @ID | Invalidity | utterance, header | ✅ |
| [E523](E523.md) | Orphan @ID header | Invalidity | header | ✅ |
| [E524](E524.md) | @Birth for an unknown participant | Invalidity | header | ✅ |
| [E525](E525.md) | Unknown header | Invalidity | header | ✅ |
| [E526](E526.md) | Unmatched begin gem | Invalidity | header | ✅ |
| [E527](E527.md) | Unmatched end gem | Invalidity | header | ✅ |
| [E528](E528.md) | Gem label mismatch | Invalidity | header | ✅ |
| [E529](E529.md) | Nested begin gem with the same label | Invalidity | header | ✅ |
| [E530](E530.md) | Lazy gem inside a begin/end scope | Invalidity | header | ✅ |
| [E531](E531.md) | Media filename mismatch | Invalidity | header | ✅ |
| [E532](E532.md) | Invalid participant role | Invalidity | header | ✅ |
| [E533](E533.md) | Empty @Options header | Invalidity | header | ✅ |
| [E534](E534.md) | Unsupported @Options Value | Invalidity | header | ✅ |
| [E535](E535.md) | Unsupported @Media Type | Invalidity | header | ✅ |
| [E536](E536.md) | Unsupported @Media Status | Invalidity | header | ✅ |
| [E537](E537.md) | Unsupported @Number Value | Invalidity | header | ✅ |
| [E538](E538.md) | Unsupported @Recording Quality Value | Invalidity | header | ✅ |
| [E539](E539.md) | Unsupported @Transcription Value | Invalidity | header | ✅ |
| [E540](E540.md) | @Time Duration does not match a legal CLAN time pattern | Invalidity | header | ✅ |
| [E541](E541.md) | @Time Start does not match a legal CLAN time pattern | Invalidity | header | ✅ |
| [E542](E542.md) | Unsupported @ID Sex Value | Invalidity | header | ✅ |
| [E543](E543.md) | Header out of canonical order | Invalidity | header | ✅ |
| [E544](E544.md) | @Media claims linkage but transcript has no timing evidence | Invalidity | file | ✅ |
| [E545](E545.md) | @Birth of date does not match a legal CHAT date pattern | Invalidity | header | ✅ |
| [E546](E546.md) | Unsupported @ID SES Value | Invalidity | header | ✅ |
| [E547](E547.md) | Constant participant header out of order | Invalidity | header | ✅ |
| [E548](E548.md) | @ID header out of order | Invalidity | header | ✅ |
| [E549](E549.md) | Duplicate speaker declaration | Invalidity | header | ✅ |
| [E550](E550.md) | Trailing comma in @Participants | Invalidity | header | ✅ |
| [E551](E551.md) | @Options header out of order | Invalidity | header | ✅ |
| [E552](E552.md) | @Media declares unlinked but transcript carries timing | Invalidity | file | ✅ |
| [E600](E600.md) | Tier alignment skipped due to parse errors | Invalidity | tier | ✅ |
| [E601](E601.md) | Invalid dependent tier content | Invalidity | tier | ✅ |
| [E602](E602.md) | Malformed dependent tier header | Invalidity | tier | ✅ |
| [E603](E603.md) | Invalid %tim Tier Format | Unmodeled | utterance | ✅ |
| [E604](E604.md) | %gra Tier Without %mor Tier | Invalidity | utterance | ✅ |
| [E605](E605.md) | Unsupported Dependent Tier | Invalidity | utterance | ✅ |
| [E701](E701.md) | Per-speaker start-time not monotonically increasing | Invalidity | utterance | ✅ |
| [E702](E702.md) | %mor item without a separator | Invalidity | tier | ⏳ |
| [E704](E704.md) | Speaker self-overlap, overlapping overlap markers | Invalidity | tier | ✅ |
| [E705](E705.md) | %mor has fewer items than the main tier has words | Invalidity | tier | ✅ |
| [E706](E706.md) | %mor has more items than the main tier has words | Invalidity | tier | ✅ |
| [E707](E707.md) | Mor terminator presence mismatch | Invalidity | tier | ⏳ |
| [E708](E708.md) | Malformed grammar relation on %gra tier | Invalidity | tier | ⏳ |
| [E709](E709.md) | Invalid grammar index | Invalidity | tier | ✅ |
| [E710](E710.md) | Unexpected node in %gra | Invalidity | tier | ✅ |
| [E711](E711.md) | Mor empty content | Invalidity | tier | ⏳ |
| [E712](E712.md) | %gra word index out of range | Invalidity | tier | ✅ |
| [E713](E713.md) | Gra head index invalid | Invalidity | tier | ✅ |
| [E714](E714.md) | %pho has fewer tokens than the main tier has words | Invalidity | tier | ✅ |
| [E715](E715.md) | %pho has more tokens than the main tier has words | Invalidity | tier | ✅ |
| [E716](E716.md) | Mor terminator value mismatch | Invalidity | tier | ✅ |
| [E718](E718.md) | %sin has fewer tokens than the main tier has words | Invalidity | tier | ✅ |
| [E719](E719.md) | %sin has more tokens than the main tier has words | Invalidity | tier | ✅ |
| [E720](E720.md) | Mor-Gra count mismatch | Invalidity | tier | ✅ |
| [E721](E721.md) | %gra indices not sequential | Invalidity | tier | ✅ |
| [E722](E722.md) | %gra has no ROOT | Invalidity | tier | ✅ |
| [E723](E723.md) | %gra has more than one ROOT | Invalidity | tier | ✅ |
| [E724](E724.md) | GRA has circular dependency | Invalidity | tier | ✅ |
| [E725](E725.md) | Modsyl tier word count does not match mod tier | Invalidity | tier | ✅ |
| [E726](E726.md) | Phosyl tier word count does not match pho tier | Invalidity | tier | ✅ |
| [E727](E727.md) | Phoaln tier word count does not match mod tier | Invalidity | tier | ✅ |
| [E728](E728.md) | Phoaln tier word count does not match pho tier | Invalidity | tier | ✅ |
| [E729](E729.md) | Cross-speaker bullet overlap | Invalidity | tier | ⏳ |
| [E730](E730.md) | Bullet timing gap | Invalidity | tier | ⏳ |
| [E731](E731.md) | Speaker bullet self-overlap via timing | Invalidity | tier | ⏳ |
| [E732](E732.md) | Missing bullet in bullet consistency mode | Invalidity | tier | ⏳ |
| [E733](E733.md) | %mod has fewer tokens than the main tier has words | Invalidity | tier | ✅ |
| [E734](E734.md) | %mod has more tokens than the main tier has words | Invalidity | tier | ✅ |
| [E735](E735.md) | Syllabification unit is not a phone:CODE pair | Invalidity | tier | ✅ |
| [E736](E736.md) | Illegal syllable constituent code | Invalidity | tier | ✅ |
| [E737](E737.md) | Modsyl does not reproduce the mod word | Invalidity | tier | ✅ |
| [E738](E738.md) | Phosyl does not reproduce the pho word | Invalidity | tier | ✅ |
| [E739](E739.md) | Phoaln pair is malformed | Invalidity | tier | ✅ |
| [E740](E740.md) | Phoaln model side does not reproduce the mod word | Invalidity | tier | ✅ |
| [E741](E741.md) | Phoaln actual side does not reproduce the pho word | Invalidity | tier | ✅ |
| [E742](E742.md) | Xphoint bullet has start >= end | Invalidity | tier | ✅ |
| [E743](E743.md) | Xphoint interval starts are not non-decreasing | Invalidity | tier | ✅ |
| [E744](E744.md) | Xphoint intervals fall outside the media bullet | Invalidity | tier | ✅ |
| [E745](E745.md) | Xphoint group does not reproduce the pho word | Invalidity | tier | ✅ |
| [E746](E746.md) | Xphoint group count does not match the pho word count | Invalidity | tier | ✅ |
| [E747](E747.md) | Blank line not allowed | Invalidity | file | ✅ |
| [E748](E748.md) | Leading zero in bullet timestamp | Invalidity | tier | ✅ |
| [E749](E749.md) | Comma glued to the following word | Invalidity | utterance | ✅ |
| [E750](E750.md) | Space inside angle-bracket group delimiters | Invalidity | utterance | ✅ |
| [E751](E751.md) | Pause glued to the preceding word | Invalidity | utterance | ✅ |
| [E752](E752.md) | Timing bullets without an @Media header | Invalidity | file | ✅ |
| [E753](E753.md) | Word consisting only of repetition segments | Invalidity | utterance | ✅ |
| [E755](E755.md) | Utterance language not declared in @Languages | Invalidity | utterance | ✅ |
| [E756](E756.md) | Empty dependent tier | Invalidity | utterance | ✅ |
| [E757](E757.md) | Bracketed code glued to the following content | Style | utterance | ✅ |
| [E758](E758.md) | Trailing space in a line's tier separator (non-CA file) | Invalidity | utterance | ✅ |
| [E759](E759.md) | Annotation at utterance start has nothing to attach to | Invalidity | utterance | ✅ |
| [E760](E760.md) | %mor item has an empty part-of-speech field | Invalidity | tier | ✅ |
| [E761](E761.md) | %gra relation head is not a Universal Dependencies relation | Invalidity | tier | ✅ |
| [E762](E762.md) | prefix marker stands alone or opens a word | Invalidity | word | ✅ |
| [E763](E763.md) | prefix marker in a language that does not use it | Invalidity | word | ✅ |
| [E764](E764.md) | prefixed form glued to the preceding word | Style | utterance | ✅ |
| [E765](E765.md) | separator glued to the following content | Invalidity | utterance | ✅ |
| [E766](E766.md) | linker not utterance-initial | Invalidity | utterance | ✅ |
| [E767](E767.md) | whitespace before the comma in @Media | Invalidity | header | ✅ |
| [E768](E768.md) | @Media filename cannot be written and read back unchanged | Invalidity |  | ✅ |
| [E999](E999.md) | Unknown error (internal fallback) | Invalidity | tier | ⏳ |
| [W108](W108.md) | Speaker not declared in @Participants (warning form) | Invalidity | utterance | ✅ |

