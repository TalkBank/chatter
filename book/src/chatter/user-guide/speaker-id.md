# Speaker-ID (`chatter speaker-id`)

**Status:** Current
**Last modified:** {{git-dates:page}}

This experimental command applies speaker mappings to CHAT. Renames change
speaker codes and participant/ID metadata; drops remove that speaker's turns
and corresponding metadata. It does not analyze voices, correct transcription,
prove recording identity or certify the output for publication.

This guide covers deterministic judgment. The separate holistic judgment engine
can call an external LLM and writes review-gated pending suggestions rather than
relabeled CHAT; see the [CLI reference](cli-reference.md). Do not treat local
deterministic checks as provider qualification.

## Identification modes

For deterministic judgment, choose exactly one mapping source.

### 1. Reference mode

Supply a reference containing the known anchor speaker. Lexical similarity
selects a donor speaker for dropping; all other donor speakers are renamed to
the single supplied `CODE:ROLE`. Dropping is intended for later assembly with
the retained reference, not for producing a complete standalone transcript.

```bash
chatter speaker-id donor.cha \
  --reference reference.cha --anchor CHI --inserted-role INV:Investigator \
  --write-match-report match-report.json \
  --write-override overrides.toml --write-pending pending.toml \
  -o relabeled.cha
```

These are placeholder paths: establish source correspondence and speaker evidence
before using this operation on real inputs. Reference mode's single inserted
identity is not a safe way to distinguish several non-anchor speakers.

The default confidence threshold is two. No lexical information refuses;
positive winner/zero runner-up is unbounded. Low confidence exits four without
relabeled CHAT, and writes a pending entry when requested.
Inspect the support counts and evidence rather than lowering a threshold just
to obtain output.

### 2. Explicit-mapping mode

```bash
chatter speaker-id donor.cha \
  --mapping "PAR0=INV:Investigator,PAR1=drop" -o relabeled.cha
```

Assignments are comma-separated `OLD=CODE:ROLE` or `OLD=drop`.
Malformed/duplicate source assignments refuse.
Speakers absent from the map currently pass through unchanged; unused mapping
keys do not prove that those speakers existed in the input. Review the whole
mapping yourself. Target uniqueness and complete source coverage are not
validated here.

`--write-override` is rejected in explicit mode. Use
[adjudication](merge-workflow.md) to persist operator decisions; an explicit
mapping invocation does not automatically preserve scores from an earlier run.

### 3. Override-file mode

```bash
chatter speaker-id donor.cha --override-file overrides.toml \
  --session-id session-demo -o relabeled.cha
```

The session ID defaults to the input basename stem. The requested entry must
exist and its renames must have role records. Replay applies the saved mapping
without rerunning lexical identification or updating the original record's
mode, operator or timestamp. A session key is not proof of recording identity.

## Outputs and failures

Without `-o`, relabeled CHAT goes to stdout. An output path is written with
replacement semantics, not exclusive creation: choose a new destination and
keep source files intact.

Input parse/validation and I/O failures exit one; mapping/precondition refusals
exit two; low reference confidence exits four. Unsupported override versions
also exit two. Complete output validity and speaker correctness are not proved
by exit zero: mapping application serializes an edited model without a new
output-validation transition.

The transform preserves retained content in the model and updates
`@Participants` and `@ID` for mapped speakers. It is not a general raw-byte
editing API. A supplied specific role can replace the participant name field.
Do not generalize fixture byte-stability checks to all source formatting.

Reference `--write-override` keys entries by input basename, with operator
from `USER` or `unknown`. It writes the decision before writing CHAT output,
so a later output failure can leave the override changed.
`--session-id` selects replay entries; it does not rename reference-mode
output records. Pending writes append requests and do not deduplicate repeated
runs. See [Override File Format](../integrating/merge-overrides.md).

After a low-confidence refusal, review the pending decision and prepare
adjudication input. On successful adjudication, replay the resulting override
into a new CHAT path. Validate and inspect that output separately.
No removed merge CLI or automatic two-pass orchestrator is implied.

## Algorithm (reference mode)

### Token cleaning

Both the reference anchor's bag of words and each input speaker's
bag of words are built by walking the typed CHAT AST. Word leaves supply
their model-owned `cleaned_text`; the matcher does not independently strip
CHAT markup or run a parallel text parser. It trims each value, applies ASCII
lowercasing, and retains only entirely ASCII-alphabetic tokens of at least
two characters. Non-word items do not enter the bag.

Comma, tag-question and vocative separators contribute no tokens. The reference
separator control checks exact support counts and verifies that a stricter
confidence threshold refuses the same evidence without changing it.

For a `ReplacedWord` (`word [: replacement]`), the matcher counts the original
spoken word, not the replacement targets. Thus `goed [: went]` contributes
`goed`, while `child [: children]` contributes `child`. The same cleaning and
eligibility rules apply to these originals as to other words. This policy is
shared by reference and input speakers; it does not alter either transcript or
claim that an ASR system necessarily preserves the spoken form.

The ASCII restriction is a limitation of the matcher, not a CHAT validity rule:
accented and non-Latin words are excluded. For example, the Mandarin reference
transcript yields no lexical information even when compared with itself, and
the operation refuses with `low_confidence` / `no_information`; it must not
promote the first speaker in document order into an accepted identity match.
Mixed-language speech is scored only on the eligible tokens, so inspect the
reported token counts rather than treating a high score as multilingual evidence.
Both sides use the same policy, and identification does not rewrite the input.

### Multiset Jaccard

For two bags-of-words `A` and `B` (counted multisets):

```text
J(A, B) = sum_w min(A[w], B[w])  /  sum_w max(A[w], B[w])
```

Range `[0, 1]`. The multiset (rather than set) form rewards
speakers who say similar things to the anchor in similar volume,
not just speakers whose vocabulary happens to intersect.

### Decision

Scores are ranked with document order retained for ties. The reference must
contain anchor utterances, and the donor must contain at least two distinct
utterance speakers. The leading score supplies the drop candidate; the
runner-up supplies the margin comparison. A zero/zero comparison is
`NoInformation`, not an accepted tie.

The checked threshold admits finite values at least one. A finite ratio must
meet that threshold; an unbounded positive match does. Neither a high ratio nor
a large shared token count independently proves speaker identity.

### Match-evidence report

`--write-match-report` preserves the observations behind the scalar score and
the typed reason when matching could not begin.
This matters because a ratio alone cannot distinguish a winner supported by
one shared token from one supported by hundreds. The JSON schema records, for
each donor speaker, `reference_tokens`, `donor_tokens`, `shared_tokens`,
`union_tokens`, and the derived `score`. Its margin is one of
`no_information`, `finite`, or `unbounded`; no-information and a zero-scoring
runner-up are not represented by floating-point sentinels.

The outer outcome is `accepted`, `low_confidence`,
`reference_missing_anchor`, `donor_too_few_speakers`, or `input_rejected`.
Matched outcomes contain the lexical report; other outcomes cannot pretend to
contain one. Input rejection records donor versus reference, the typed
pipeline-failure category, and TalkBank diagnostic codes when available.

Within `input_rejected`, `failure_kind: "incomplete_validation"` means missing
or recovered parser provenance prevented complete validation. This differs from
`"validation"` (the model failed validation) and `"internal_failure"` (the tool
failed). Imported JSON may have unchanged model content but lack parser
provenance, so an incomplete result can have an empty `diagnostic_codes` array.
It still cannot supply a `match_report`. Consumers must handle this distinct
category rather than treating every input rejection as invalid CHAT. The outer
outcome and schema version remain unchanged.

The option is intended for audit and calibration tooling. It does not change
which speaker wins or the confidence threshold. Chatter stages the complete
JSON beside its destination and persists it without clobbering, so a later run
cannot silently replace the evidence used for an earlier decision and a failed
write cannot leave a partial final report. The no-clobber persist is not
guaranteed to be atomic on every platform; an interrupted persist can leave
the staging link behind, but it never replaces an existing report.

If `margin < --confidence-threshold` (default 2.0), the command
exits with code 4 and prints per-speaker scores to stderr. The
operator must inspect, adjudicate, and re-run with
`--mapping` or `--override-file`.

## Override file format

Use [Merge Override File Format](../integrating/merge-overrides.md) for the
version-two TOML representation, per-speaker roles, optional evidence and
replacement/recovery limits. The override wire margin is numeric; it is not
the tagged margin in the JSON match report.

## Implementation and evidence

The library owns lexical evidence and mappings in
`crates/talkbank-transform/src/speaker_id/`; CLI mode dispatch and persistence
live in `crates/chatter/src/commands/speaker_id/`.

[Merge Test Plan](../../architecture/merge-test-plan.md) identifies actual
integration and canonical-reference owners. Finite fixtures check lexical,
mapping and wire contracts, not universal speaker identity or multilingual
accuracy. No calibration-success census is claimed here.
