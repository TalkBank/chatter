# `%wor` Timing Semantics

**Status:** Current
**Last modified:** {{git-dates:page}}

## Purpose

`%wor` is a timing sidecar over a named subset of main-tier word slots. It is
not an independent lexical transcript and it is not a structural dependent-tier
alignment like `%mor`, `%pho`, or `%sin`.

The main tier owns lexical identity. `%wor` contributes an optional inline
media bullet for each selected position. The visible word printed on `%wor` is
display material and optional corroborating evidence. It can prevent stale
timing reuse, but it never supplies lexical identity.

Generation and serialization are different operations. Newly generated `%wor`
words use the named cleaned-text display convention. Serializing an already
parsed tier preserves its typed word structure, including shortening, compounds
and markers, then writes its timing bullet. It must not silently regenerate a
parsed display token from cleaned text. The selective-name and pseudonymizer
source/expected references exercise this distinction through fragment,
round-trip, normalization and downstream transform contracts.

WOR does not carry main-tier scoped error, explanation, replacement or retracing
annotations. The [CHAT manual's word-level diarization section](https://talkbank.org/0info/manuals/CHAT.html#Wor_Tier)
excludes comments and retracing marks from echoed words. Both parsers reject
these annotated tiers as timing models and retain complete original source as
`UnparsedWor` at the file boundary. They must not consume an annotation and then
serialize a plain word in its place. A bare action `0` cannot become an empty
word; omission words such as `0dog` retain their bodies. Word-internal structure remains supported;
this is distinct from generating cleaned display words from a main tier.

Actual timing presence is a separate typed question from correspondence.
`WorTier::timing_evidence()` returns `Absent` or a `RecordedWorTiming` carrying
the first real word-level bullet. Media validation uses this state directly.
Equal counts with no bullets are not timing evidence, while a real bullet
remains timing evidence even when counts drift. Alignment processing is not
required to observe a bullet already in the typed CHAT model. A `%wor` tier
cannot carry a trailing tier-level bullet: the grammar does not permit that
state, and `WorTier` cannot construct or serialize it.

Each parsed bullet consumes the immediately preceding untimed word. A leading
bullet, a bullet after a separator, or a second bullet cannot be associated
without loss. Both file parsers retain such a tier as `UnparsedWor`, with its
complete original source, and refuse it as timing evidence. Numerically
unrepresentable timing also rejects the whole parsed tier; valid sibling timing
remains in the retained source. Fragment APIs requiring `WorTier` reject these
inputs. Untimed words remain supported. Recovery is not a partial timing model.

Complete CHAT admission rejects a word bullet that ends at or before it starts
(E362). Untimed words remain legal partial timing, and there is
no cross-word monotonicity, word-overlap or cross-speaker restriction (see the
leniency policy, Decision 10). A consumer that needs positive intervals, such as
`assess_wor_timing_sequence`, additionally requires every slot to be timed. Each word bullet
carries its own source span, so E362 is located inside its `%wor` tier.

These are word-level rules, not the main-tier timeline rules. See
[Bullet Validation](bullet-validation.md#current-policy-by-timing-surface)
for the global E362 start-order check, per-speaker E701/E704, media linkage,
and the distinction between overlapping words and overlapping utterances.

The adaptive source-bound replacement plan (`WordTimingPlan::PreferRetained`)
first removes tiers with owned lowering failures or located validation errors.
If errors cannot be attributed to a word tier, it removes all remaining word
tiers and records that uncertainty. Removal is not proof that each discarded
tier was independently defective:

```mermaid
flowchart TD
    L[Lower each %wor once] -->|own lowering failed| R1[Removed: OwnLowering]
    L -->|clean| I[Move entry into the document]
    I --> V{Validate the candidate}
    V -->|valid| A[Admit; removed tiers decide the disposition]
    V -->|errors inside word tiers| R2[Remove those tiers: LocatedValidation]
    R2 --> V
    V -->|errors in no word tier| R3[Remove every remaining tier: UnattributedValidation]
    R3 --> V
    V -->|rejected, no word tier left| F[Refuse: a retained fault]
```

A validation diagnostic belongs to a tier when its span lies inside that tier's
source span; the tiers holding an error are removed with every diagnostic
inside them bound to the receipt (`RemovedTier::cause`), and the reduced
document is validated again. Errors located in no word tier remove every word
tier still retained, recorded as unattributed with the shared diagnostics; any
retained fault then still refuses. That fallback is the old remove-all
behaviour: when removal makes such an error go away, it shows only that the
error depended on word tiers, not which one, so the receipt says
"unattributed" rather than naming a cause. If a removed tier carried a recorded word
bullet, the next validation runs in the timing-regeneration phase, so a file
whose only timing was in removed tiers becomes a pending regeneration (E544
obligation), while one that keeps some timing is a complete replacement. A
preservation plan has no exemption of any kind.

## Current membership policy

The canonical policy is `FilteredLexicalV1`. A typed
`WorMainTierProjection` is its single traversal owner. Both `%wor` generation
and timing binding consume that projection, so a policy edit cannot update one
path and leave the other behind.

| Main-tier content | Current membership |
|---|---|
| Regular word | Included |
| Filler such as `&-um` | Included |
| Retraced regular word | Included |
| Original surface of a replacement | Included when otherwise eligible |
| Phonological fragment such as `&+w` | Excluded |
| Nonword such as `&~gaga` | Excluded |
| `xxx`, `yyy`, or `www` | Excluded |
| Omission | Excluded |
| Separator or terminator | Excluded |

This policy is explicit because the meaning of one-to-one correspondence
depends on which main-tier items count. A future research policy must receive a
new name and separate evaluation. It must not silently change
`FilteredLexicalV1`.

## Typed binding and correspondence states

Consumers call `bind_wor_timing(main, wor)`. The data state is one of:

- `Missing`: no `%wor` tier exists. This is distinct from a present empty tier.
- `Drifted`: the selected main-tier slot count differs from the physical `%wor`
  word-entry count. No positional slots are exposed.
- `CountMatched`: counts match under the named policy. This state permits a
  positional comparison but exposes no timing slots. Equal counts alone do not
  prove that a parsed legacy tier and the current main tier share origin.

Consumers pass `CountMatched` to `corroborate_wor_timing`. The next state is:

- `Uncorroborated`: one or more `%wor` display tokens differ from the canonical
  display tokens generated from the current main-tier projection. The state
  exposes exhaustive mismatch diagnostics but no timing slots.
- `Corroborated`: every display token matches the canonical generated token at
  its count-matched position. Only this state exposes positional timing slots.

Each corroborated slot has:

- a borrowed typed main-tier `Word` and its `cleaned_text`, which remain the
  only lexical identity;
- `WorSourceContent`, retaining the producing main tier and that word's
  top-level content position, including the whole root of a nested word;
- the exact corroborating `%wor` `Word`, exposed by `wor_word()` for
  source-bound display-token updates that preserve timing and metadata;
- `Timed(WorRecordedInterval)` when the corresponding `%wor` entry has an
  inline bullet;
- `Unaligned` when the entry exists but has no inline bullet.

This transition detects same-count edits when they change at least one
canonical display token. It cannot establish immutable common origin: repeated
tokens can be exchanged invisibly, and CHAT does not carry a generation
identifier. The state is therefore named `Corroborated`, not `Aligned` or
`Proven`.

Selective transforms must finish corroboration before proposing `%wor`
changes. They derive replacement display text from the selected main-tier
word, not a second name search over `%wor`. Count drift or lexical drift
requires an explicit refusal/review path, not an independently guessed pairing.
The `tiers/wor-drift.cha` reference file witnesses both count-drift directions
and same-count lexical drift; these are stale sidecars in otherwise valid CHAT.

## Temporal sequence transition

Lexical corroboration is necessary but not sufficient for algorithms that need
a word-timing hull. A corroborated tier may still contain an untimed slot, a
zero or backwards interval, or adjacent word intervals that overlap.

Consumers call `assess_wor_timing_sequence(corroborated)` for the next checked
transition. It returns one of:

- `Empty`: the sidecar is present and corroborated, but the membership
  policy selected no words. There is no hull.
- `Rejected`: the binding contains one or more `Unaligned` or
  `NonPositiveInterval` issues. The diagnostic state exposes typed slot indices
  and numeric evidence, but no partial hull.
- `Complete`: every selected word has a positive interval. Only this state
  exposes borrowed main words paired with typed recorded intervals, a min/max
  `WorTimingHull`, and every typed adjacency relation.

Each adjacency is `Gap`, `Touching`, `Overlap`, or `BackwardStart`. Overlap and
backwards starts remain visible evidence, but do not erase a hull that is still
mechanically defined by the recorded extrema. An algorithm that requires
common origin, acoustic accuracy, non-overlap, or monotonic starts must state
and enforce that later policy over additional evidence or the relation types.

These alternatives have an order: a start earlier than the previous start is
`BackwardStart`, even if the intervals also overlap. Otherwise the current
start is compared with the previous end to distinguish overlap, touching and
gap. The reported overlap duration is the previous end minus the current
start, not necessarily the intersection length when one interval contains
the other. Consumers needing intersection geometry must check both ends.

The assessment transition is infallible because its control flow makes the
remaining construction failure unrepresentable. An empty binding returns
`Empty`. A nonempty binding assesses the first slot before it can construct a
complete accumulator. A first-slot failure starts a rejected accumulator; a
first complete slot is the required seed for the hull. No caller or internal
branch can construct a nonempty complete sequence without that seed.

`WorSlotIndex`, `WorMediaOffsetMs`, `WorDurationMs`, and `WorTimingHull` have
private constructors. Callers cannot construct these wrappers from arbitrary
integers. Recorded offsets and durations derived by subtraction remain different
types. These copyable numeric values do not, however, carry a source identity:
an index issued for one assessment is not type-branded to that assessment, and
equal coordinates do not establish a common recording. Keep them associated
with their producing result. `WorSourceContent`, described below, separately
retains the actual main-tier borrow and root position.

The complete slot's timing accessor returns `WorRecordedInterval`, not the
original `Bullet`, so its checked timing surface does not require raw integer
arithmetic. This is an API contract, not a restriction on inspecting the input
model: count-matched pairs and corroborated `wor_word()` still expose borrowed
words, and callers may retain the original tier. Reading those bullets directly
does not establish lexical corroboration or temporal completeness.

This is deliberately stricter than ordinary CHAT validation. CHAT can retain
legacy or partially aligned data. A timing-consuming algorithm needs an
explicit admission contract and must not infer one from the fact that the
file parsed.

The count types for the main sequence and the physical `%wor` sequence are
different newtypes. Callers cannot swap them accidentally. Constructors for
the proof states and counts are private.

The count-matched state is deliberately named for the fact it actually proves.
It does not claim common origin and it cannot expose timing. The corroborated
state is also deliberately limited: canonical display equality provides useful
evidence against stale reuse, but serialized CHAT carries no immutable
generation identity. Acoustic or common-origin qualification requires later
evidence and a different state.

The binding borrows both typed sequences until correspondence is decided.
Corroboration retains main-tier words as lexical identity and copies the two
recorded media offsets into private-constructor coordinate types. It does not
clone a temporary generated `%wor` tier or reduce structured lexical identity
to a string.

The main-tier projection owns word and separator selection in source order.
Its constructor is private to `MainTier::wor_projection()`. Generation derives
the visible tier from this capability, and binding consumes the same capability
to obtain lexical slots. There is no independent counter or generator whose
agreement must be tested at runtime. Count drift between a parsed legacy tier
and its main tier remains a real `Drifted` data state.

The projection also issues `WorSourceContent` for each selected word. Its
private construction binds the root index to the immutable producing main tier;
`source()`, `index()` and `content()` read that association. Locations compare
by producing main-tier identity and position, not semantic equality of copied
models; comparison and debug formatting do not traverse that main tier.
These stronger source comparisons also apply to timing-binding equality.
Locations can be inspected without claiming timing. The lexical slot carries the same
location through count matching, corroboration and complete timing rather than
asking a downstream consumer to reconstruct it. Ordinary and speaker splits
use these positions to travel their source-bound content groups, retaining
indivisible roots and the spoken originals of replacements. This main-tier
association does not establish a legacy `%wor` tier's immutable common origin.

```mermaid
flowchart LR
    Main[Typed MainTier] --> Projection[WorMainTierProjection]
    Projection --> Generated[Derived WorTier]
    Projection --> Binding{Timing binding}
    Parsed[Parsed legacy WorTier] --> Binding
    Binding --> Missing
    Binding --> Drifted
    Binding --> CountMatched[CountMatched, source roots retained]
    CountMatched --> Correspondence{Canonical token correspondence}
    Correspondence --> Uncorroborated
    Correspondence --> Corroborated[Corroborated, source roots retained]
    Corroborated --> Sequence{Sequence assessment}
    Sequence --> Empty
    Sequence --> Rejected
    Sequence --> Complete[Complete, source roots retained]
```

## Generation and parsing

For newly generated data, word timings are embedded on typed main-tier words.
`MainTier::generate_wor_tier()` derives the visible sidecar from those words.
It copies main-tier `cleaned_text` for display and copies the inline bullets for
timing.

For parsed legacy CHAT, the main tier and `%wor` are separate AST values. A
consumer must use the binding transition and then lexical corroboration before
recovering timing by position. Different display words produce an
`Uncorroborated` state. They do not replace main-tier lexical identity and do
not make the CHAT file invalid.

## Validation versus evidence admission

A drifted `%wor` tier does not make a legacy CHAT file invalid. Editors may
change the main tier without immediately rerunning forced alignment, and older
corpora used different membership conventions.

Evidence-consuming algorithms have a stricter contract. They must refuse
`Missing`, `Drifted`, or `Uncorroborated` when their operation requires word
timing. They must also decide whether `Unaligned` slots, empty intervals,
nonmonotonic intervals, or timings outside the main bullet are acceptable for
that specific operation. Structural and lexical admission do not prove
acoustic accuracy.

Temporal completeness does not prove acoustic accuracy either. It establishes
only coverage, positive duration, a min/max location hull, and explicit
adjacency geometry. The aligner may still place a perfectly well-formed onset
or offset too early or too late. Model score, boundary origin, human
calibration, and downstream merge outcome belong to a later evidence layer.

## Research boundary

The abandoned goal of timing every spoken main-tier item remains a legitimate
research question. It is not a correction to current semantics until the
membership question has been specified and evaluated. In particular,
fragments, nonwords, untranscribed material, interactional sounds, retraces,
and editorial replacements need explicit rules.

Confidence, acoustic quality, and provenance should remain typed internal
evidence attached to a binding or downstream decision. Chatter must not revive
public `%xalign` clutter as a side effect. A public `%wor` tier remains a
derived view unless TalkBank deliberately adopts a new visible format.

Alternative policies and acoustic qualification should be tested against
immutable evidence artifacts before changing corpus output. The relevant
questions include:

- whether the proposed policy reduces human correction time;
- whether every additional slot can receive defensible acoustic boundaries;
- whether changed timing improves the placement of manual transcripts merged
  onto automatic ones;
- whether confidence and provenance improve decisions without becoming public
  transcript clutter;
- whether a new policy can coexist with legacy `%wor` data without ambiguous
  automatic reinterpretation.

## Release boundary

Consumers must pin a compatible, reviewed Chatter revision and qualify their
own behavior against it. A release-pinned consumer needs a release containing
the required API; a separately authorized immutable development revision is
not a released artifact. A moving checkout is not a reproducible dependency.
Do not reproduce membership or correspondence in a parallel downstream pairing:
use `bind_wor_timing` followed by `corroborate_wor_timing` when those APIs are
available in the selected revision. Regression evidence must show both that a
same-count token edit is refused and that `%wor` text never becomes lexical
authority.

Downstream code that currently derives a main-tier bullet by manually checking
every `%wor` word and taking the minimum and maximum timing should then use the
sequence transition, `WorTimingHull`, and typed adjacency relations. This
removes the repeated loose procedure while preserving the important rule that
one untimed word cannot claim a complete child-utterance span. Compatibility
with a downstream policy must be measured before replacement because the new
API reports overlap and backwards starts instead of silently ignoring them.
