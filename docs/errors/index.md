# CHAT Error Reference

Complete reference for all CHAT parser and validation errors.

Status legend: ✅ = active in the validator, ⏳ = documented but not yet enforced.

## internal (E0x)

Internal invariant failure. This error indicates a bug in the parser itself, not in the CHAT input. It cannot be triggered by any CHAT file.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E001](E001.md) | E001: InternalError | error | ✅ |

## internal (E0x)

Test-only sentinel error code. Used exclusively in the test suite to verify error handling plumbing. Never emitted in production.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E002](E002.md) | E002: TestError | error | ✅ |

## validation (E0x)

The input string is empty. E003 (EmptyString) is the default error code for empty NonEmptyString fields during model validation, but an empty file does not trigger E003 end-to-end. Instead, the parser produces header validation errors (missing @UTF8, @End, @Participants, etc.) and E316 (unparsable content) because there are no headers to find.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E003](E003.md) | E003: Empty string input | error | ⏳ |

## validation (E1x)

A line in the CHAT file does not match any valid line format (must start with @, *, %, or be a continuation tab). E101 (InvalidLineFormat) is defined as an error code but is not currently emitted by the tree-sitter parser. The parser produces header validation errors for the missing scaffolding and does not reach E101 detection.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E101](E101.md) | E101: Invalid line format | error | ⏳ |

## Parser error (E2x)

Missing form type after @

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E202](E202.md) | E202: Missing form type after @ | error | ✅ |

## Word validation (E2x)

A word contains @ at a position where a form type marker is expected, but no valid form type follows. Tree-sitter produces an ERROR node at the @.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E202](E202.md) | E202: Missing form type after @ | error | ✅ |

## validation (E2x)

Word contains an invalid or undeclared @ form type marker (e.g., dog@b@c has multiple stacked markers).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E203](E203.md) | E203: Invalid form type marker | error | ✅ |

## Word validation (E2x)

Unknown scoped annotation marker

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E207](E207.md) | E207: Unknown scoped annotation marker | error | ✅ |

## validation (E2x)

Empty replacement

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E208](E208.md) | E208: Empty replacement | error | ✅ |

## validation (E2x)

A word on the main tier consists entirely of shortening notation (text) with no actual spoken material. In CHAT, (the) means the sounds were omitted; it is not the same as the word being spoken. To mark an omitted word, use 0the (zero-word) instead.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E209](E209.md) | E209, Word has no spoken content | error | ✅ |

## Word validation (E2x)

Deprecated. This error code was replaced by E387 (ReplacementOnFragment). The validation logic now emits E387 instead of E210 for the same condition.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E210](E210.md) | E210: Deprecated, replaced by E387 | error | ? |

## Word validation (E2x)

A word on the main tier has an invalid format that does not match any recognized CHAT word structure. The validator reports E212 for specific structural violations such as CA omissions used outside CA mode, CA omissions without spoken text, or standalone shortenings.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E212](E212.md) | E212: Invalid word format | error | ⏳ |

## Word validation (E2x)

Deprecated. This error code was replaced by E391 (ReplacementContainsUntranscribed). The validation logic now emits E391 instead of E213 for the same condition.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E213](E213.md) | E213: Deprecated, replaced by E391 | error | ? |

## validation (E2x)

A scoped annotation (e.g., error annotation [*], replacement [: ...]) has an empty content list. The validator reports E214 when annotated content has zero scoped annotations attached.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E214](E214.md) | E214: Empty scoped annotation content | error | ⏳ |

## Word validation (E2x)

A word on the main tier contains numeric digits in a language context that does not permit them. Most natural languages (English, Spanish, French, etc.) do not allow bare digits in words on the main tier. A small set of languages (Chinese, Welsh, Vietnamese, Thai, Cantonese, etc.) permit digits as part of tone notation or numerals.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E220](E220.md) | E220, Illegal digits in word content | error | ✅ |

## validation (E2x)

Compound delimiter (∆) is not properly balanced, opening delimiter has no matching closing delimiter.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E230](E230.md) | E230: Unbalanced CA delimiter | error | ✅ |

## validation (E2x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E231](E231.md) | generated from corpus | error | ✅ |

## validation (E2x)

Compound marker (+) cannot be at the start of a word. Valid compounds have the form left+right.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E232](E232.md) | E232: Compound marker at word start | error | ✅ |

## validation (E2x)

Compound markers (+) must connect two non-empty parts. Adjacent compound markers (un++do) create an empty part between them, which is invalid.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E233](E233.md) | E233: Empty compound part | error | ✅ |

## validation (E2x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E241](E241.md) | generated from corpus | error | ✅ |

## word_validation (E2x)

The marker 'xx' is used for untranscribed speech, but this is not allowed in CHAT. The correct marker for untranscribed speech is 'xxx' (three x's).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E241](E241.md) | E241: Illegal Untranscribed Marker 'xx' | error | ✅ |

## validation (E2x)

Quotation marks must be balanced within an utterance.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E242](E242.md) | E242: Unbalanced quotation marks | error | ✅ |

## validation (E2x)

Word contains illegal characters such as whitespace, control characters, or bullet markers that are not valid in word content.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E243](E243.md) | E243: Illegal characters in word | error | ✅ |

## Word structure (E2x)

The | character is the %mor tier's part-of-speech delimiter and has no meaning in main-tier word text; a word consisting of or containing a bare pipe (hello | there) is invalid (CLAN CHECK error 48, "Illegal character(s) '|' found."). This is a trigger shape of the existing E243 (IllegalCharactersInWord) rule, not a new code: the word scanner already rejects whitespace, bullet markers, control characters, and private-use code points; the pipe joins that set as a reserved tier-delimiter character.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E243](E243.md) | tier word text | error | ✅ |

## validation (E2x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E244](E244.md) | generated from corpus | error | ✅ |

## validation (E2x)

A primary stress marker (ˈ) or secondary stress marker appears at the start of a word but is not followed by any spoken material. The marker has nothing to attach to.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E245](E245.md) | E245, Stress marker without following spoken material | error | ✅ |

## validation (E2x)

A lengthening marker (:) appears before any spoken material in a word rather than after it. In CHAT, the colon : indicates phonological lengthening and must follow the spoken text it modifies (e.g., hel:o is valid, :hello is not).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E246](E246.md) | E246: Lengthening marker not after spoken material | error | ⏳ |

## validation (E2x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E247](E247.md) | generated from corpus | error | ✅ |

## validation (E2x)

The bare @s shortcut toggles between the first two languages declared in @Languages. When an utterance is scoped to a tertiary language (position 3 or later in the @Languages list) via [- code], bare @s is ambiguous, it could mean either the primary or secondary language. The speaker must use an explicit code (@s:eng, @s:spa, etc.) instead.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E248](E248.md) | E248, Bare @s shortcut in tertiary language context | error | ✅ |

## validation (E2x)

The @s shortcut means "the other language"; it toggles between the primary and secondary language declared in @Languages. When there is no secondary language (the @Languages header lists only one language), @s has no target to resolve to. The speaker must use an explicit language code (@s:spa, @s:zho, etc.) or add a second language to the @Languages header.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E249](E249.md) | E249, Bare @s shortcut with no secondary language | error | ✅ |

## validation (E2x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E250](E250.md) | generated from corpus | error | ✅ |

## validation (E2x)

A word content text segment (the spoken text portion of a word or the text inside a shortening) is empty. The validator reports E251 when a Text or ShorteningText element validates to empty via its inner NonEmptyText wrapper.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E251](E251.md) | E251: Empty word content text | error | ⏳ |

## Prosodic marker placement (E2x)

Syntax error - caret at word start

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E252](E252.md) | caret at word start | error | ✅ |

## validation (E2x)

A parsed Word object has empty content, the word node exists in the CST but contains no text.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E253](E253.md) | E253: Empty word content | error | ✅ |

## parser (E2x)

A curly single quotation mark (U+2018 or U+2019) is used as a word character. CHAT requires the ASCII apostrophe; the curly form (typically a typographic apostrophe introduced by autocorrect or ASR) is not a legal word character. Mirrors CLAN CHECK errors 138 (U+2019) and 139 (U+2018).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E256](E256.md) | E256: Illegal curly single quote | error | ✅ |

## validation (E2x)

Consecutive commas

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E258](E258.md) | E258: Consecutive commas | error | ✅ |

## validation (E2x)

Comma without any preceding spoken word in the utterance

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E259](E259.md) | spoken content | error | ✅ |

## Main tier validation (E3x)

Empty speaker code

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E301](E301.md) | E301: Empty speaker code | error | ✅ |

## validation (E3x)

Expected tree-sitter node is missing. E302 (MissingNode) fires when tree-sitter's error recovery inserts a MISSING placeholder node, indicating the grammar expected a specific construct that was not found. This is an internal parser condition triggered by tree-sitter error recovery, not by specific CHAT syntax patterns. It also fires in speaker code validation for invalid characters.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E302](E302.md) | E302: Missing required node | error | ⏳ |

## Parser bugs (experimental) (E3x)

Unexpected node - helper function

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E303](E303.md) | helper function | error | ⏳ |

## Main tier validation (E3x)

Main tier line is missing its speaker code after *.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E304](E304.md) | E304: Missing speaker code | error | ⏳ |

## Main tier validation (E3x)

Main tier is missing its required utterance terminator.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E305](E305.md) | E305: Missing terminator | error | ✅ |

## Main tier validation (E3x)

Utterance has no content

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E306](E306.md) | E306: Utterance has no content | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E307](E307.md) | generated from corpus | error | ✅ |

## Main tier validation (E3x)

Invalid speaker format

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E308](E308.md) | E308: Invalid speaker format | error | ✅ |

## validation (E3x)

Unexpected syntax encountered during parsing. E309 (UnexpectedSyntax) fires when the parser encounters an ERROR node from tree-sitter that contains unexpected content. The error is emitted from make_error_from_node() in helpers.rs.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E309](E309.md) | E309: Unexpected syntax | error | ⏳ |

## Main tier validation (E3x)

Tree-sitter's internal parser returned None (e.g., due to timeout or cancellation) or the parse outcome was rejected with no other errors collected. E310 is a catch-all for complete parse failures where no more specific error code applies.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E310](E310.md) | E310: Parser failed to produce valid parse tree | error | ⏳ |

## Main tier validation (E3x)

Failed to parse utterance

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E311](E311.md) | E311: Failed to parse utterance | error | ⏳ |

## validation (E3x)

Opening bracket [ on the main tier has no matching closing bracket ].

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E312](E312.md) | E312: Unclosed bracket | error | ⏳ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E313](E313.md) | generated from corpus | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E314](E314.md) | generated from corpus | error | ✅ |

## validation (E3x)

Main tier or dependent tier contains an invalid control character (e.g., embedded NUL, SOH, or other non-printable ASCII).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E315](E315.md) | E315: Invalid control character | error | ✅ |

## Dependent tier validation (E3x)

A %mor tier entry contains an angle-bracketed prefix inside the stem position (e.g., noun|<sos>tos, sconj|<sos>tos~aux|...). The CHAT manual's %mor grammar uses these separators inside the stem: - (feature), & (fusion), # (prefix), : (category), ~ (clitic), + (compound). Angle brackets are not valid stem content. The parser produces an ERROR node at the < and the validator reports E316 on the surrounding |<stem>~... region.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E316](E316.md) | bracketed annotation inside %mor stem is invalid | error | ✅ |

## Main tier validation (E3x)

Unparsable content

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E316](E316.md) | E316: Unparsable content | error | ✅ |

## parser_recovery (E3x)

A line could not be classified as a header, utterance, or dependent tier. This is a fallback error emitted when tree-sitter produces an ERROR node for a line whose children cannot be identified as either a header or utterance context.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E319](E319.md) | E319: UnparsableLine | error | ⏳ |

## parser_recovery (E3x)

A header line (starting with @) could not be parsed. This is a fallback error emitted when tree-sitter produces an ERROR node in header context, but the header type is not one of the specifically handled types (@Participants, @Languages, @Date, @Media, @ID).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E320](E320.md) | E320: UnparsableHeader | error | ⏳ |

## parser_recovery (E3x)

An utterance line (starting with *SPEAKER:) could not be parsed. The utterance body contains syntax errors that tree-sitter cannot recover from, and the error doesn't match any of the specifically checked patterns (missing form type, empty replacement, unknown annotation).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E321](E321.md) | E321: UnparsableUtterance | error | ⏳ |

## parser_recovery (E3x)

The main tier speaker prefix has a zero-width (MISSING) colon node. This occurs when tree-sitter synthesizes an empty colon placeholder because the speaker code has no colon at all.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E322](E322.md) | E322: EmptyColon | error | ⏳ |

## validation (E3x)

Missing colon after speaker code on main tier. E323 (MissingColonAfterSpeaker) fires in prefix.rs when the tree-sitter grammar parses a main tier but the colon child node is missing. However, when the colon is absent, the grammar typically fails to match the main tier pattern at all, producing an ERROR node (E316 UnparsableContent) rather than a partial main tier with a missing colon.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E323](E323.md) | E323: Missing colon after speaker code | error | ⏳ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E324](E324.md) | generated from corpus | error | ✅ |

## parser_recovery (E3x)

An unexpected child node was found inside a parsed utterance. The CST contains a node that is neither the main tier nor a recognized dependent tier kind. This typically indicates a tree-sitter error recovery scenario where an unusual node type ends up inside an utterance subtree.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E325](E325.md) | E325: UnexpectedUtteranceChild | error | ⏳ |

## parser_recovery (E3x)

A line was classified as an unexpected type during file structure parsing. This covers two sub-cases:

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E326](E326.md) | E326: UnexpectedLineType | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E330](E330.md) | generated from corpus | error | ✅ |

## parser_recovery (E3x)

A tree-sitter node appeared in a syntactic context where it is not expected. The node type itself is valid CHAT syntax, but it occurs at a position in the AST that violates the grammar. This error is emitted during tree-sitter error recovery, the parser attempts to continue after encountering invalid syntax, and the recovered structure contains nodes in unexpected positions.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E331](E331.md) | E331: UnexpectedNodeInContext | error | ⏳ |

## parser_recovery (E3x)

Main tier content could not be classified as any known word or construct type. This fires when a base_content_item CST node has a child kind that the Rust parser doesn't recognize, indicating a grammar/parser mismatch (the grammar produces a new node type that the parser hasn't been updated to handle).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E340](E340.md) | E340: UnknownBaseContent | error | ✅ |

## cross_utterance (E3x)

A quotation-follows terminator (+"/.) is used but the next utterance from the same speaker does not begin with a quotation precedes linker (+"). This indicates an unbalanced cross-utterance quotation sequence.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E341](E341.md) | E341: UnbalancedQuotationCrossUtterance | error | ⏳ |

## Word validation (E3x)

Missing required element

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E342](E342.md) | E342: Missing required element | error | ⏳ |

## Main tier structure (E3x)

An angle-bracket group <...> on the main tier must be followed by an annotation (a retrace marker such as [//], a scope code such as [?], an explanation [= ...], etc.). A bare <...> group with nothing after it is malformed CHAT. CLAN's check reports it as expected [ ]; < > should be followed by [ ].

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E342](E342.md) | bracket group with no following annotation is invalid | error | ✅ |

## validation (E3x)

Invalid nesting of scoped annotations (quotation precedes pattern). This is a cross-utterance validator (check_quotation_precedes) that is currently DISABLED (enable_quotation_validation: false).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E344](E344.md) | E344: Invalid scoped annotation nesting | error | ⏳ |

## validation (E3x)

Unmatched scoped annotation end marker (> without matching <). This is a cross-utterance validator (check_quoted_linker) that is currently DISABLED (enable_quotation_validation: false).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E346](E346.md) | E346: Unmatched scoped annotation end | error | ⏳ |

## validation (E3x)

An indexed top overlap region (e.g., ⌈2...⌉2) on one speaker has no matching indexed bottom overlap region (⌊2...⌋2) from a different speaker, or vice versa. Reported as a warning because some onset-only marking conventions exist.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E347](E347.md) | speaker overlap (indexed markers) | error | ✅ |

## validation (E3x)

Reserved for within-utterance overlap pairing violations: a closing marker (⌉ or ⌋) without a preceding opening marker (⌈ or ⌊) in the same utterance, or vice versa.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E348](E348.md) | E348, Unpaired overlap marker within utterance | error | ⏳ |

## cross_utterance (E3x)

A self-completion linker (+,) was used but there is no prior utterance from the same speaker. The +, linker requires a preceding interrupted utterance from the same speaker to complete.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E351](E351.md) | E351: MissingQuoteBegin | error | ⏳ |

## cross_utterance (E3x)

A self-completion linker (+,) was used and there IS a prior utterance from the same speaker, but that prior utterance did not end with a +/. (interruption) terminator.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E352](E352.md) | E352: MissingQuoteEnd | error | ⏳ |

## cross_utterance (E3x)

An other-completion linker (++) was used but it is the very first utterance in the file. The ++ linker requires a preceding utterance (from a different speaker) to complete.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E353](E353.md) | E353: MissingOtherCompletionContext | error | ⏳ |

## cross_utterance (E3x)

An other-completion linker (++) was used and the preceding utterance is from a different speaker, but that preceding utterance did not end with +... (trailing off). The other-completion convention requires the previous speaker to have trailed off.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E354](E354.md) | E354: MissingTrailingOffTerminator | error | ⏳ |

## cross_utterance (E3x)

An other-completion linker (++) was used but the preceding utterance is from the same speaker. The ++ linker is for other-completion (completing a different speaker's utterance). To complete one's own utterance, use +, (self-completion) instead.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E355](E355.md) | E355: InterleavedScopedAnnotations | error | ⏳ |

## underline_balance (E3x)

An underline begin marker was found without a matching underline end marker in the same utterance. Underline markers (used in CA transcription to mark stressed syllables) must occur in matched begin/end pairs within a single utterance.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E356](E356.md) | E356: UnmatchedUnderlineBegin | error | ✅ |

## underline_balance (E3x)

An underline end marker was found without a preceding underline begin marker in the same utterance. The end marker has no open underline to close.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E357](E357.md) | E357: UnmatchedUnderlineEnd | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E358](E358.md) | generated from corpus | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E359](E359.md) | generated from corpus | error | ✅ |

## validation (E3x)

Media bullet (timestamp marker) contains malformed content, e.g., non-numeric characters, missing underscore separator, or structurally invalid timestamp format.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E360](E360.md) | E360: Invalid media bullet | error | ⏳ |

## Main tier validation (E3x)

The media bullet contains a deprecated skip flag (dash before closing NAK delimiter). The skip flag is deprecated. Only a small number of occurrences exist across the corpus.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E360](E360.md) | E360: Deprecated Skip Bullet | error | ⏳ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E361](E361.md) | generated from corpus | error | ✅ |

## validation (E3x)

Bullet timestamps must be monotonic

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E362](E362.md) | E362: Bullet timestamps must be monotonic | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E363](E363.md) | generated from corpus | error | ✅ |

## validation (E3x)

Word content is structurally malformed, the parser recognized a word node but its internal structure is invalid (e.g., @s:+ with + instead of a language code).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E364](E364.md) | E364: Malformed word content | error | ⏳ |

## validation (E3x)

A header or tier has content that does not match any recognized CHAT header structure. The parser reports E365 when it encounters an unknown node type during header dispatch in the CST.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E365](E365.md) | E365: Malformed tier content | error | ⏳ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E367](E367.md) | generated from corpus | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E368](E368.md) | generated from corpus | error | ✅ |

## retrace (E3x)

A structural ordering violation in main-tier content. In particular, a retrace or repetition marker ([/], [//], [///]) must be followed by the repeated or corrected material: per the CHAT manual the marker is necessarily followed by the material it retraces. A retrace marker followed only by a terminator (e.g. <the> [/] .) has nothing to retrace and is reported as E370.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E370](E370.md) | E370, Structural order error | error | ✅ |

## validation (E3x)

Pause inside phonological group

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E371](E371.md) | E371: Pause inside phonological group | error | ✅ |

## validation (E3x)

Nested quotation

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E372](E372.md) | E372: Nested quotation | error | ✅ |

## overlap (E3x)

An overlap marker has an index value outside the valid range. For CA overlap brackets (⌈⌉⌊⌋), the index must be 2-9. For scoped overlap annotations ([<], [>]), the index must be 1-9.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E373](E373.md) | E373: InvalidOverlapIndex | error | ✅ |

## Parser bugs (experimental) (E3x)

Scoped annotation parse error

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E375](E375.md) | E375: Scoped annotation parse error | error | ✅ |

## Word annotation (E3x)

A replacement annotation [: text] must be preceded by whitespace, exactly like every other bracketed annotation (the scope codes [?], [!], [/], [//], etc., which base_annotations already requires a space before). A replacement written with no space, glued directly to the word it replaces (word[: foo]), is invalid CHAT.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E375](E375.md) | E375: Replacement [: ...] glued to a word without a preceding space | error | ✅ |

## Word validation (E3x)

Failed to parse replacement annotation content. The [: replacement annotation contains content that cannot be parsed as valid replacement words.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E376](E376.md) | E376: Replacement parse error | error | ✅ |

## Dependent tier parsing (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E382](E382.md) | generated from corpus | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E387](E387.md) | generated from corpus | error | ✅ |

## validation (E3x)

Replacement annotation [: ...] is attached to a non-word element (e.g., a paralinguistic event like &=laugh), which cannot be replaced.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E388](E388.md) | word | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E389](E389.md) | generated from corpus | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E390](E390.md) | generated from corpus | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E391](E391.md) | generated from corpus | error | ✅ |

## validation (E4x)

Duplicate dependent tiers

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E401](E401.md) | E401: Duplicate dependent tiers | error | ✅ |

## validation (E4x)

A dependent tier (%mor, %gra, etc.) appears before any main tier in the file. E404 (OrphanedDependentTier) is emitted by report_top_level_dependent_tier_error() in crates/talkbank-parser/src/parser/chat_file_parser/chat_file/helpers.rs when a %-prefixed ERROR node appears before any utterance.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E404](E404.md) | E404: Orphaned dependent tier | error | ✅ |

## Header validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E501](E501.md) | generated from corpus | error | ✅ |

## validation (E5x)

Every valid CHAT file must end with an @End header. This error indicates the file is missing @End, usually because the file is truncated, empty, or was saved incompletely.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E502](E502.md) | E502: Missing required @End header | error | ✅ |

## parser (E5x)

When a %wor tier contains invalid content (e.g., an action marker like &=head:no) AND the %wor line has 7+ words after the error, tree-sitter's error recovery fails catastrophically: instead of isolating the ERROR to the %wor tier, the entire file becomes one ERROR node. This causes:

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E502 (false positive)](E502 (false positive).md) | E502 false positive: %wor parse error cascades to entire file | error | ✅ |

## Header validation (E5x)

Every valid CHAT file must begin with an @UTF8 header as its first line. This error indicates the file is missing @UTF8, which means the file's character encoding is unspecified. All modern CHAT files are expected to be UTF-8 encoded.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E503](E503.md) | E503: Missing required @UTF8 header | error | ✅ |

## Header validation (E5x)

Missing required header

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E504](E504.md) | E504: Missing required header | error | ✅ |

## Header validation (E5x)

Invalid @ID format

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E505](E505.md) | E505: Invalid @ID format | error | ✅ |

## Header validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E506](E506.md) | generated from corpus | error | ✅ |

## Header validation (E5x)

@Languages header cannot be empty

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E507](E507.md) | E507: @Languages header cannot be empty | error | ✅ |

## Header validation (E5x)

@Date header cannot be empty

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E508](E508.md) | E508: @Date header cannot be empty | error | ✅ |

## Header validation (E5x)

@Media header cannot be empty

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E509](E509.md) | E509: @Media header cannot be empty | error | ✅ |

## Header validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E510](E510.md) | generated from corpus | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E511](E511.md) | generated from corpus | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E512](E512.md) | generated from corpus | error | ✅ |

## Header validation (E5x)

Participant entry should have both code and role

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E513](E513.md) | E513: Participant entry should have both code and role | error | ✅ |

## header_validation (E5x)

The corpus field (2nd field) of an @ID header is blank. The @ID header is lang|corpus|code|age|sex|group|SES|role|education|custom|, and the corpus name is required: a blank corpus is invalid.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E514](E514.md) | E514: Empty corpus field in @ID | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E515](E515.md) | generated from corpus | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E516](E516.md) | generated from corpus | error | ✅ |

## header_validation (E5x)

The @ID header's fourth field (age) must conform to one of the three legal CHAT date patterns defined by CLAN's authoritative depfile.cut:

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E517](E517.md) | E517: @ID age field does not match a legal CHAT date pattern | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E518](E518.md) | generated from corpus | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E519](E519.md) | generated from corpus | error | ✅ |

## header_validation (E5x)

The @L1 of SPK header names a participant's first language. Wild usage is uniformly ISO 639-3 codes (16 distinct values across 1,158 kept files, all registry-valid), so the field is a language CODE and is held to the same registry rule as @Languages / @ID / word-level switches (maintainer ruling 2026-07-15, part 2).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E519](E519.md) | 3 registry | error | ✅ |

## Main tier words (E5x)

An explicit word-level language switch (word@s:CODE) must name a real ISO 639-3 language. The code needs NO declaration in @Languages (maintainer ruling 2026-07-15, part 1), but it must exist in the registry (same ruling, part 2): registry validation is what actually catches typo'd codes (the historical cye/sp/nle class), and it reuses E519, the same rule that already guards @Languages and @ID.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E519](E519.md) | level language code not in the ISO 639 | error | ✅ |

## Header validation (E5x)

@Participants header cannot be empty

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E522](E522.md) | E522: @Participants header cannot be empty | error | ✅ |

## header_validation (E5x)

An utterance uses a speaker code that was not defined in the @Participants header. All speaker codes used in utterances must be declared in the @Participants header.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E522](E522.md) | E522: Undefined Participant in Utterance | error | ✅ |

## Participant validation (E5x)

Orphan @ID header

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E523](E523.md) | E523: Orphan @ID header | error | ✅ |

## Participant validation (E5x)

@Birth header for unknown participant

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E524](E524.md) | E524: @Birth header for unknown participant | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E525](E525.md) | generated from corpus | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E526](E526.md) | generated from corpus | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E527](E527.md) | generated from corpus | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E528](E528.md) | generated from corpus | error | ✅ |

## validation (E5x)

Nested background with identical label

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E529](E529.md) | E529: Nested background with identical label | error | ✅ |

## validation (E5x)

Lazy gem inside background

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E530](E530.md) | E530: Lazy gem inside background | error | ✅ |

## validation (E5x)

The filename in the @Media header does not match the name of the CHAT file being parsed (case-insensitive comparison). For example, if foo.cha contains @Media: bar, audio, E531 is reported because bar does not match foo.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E531](E531.md) | E531: Media filename mismatch | error | ✅ |

## validation (E5x)

Invalid participant role

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E532](E532.md) | E532: Invalid participant role | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E533](E533.md) | generated from corpus | error | ✅ |

## header_validation (E5x)

An @Options header contains a flag that is not one of the recognized option values. The file parses successfully but the unsupported flag is stored as Unsupported(String) and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E534](E534.md) | E534: Unsupported @Options Value | error | ✅ |

## header_validation (E5x)

An @Media header contains a media type that is not one of the recognized values. The file parses successfully but the unsupported type is stored as Unsupported(String) and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E535](E535.md) | E535: Unsupported @Media Type | error | ✅ |

## header_validation (E5x)

An @Media header contains a status value that is not one of the recognized values. The file parses successfully but the unsupported status is stored as Unsupported(String) and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E536](E536.md) | E536: Unsupported @Media Status | error | ✅ |

## header_validation (E5x)

An @Number header contains a value that is not one of the recognized number options. The file parses successfully but the unsupported value is stored as Unsupported(String) and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E537](E537.md) | E537: Unsupported @Number Value | error | ✅ |

## header_validation (E5x)

An @Recording Quality header contains a value that is not one of the recognized quality ratings. The file parses successfully but the unsupported value is stored as Unsupported(String) and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E538](E538.md) | E538: Unsupported @Recording Quality Value | error | ✅ |

## header_validation (E5x)

An @Transcription header contains a value that is not one of the recognized transcription types. The file parses successfully but the unsupported value is stored as Unsupported(String) and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E539](E539.md) | E539: Unsupported @Transcription Value | error | ✅ |

## header_validation (E5x)

An @Time Duration header must match one of the three time patterns that CLAN's authoritative depfile.cut declares legal:

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E540](E540.md) | E540: @Time Duration does not match a legal CLAN time pattern | error | ✅ |

## header_validation (E5x)

An @Time Start header must match one of the two time patterns that CLAN's authoritative depfile.cut declares legal:

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E541](E541.md) | E541: @Time Start does not match a legal CLAN time pattern | error | ✅ |

## header_validation (E5x)

An @ID header contains a sex field value that is not one of the recognized values. The file parses successfully but the unsupported value is stored as Unsupported(String) and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E542](E542.md) | E542: Unsupported @ID Sex Value | error | ✅ |

## header_validation (E5x)

A header appears out of canonical order. For example, @Options or @ID appears before @Participants. CHAT headers must follow the canonical ordering: @UTF8, @Begin, @Languages, @Participants, then other headers like @Options and @ID.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E543](E543.md) | E543: Header out of canonical order | error | ✅ |

## header_validation (E5x)

An @Media header declares a linked media file (no unlinked / missing / notrans status), but the transcript body contains no evidence that any utterance is actually linked to that media. By the CHAT manual's @Media semantics, an unqualified declaration is a promise that the transcript is time-linked to the named file; this check catches transcripts that make that promise without keeping it.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E544](E544.md) | E544: @Media claims linkage but transcript has no timing evidence | error | ✅ |

## header_validation (E5x)

An @Birth of <CODE> header must carry a date matching CLAN's authoritative depfile.cut date template:

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E545](E545.md) | E545: @Birth of date does not match a legal CHAT date pattern | error | ✅ |

## header_validation (E5x)

An @ID header contains an SES (socioeconomic status) field value that is not one of the recognized values. The file parses successfully but the unsupported value is stored as Unsupported(String) and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E546](E546.md) | E546: Unsupported @ID SES Value | error | ✅ |

## header_validation (E5x)

A constant participant-specific header (@Birth of, @Birthplace of, or @L1 of) does not immediately follow the @ID block. These headers must come directly after the @ID headers, before any changeable header such as @Comment, @Date, @Situation, or @Types. A changeable header between the @ID block and a constant participant header is an ordering violation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E547](E547.md) | E547: Constant participant header out of order | error | ✅ |

## header_validation (E5x)

An @ID header does not immediately follow the @Participants / @Options headers (or another @ID). The @ID block must come directly after @Participants (and the optional @Options), with no other header in between. A changeable header such as @Comment between @Participants/@Options and the @ID block is an ordering violation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E548](E548.md) | E548: @ID header out of order | error | ✅ |

## header_validation (E5x)

The same speaker code is declared more than once in the @Participants header. Each participant must be declared exactly once; a repeated speaker code is a declaration error.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E549](E549.md) | E549: Duplicate speaker declaration | error | ✅ |

## header_validation (E5x)

The @Participants header ends with a trailing comma: a stray comma after the last participant, with no participant following it. The participant list is comma-separated (CHI Target_Child, MOT Mother), so a comma with nothing after it is a dangling separator. This is distinct from an empty @Participants header; the header has participants, it just has an extra comma at the end.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E550](E550.md) | E550: Trailing comma in @Participants | error | ✅ |

## header_validation (E5x)

The @Media header's unlinked status declares that the transcript is not time-aligned to the media file. Timing evidence anywhere in the transcript contradicts that declaration: either the transcript really is aligned (so unlinked must be removed), or the timing tier is stale (so it must be removed). This is the inverse of E544 (declared linkage without timing evidence).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E552](E552.md) | E552: @Media declares unlinked but transcript carries timing | error | ✅ |

## validation (E6x)

A dependent tier (typically %mor) had parse errors during lenient recovery, so the validator cannot verify alignment between tiers. Alignment checks (main↔%mor, %mor↔%gra) are skipped for the affected utterance. This is a warning, not an error, the file still parses, but alignment correctness is unverified for tainted tiers.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E600](E600.md) | E600: Tier alignment skipped due to parse errors | error | ✅ |

## validation (E6x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E601](E601.md) | generated from corpus | error | ✅ |

## validation (E6x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E602](E602.md) | generated from corpus | error | ✅ |

## tier_validation (E6x)

A %tim dependent tier contains content that does not match the expected time format. The tier parses successfully but the invalid content is stored as Unsupported and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E603](E603.md) | E603: Invalid %tim Tier Format | error | ✅ |

## Dependent tier parsing (E6x)

Empty GRA relation

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E604](E604.md) | E604: Empty GRA relation | error | ✅ |

## tier_validation (E6x)

A %gra (grammatical relations) tier appears without a corresponding %mor (morphology) tier. According to CHAT rules, %gra depends on %mor and cannot exist independently.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E604](E604.md) | E604: %gra Tier Without %mor Tier | error | ✅ |

## tier_validation (E6x)

An utterance contains a dependent tier with a label that is not a standard CHAT tier name and does not follow the %x user-defined tier naming convention. The file parses successfully but the tier is stored as DependentTier::Unsupported and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E605](E605.md) | E605: Unsupported Dependent Tier | error | ✅ |

## Temporal validation (E7x)

Each utterance's first media bullet must have a start time greater than or equal to the previous utterance's first bullet start time (for the same speaker). Corresponds to CLAN CHECK Error 83.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E701](E701.md) | speaker start | error | ✅ |

## Dependent tier parsing (E7x)

Invalid MOR chunk format - missing |

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E702](E702.md) | missing | | error | ⏳ |

## validation (E7x)

A single speaker has consecutive utterances with overlap markers (⌈⌉/⌊⌋) that overlap with each other. Overlap markers should indicate simultaneous speech between different speakers, not self-overlap.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E704](E704.md) | overlap, overlapping overlap markers | error | ✅ |

## Alignment count mismatch (E7x)

Mor count mismatch - too few items

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E705](E705.md) | too few items | error | ✅ |

## Alignment count mismatch (E7x)

Mor count mismatch - too many mor items

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E706](E706.md) | too many mor items | error | ✅ |

## Alignment terminator mismatch (E7x)

The %mor tier has a terminator but the main tier does not, or vice versa. One tier ends with a sentence-final punctuation mark while the other does not.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E707](E707.md) | E707: Mor terminator presence mismatch | error | ⏳ |

## Dependent tier parsing (E7x)

A grammar relation on the %gra tier is malformed, missing an index, head, or relation label, or containing non-integer values where integers are expected. The %gra tier format is index|head|RELATION for each word.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E708](E708.md) | E708: Malformed grammar relation on %gra tier | error | ⏳ |

## validation (E7x)

A %gra relation uses an invalid index. %gra indices are 1-indexed: the first word is 1, and 0 is reserved for the ROOT attachment in the dependent slot (n|0|ROOT). Using 0 in the first (index) slot of a relation triggers E709.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E709](E709.md) | E709: Invalid grammar index | error | ✅ |

## Dependent tier parsing (E7x)

Invalid GRA format

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E710](E710.md) | E710: Invalid GRA format | error | ✅ |

## Mor content validation (E7x)

A %mor word has an empty stem, POS category, prefix, or suffix. Every morphosyntax item on the %mor tier must have a non-empty POS category and a non-empty stem at minimum.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E711](E711.md) | E711: Mor empty content | error | ⏳ |

## validation (E7x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E712](E712.md) | generated from corpus | error | ✅ |

## validation (E7x)

A %gra relation has a head index that falls outside the valid range 0..=N, where N is the number of %mor chunks in the utterance. Index 0 is reserved for the ROOT head; otherwise the head index must point to an existing chunk.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E713](E713.md) | E713: Gra head index invalid | error | ✅ |

## Alignment count mismatch (E7x)

The %pho (actual phonology) tier has fewer alignable tokens than the main tier. Each main-tier word must have a corresponding %pho token.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E714](E714.md) | E714: %pho alignment count mismatch, too few tokens | error | ✅ |

## Alignment count mismatch (E7x)

The %pho (actual phonology) tier has more alignable tokens than the main tier. Remove the extra %pho tokens so counts match.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E715](E715.md) | E715: %pho alignment count mismatch, too many tokens | error | ✅ |

## Alignment terminator mismatch (E7x)

The %mor tier has a terminator that does not match the main tier's terminator. Both tiers have terminators, but they differ (e.g., main tier ends with "?" but %mor ends with "."). This typically indicates stale or incorrectly cached morphosyntax data.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E716](E716.md) | E716: Mor terminator value mismatch | error | ✅ |

## Alignment count mismatch (E7x)

Sin count mismatch - too few sin tokens

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E718](E718.md) | too few sin tokens | error | ✅ |

## Alignment count mismatch (E7x)

Sin count mismatch - too many sin tokens

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E719](E719.md) | too many sin tokens | error | ✅ |

## Alignment count mismatch (E7x)

The number of %mor chunks does not equal the number of %gra relations for an utterance. %gra aligns 1-to-1 with %mor chunks (not items, a %mor item with post-clitics produces multiple chunks).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E720](E720.md) | Gra count mismatch | error | ✅ |

## validation (E7x)

%gra tier indices must be sequential (1, 2, 3, ..., N). Non-sequential indices indicate a malformed dependency structure.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E721](E721.md) | sequential index | error | ✅ |

## validation (E7x)

%gra tier has no ROOT relation. Every %gra tier must have exactly one relation with head=0 or head=self (the ROOT of the dependency tree).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E722](E722.md) | E722: GRA has no ROOT | error | ✅ |

## validation (E7x)

%gra tier has multiple ROOT relations. Every %gra tier should have exactly one ROOT (relation with head=0 or head=self).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E723](E723.md) | E723: GRA has multiple ROOTs | error | ✅ |

## validation (E7x)

A %gra tier contains a circular dependency where following parent pointers creates a cycle. This violates the fundamental requirement that dependency structures must form a tree.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E724](E724.md) | E724: GRA has circular dependency | error | ✅ |

## Alignment count mismatch (E7x)

The %xmodsyl tier word count does not match the %mod tier word count. Each word-level entry in %xmodsyl must correspond one-to-one with a word-level entry in %mod.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E725](E725.md) | E725: Modsyl tier word count does not match mod tier | error | ✅ |

## Alignment count mismatch (E7x)

The %xphosyl tier word count does not match the %pho tier word count. Each word-level entry in %xphosyl must correspond one-to-one with a word-level entry in %pho.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E726](E726.md) | E726: Phosyl tier word count does not match pho tier | error | ✅ |

## Alignment count mismatch (E7x)

The %xphoaln tier word count does not match the %mod tier word count. Each word-level entry in %xphoaln must correspond one-to-one with a word-level entry in %mod.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E727](E727.md) | E727: Phoaln tier word count does not match mod tier | error | ✅ |

## Alignment count mismatch (E7x)

The %xphoaln tier word count does not match the %pho tier word count. Each word-level entry in %xphoaln must correspond one-to-one with a word-level entry in %pho.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E728](E728.md) | E728: Phoaln tier word count does not match pho tier | error | ✅ |

## Alignment count mismatch (E7x)

The %mod (model/target phonology) tier has fewer alignable tokens than the main tier. Each main-tier word must have a corresponding %mod token.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E733](E733.md) | E733: %mod alignment count mismatch, too few tokens | error | ✅ |

## Alignment count mismatch (E7x)

The %mod (model/target phonology) tier has more alignable tokens than the main tier. Remove the extra %mod tokens so counts match.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E734](E734.md) | E734: %mod alignment count mismatch, too many tokens | error | ✅ |

## Phon syllabification content (E7x)

Every %xmodsyl/%xphosyl unit must be one phone, an ASCII ':', then one constituent code.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E735](E735.md) | E735: Syllabification unit is not a phone:CODE pair | error | ✅ |

## Phon syllabification content (E7x)

Constituent codes on %xmodsyl/%xphosyl must be one of O N C L R E A D U.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E736](E736.md) | E736: Illegal syllable constituent code | error | ✅ |

## Phon syllabification content (E7x)

Stripping :CODE from each %xmodsyl unit must reproduce the corresponding %mod word. A pause filler ((.), (..), (...)) on %xmodsyl must mirror the same pause token as the %mod word at that position.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E737](E737.md) | E737: Modsyl does not reproduce the mod word | error | ✅ |

## Phon syllabification content (E7x)

Stripping :CODE from each %xphosyl unit must reproduce the corresponding %pho word. A pause filler ((.), (..), (...)) on %xphosyl must mirror the same pause token as the %pho word at that position.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E738](E738.md) | E738: Phosyl does not reproduce the pho word | error | ✅ |

## Phon phone alignment (E7x)

Every %xphoaln pair has exactly one ↔ with a non-null phone on at least one side.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E739](E739.md) | E739: Phoaln pair is malformed | error | ✅ |

## Phon phone alignment (E7x)

Concatenating the model (left) sides of %xphoaln, skipping ∅, must reproduce the %mod word. The comparison is segment-level: stress markers (\u{02C8}, \u{02CC}) and syllable-boundary notation (Phon's ^, IPA's .) in either string are ignored, since the alignment pairs carry bare segments while the source word may carry suprasegmental and boundary notation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E740](E740.md) | E740: Phoaln model side does not reproduce the mod word | error | ✅ |

## Phon phone alignment (E7x)

Concatenating the actual (right) sides of %xphoaln, skipping ∅, must reproduce the %pho word. The comparison is segment-level: stress markers (\u{02C8}, \u{02CC}) and syllable-boundary notation (Phon's ^, IPA's .) in either string are ignored, since the alignment pairs carry bare segments while the source word may carry suprasegmental and boundary notation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E741](E741.md) | E741: Phoaln actual side does not reproduce the pho word | error | ✅ |

## Phon phone interval (E7x)

Each %xphoint phone interval must have start strictly less than end.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E742](E742.md) | E742: Xphoint bullet has start >= end | error | ✅ |

## Phon phone interval (E7x)

%xphoint interval start times must be non-decreasing across the tier.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E743](E743.md) | decreasing | error | ✅ |

## Phon phone interval (E7x)

The first start and last end of %xphoint must lie within the *SPK: media bullet (1 ms tolerance).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E744](E744.md) | E744: Xphoint intervals fall outside the media bullet | error | ✅ |

## Phon phone interval (E7x)

Concatenating a %xphoint group's phones must reproduce the corresponding %pho word.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E745](E745.md) | E745: Xphoint group does not reproduce the pho word | error | ✅ |

## Phon phone interval (E7x)

%xphoint must have exactly one ' / '-separated group per %pho word.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E746](E746.md) | E746: Xphoint group count does not match the pho word count | error | ✅ |

## Media bullets (E7x)

A media bullet timestamp is written with a leading zero before another digit (for example 012_200). CHAT bullet times are plain millisecond integers; a leading zero is an illegal time representation (CLAN CHECK error 90, check_getMediaTagInfo res 3). A bare 0 timestamp (for example 0_200) is legal: the rule fires only when a 0 is followed by another digit.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E748](E748.md) | E748: Leading zero in bullet timestamp | error | ✅ |

## Main tier separators (E7x)

A comma on a speaker tier must be followed by a space or end-of-line (CLAN CHECK error 92, "Item ',' must be followed by space or end-of-line.", check.cpp 4309-4320). Writing hey ,you glues the comma to the next word. The rule fires only when the next in-order item is a word starting at the byte immediately after the comma; constructs that put any other character after the comma (group <, overlap marks, CA marks) are not flagged, matching CLAN's CA exemptions conservatively.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E749](E749.md) | E749: Comma glued to the following word | error | ✅ |

## Main tier groups (E7x)

A space directly after the opening < or directly before the closing > of an angle-bracket group (< dog> or <dog >) is invalid (CLAN CHECK error 160, "Space character is not allowed after '<' or before '>' character.", check.cpp 4300/4306). The grammar tolerates the whitespace as an explicit optional whitespaces CST node so the parse recovers, but the construct is invalid CHAT; before this rule the parser silently DROPPED that whitespace, so accepted files were also being silently rewritten on normalize.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E750](E750.md) | bracket group delimiters | error | ✅ |

## Main tier separators (E7x)

A pause marker opening directly attached to the end of a word with no space (hello(.)) is invalid (CLAN CHECK error 57, "Please add space between word and pause symbol: '('.", check.cpp 4437). Pauses are free-standing content items and must be space-delimited from words.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E751](E751.md) | E751: Pause glued to the preceding word | error | ✅ |

## header_validation (E7x)

The transcript carries timing evidence (main-tier bullets, or a positional %wor timing sidecar), but no @Media header declares the media timeline those timestamps index. A timestamp into an undeclared recording fails to make sense: consumers cannot resolve what the offsets refer to. This is the inverse direction of E544 (@Media declares linkage but no timing evidence exists) and corresponds to CLAN CHECK error 112 ("Please add "unlinked" to @Media header.", check.cpp 3927, check_getOLDMediaTagInfo res==6).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E752](E752.md) | E752: Timing bullets without an @Media header | error | ✅ |

## Main tier words (E7x)

A word whose entire spoken material sits inside segment-repetition delimiters (↫...↫, U+21AB) marks the repetition of a segment of a word that is not there: the notation presumes a host word (a stem) outside the repeated span, as in ↫p↫parents ("p-, parents"). A fully wrapped word asserts a repetition of nothing and fails to make sense. Corresponds to CLAN CHECK error 151 ("This word has only repetition segments.", check.cpp check_isThereStem), which only the GUI CLAN build enforces; chatter adopts the rule in its own semantics (maintainer ruling, 2026-07-15).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E753](E753.md) | E753: Word consisting only of repetition segments | error | ✅ |

## Main tier words (E7x)

The @l special form marks a single spoken LETTER (b@l, reading a letter aloud). Multi-character content has its own form, @k (letter sequence) or @ls (letter plural), so a stem of more than one character under @l is a mis-marked form: ab@l should be ab@k. Replicates CLAN CHECK error 76 ("There should be only one letter before @l.", check.cpp check_isOneLetter), per maintainer ruling 2026-07-14: replicate CHECK's one-character rule now; the deeper digraph question (Spanish ch, Dutch ij: one letter orthographically, two characters) is logged for the corpus authority and NOT decided here.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E754](E754.md) | E754: Letter form @l with more than one letter | error | ✅ |

## header_validation (E7x)

A [- CODE] precode marks a whole utterance as being in another language: substantial language presence in the transcript. The @Languages header declares the transcript's substantial languages, so an utterance-level language missing from it leaves the header misrepresenting the transcript. Matches CLAN CHECK error 152 ("Language is not defined on @Languages header tier."). Ruled 2026-07-15 (maintainer decision, docs/design/2026-07-15-at-s-language-declaration-decision.md, part 3): declaration IS required at utterance level, deliberately UNLIKE word-level @s:CODE insertions, which remain free (part 1 of the same ruling; the corpus grounding found 0 of 7,167 precode-bearing files violate this invariant while 854 files legitimately use undeclared word-level codes).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E755](E755.md) | E755: Utterance language not declared in @Languages | error | ✅ |

## Dependent tier validation (E7x)

A user-defined %x tier whose content is empty or whitespace-only declares nothing: the line asserts an annotation that is not there and fails to make sense. Formerly W601, which carried a warning-prefixed code while firing as a hard error (its doc comment even said "intentionally warning-level"); the maintainer ruling of 2026-07-16 resolved the taxonomy contradiction by keeping the rejection and giving it an honest E-number. Real CLAN has no analogue (a truly empty tier draws only structural errors); zero kept files carry the construct, so the rename has no corpus impact. W601 is retired and not reused.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E756](E756.md) | defined tier | error | ✅ |

## Main tier separators (E7x)

A bracketed code's closing ] directly attached to the start of the next word with no space (hello [/]x) is invalid (CLAN CHECK error 19, "Illegal use of delimiter in a word." / "Or a SPACE should be added after it."). Bracketed codes are free-standing items and must be space-delimited from what follows. The parse itself is unambiguous (the retrace closes at ] and x becomes a separate word), which is exactly why this is a STYLE rule: sloppy but readable source that must still be rejected so the corpus stays canonically spaced.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E757](E757.md) | E757: Bracketed code glued to the following content | error | ✅ |

## Tier structure (E7x)

Every CHAT line has the shape label:<tab>content, where the separator between the label and the content is a colon and exactly one tab. Any further whitespace after that tab is not content: it is trailing whitespace of the separator. In a file without @Options: CA, a trailing space there is invalid (CLAN CHECK error 123, "Illegal character '' found in tier text. If it CA, then add "@Options: CA"").

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E758](E758.md) | CA file) | error | ✅ |

## Main tier annotations (E7x)

Postfix annotations (retraces [/] [//] [///] [/-], overlap markers [<] [>] and their indexed forms, replacements [: text], and the quotation marker ["]) scope over the material that PRECEDES them. An utterance whose content BEGINS with one of these codes (*CHI: [/] we go home .) is malformed: the annotation has no host item, so its meaning is undefined. This matches CLAN CHECK error 52 ("Item '%s' must be preceded by text."), whose trigger set is exactly a leading bracket code starting with <, >, :, /, or ".

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E759](E759.md) | E759: Annotation at utterance start has nothing to attach to | error | ✅ |

## Dependent tier validation (E7x)

A %mor item is pos|stem (with optional prefixes, clitics, and suffixes). An item that BEGINS with the | separator (|we) declares no part of speech at all: the field before the pipe is empty, which is never meaningful %mor content. CLAN CHECK rejects it as error 11 ("Symbol is not declared in the depfile."): in depfile-era terms the empty symbol is undeclared; the modern reading is simply that the POS field is required.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E760](E760.md) | of | error | ✅ |

## Alignment count mismatch (E9x)

Unknown error

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E999](E999.md) | E999: Unknown error | error | ⏳ |

## validation (W1x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [W108](W108.md) | generated from corpus | error | ✅ |

## Warnings (W6x)

A user-defined dependent tier (%x...) uses a label that matches a known standard tier name. For example, %xpho should be updated to %pho since pho is now a recognized standard tier. This is a warning to encourage migration from legacy experimental naming to the current standard.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [W602](W602.md) | W602: Deprecated experimental tier name | error | ⏳ |

