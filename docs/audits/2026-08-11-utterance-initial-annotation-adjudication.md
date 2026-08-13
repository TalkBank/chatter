# Adjudication: an annotation that opens an utterance

**Status:** Current
**Last modified:** 2026-08-12 18:40 EDT

An adjudication of how the two parsers read `[: ...]` at the start of an
utterance, grounded against real CLAN CHECK. It found one defect in each parser.

BOTH ARE FIXED. Both change what chatter reports on a class of inputs, so the
corpus differential applies before any push.

## How this surfaced

`E311_auto.md` was marked `Status: not_implemented` on the strength of a note
saying E311 was unreachable because tree-sitter recovery wrapped the malformed
utterance in an ERROR node and E316 fired first. The parser had since outgrown
that. When the spec was corrected to `implemented`, the re2c parity harness
reported a divergence it had never recorded, because the case had been SKIPPED
rather than agreed.

## The evidence

Four inputs, each a single main tier, run through both backends and through
real CLAN CHECK (`clan-run.sh`, 2026-08-07 bundle).

| input | CLAN CHECK | tree-sitter | re2c |
|---|---|---|---|
| `[: unclosed replacement [* error] .` | **Unmatched `[` (22)**, plus 48, 11 | E311 + E305 | E759 |
| `word [: unclosed .` | illegal chars (48) | E311 | E321 |
| `[: closed] .` | **only** "Item must be preceded by text" (52) | E759 + **E305 + E342** | E759 |
| `word [: a [* b] .` | 48, 11 | E375 x2 | **silent** |

## Finding 1: re2c does not see an unmatched bracket

On row 1, re2c reads the INNER `]` as closing the outer `[:`, so it never
notices the outer bracket is unclosed and reports E759 ("annotation must be
preceded by text") instead. CHECK's "Unmatched `[`" settles it: the outer
bracket really is never closed, so tree-sitter's E311 is the right structural
diagnosis, and its E305 follows from it, because the terminator is swallowed
INSIDE the unclosed bracket and the utterance genuinely has none.

Row 4 isolates the gap without the confounder: re2c is SILENT on input CHECK
rejects.

**This reverses the first reading.** Recorded before the CHECK run, from the
codes alone, was the opposite conclusion: that the oracle looked right and the
canonical parser looked wrong. Grounding it took one command. The oracle is
authoritative about DIVERGENCE, never about which side is correct.

## Finding 2, and its fix: a terminator reported missing when one is present

Row 3 is well-formed apart from its one real fault. `[: closed] .` is a
correctly closed replacement at utterance start with nothing before it. CHECK
reports exactly one thing, rule 52, which is chatter's E759. chatter reports
E759 and two more:

As found, chatter reported E759 plus:

- **E342** "Missing required 'ca_no_break'": the documented recovery-node
  diagnostic, by design, since recovery is not validity. Noisy for a user
  (`ca_no_break` is a grammar internal) but not wrong.
- **E305** "Missing terminator in main tier": WRONG. The parse tree contains
  `ending: (utterance_end (period) (newline))`, so the terminator was parsed;
  the lowering drops it when an ERROR node precedes `tier_body`, and the model
  then correctly applies its own rule (`terminator: None` implies E305) to a
  value that should not be `None`.

Severity is bounded and worth stating: the input is already invalid, so this was
noise on rejected input rather than a false positive on valid CHAT. It put no
number at risk; it made a diagnosis less trustworthy, which is how a tool
teaches people to stop reading it.

### The fix, 2026-08-11

Two causes, one loss. The lowering's `tier_body` slot held the ERROR node, and
the real `tier_body`, holding the terminator, sat in the traversal's
`unexpected` sink, which spec Section 7 guarantees never drops a child. The arm
threw it away and then reported the absence as a fact about the user's file. It
now parses the displaced node from the sink.

That exposed a second layer: with the body parsed, the utterance's content is a
single zero-width separator, so the "empty content" checks fired instead
(E306, E253). A parse-taint exemption already existed for a genuinely EMPTY
content list, with the reasoning written out ("recovery empties the content
list even when the source line plainly has content"), and was missing on its
only-separators sibling. One fact handled in one branch and not the other.

`*CHI:\t[: closed] .` reported four codes and now reports two: E759, which is
what CHECK reports, and E342, the recovery-node diagnostic.

Corroboration that the fix is right rather than merely quieter: three E759
cases in `KNOWN_DIVERGENCES` now AGREE between the backends and were retired,
and four CHECK-parity manifest rows that had pinned the spurious E305 on lines
ending in ` .` now record what chatter actually says about the real fault.

**E342 remains, and its message is still poor.** "Missing required
'ca_no_break'" names a grammar internal, which is accurate and useless. It is
the deliberate recovery-not-validity signal, so improving its wording is a
separate change to a documented mechanism, not a bug fix.

## Finding 1's fix, 2026-08-11

The lexer's replacement rule took any content up to the first `]`, so
`[: unclosed replacement [* error]` lexed as a COMPLETE replacement whose text
merely contained `[* error`, and the unmatched `[` became invisible. grammar.js
builds a replacement from `standalone_word`s and `[` cannot appear in a word,
so the rule now excludes `[` as well as `]`.

re2c reports E321 ("utterance could not be parsed") for both row 1 and row 4,
where it previously gave a wrong reason and silence respectively. E321 is
vaguer than the canonical parser's E311, and that is the acceptable direction
for an oracle: its job is to disagree when the canonical parser is wrong, not
to phrase the diagnosis. Validated over the wild corpus by the ignored
`corpus_lex` test (86 s, clean).

## Where this leaves the two inputs

| input | before | after |
|---|---|---|
| `[: unclosed replacement [* error] .` | ts `E311, E305`; re2c `E759` | ts **`E311`**; re2c **`E321`** |
| `[: closed] .` | ts `E759, E305, E342, E306, E253` | ts **`E759, E342`** |
| `word [: a [* b] .` | re2c **silent** | re2c **`E321`** |

`E311_auto.md` remains in `KNOWN_DIVERGENCES` because the codes still differ.
That is now a difference of precision, not of correctness.

## The root cause is upstream, in the traversal generator

chatter's fix reaches into the `unexpected` sink, which works, but the trap is
in `tree-sitter-grammar-utils`, which EMITS the matcher into every consumer
(`crates/tree-sitter-node-types/src/backend/rust.rs:3148`):

```rust
let cursor = skip_extras(children, start);
if let Some(child) = children.get(cursor) {
    if child.is_error() {
        return Some(cursor.saturating_add(1));
    }
}
```

An ERROR child is consumed into WHATEVER position the cursor is at, without
checking whether a later child matches that position's kind. So one stray ERROR
shifts every following position, a required typed slot reports `Error` while
the node that genuinely matches it sits intact in the sink, and a consumer
reading `NodeSlot::Error` as "this position failed" is wrong.

Nothing is dropped, so the sink guarantee holds. What does not hold is the
reading a consumer naturally makes, and chatter made it, and shipped a false
error message to users because of it.

Reported to the tsgu session on 2026-08-11, and FIXED there the same night
(tsgu `8d708f5`, local main, unpushed): at a fixed-arity position an ERROR run
is absorbed only when nothing past it can fill that position, otherwise the run
goes to the sink and each position gets the child that belongs to it. Repeat
elements still absorb unconditionally, because variable arity displaces nothing.

**Third-party evidence, which is stronger than either reproduction.** In tsgu's
`go` conformance grammar, `RECOVERY-SCOPED (field)` went 2 to 0 while
`RECOVERY-SCOPED (ordinal)` stayed at 1. The unchanged ordinal count is the
load-bearing half: the node is still recovery-scoped, so the suppression
machinery is still running and still counting, which closes off "the count
simply stopped being taken". What vanished is the disagreement, not the
measurement, and the disagreement was with tree-sitter's own
`child_by_field_name`. Cite tsgu `893f801`,
`crates/cli/tests/suites/error_displacement/README.md`, section "Third-party
evidence that the old rule was wrong".

### The chatter-side lesson, which is the more general one

The generator bug was one bug. The consumer bug was a shape: a lowering keyed
on WHICH slot state it sees will keep finding new ways to lie as recovery
behaviour shifts underneath it.

`Error`, `Unexpected` and `Absent` were three spellings of one thing, "I did
not find the terminator where I looked", and chatter read all three as "the
user's file has no terminator". **Three enum variants were being used as a
boolean, and the fact that actually mattered, is the terminator elsewhere in
this node, was recoverable from none of them without consulting the sink.**

So the check is now state-independent: every non-`Present` state asks the sink
whether the content is there, and only reports a missing terminator when it is
genuinely absent. That question has an answer that is a fact about the user's
file; "which slot state did the traversal produce" does not.

**Honest scope of that change, and it is a shared open question rather than a
gap in this repository's coverage.**

Only the `Error` path is reachable by any input I could construct: six
malformed main tiers, including the tsgu session's own minimal shape adapted to
this grammar, all report E316 and never reach the `Absent` or `Unexpected`
arms. Asked for a fixture that would reach them, the tsgu session went looking
and could not produce one either. Their census over nine malformed inputs
against exactly this shape, under the FIXED generator, finds the required
`tier_body` position **Present in all nine**, with the recovery material in the
sink; kept as an ignored instrument,
`probe_slot_state_census_for_main_tier_shape` in
`crates/tree-sitter-node-types/tests/reconstruction.rs`. They also report that
no tsgu test positively asserts `Absent` anywhere: every mention is a NEGATIVE
assertion, several pinning that a tail is Present rather than Absent.

Their proposed mechanism, flagged by them as ANALYSIS rather than measurement,
and repeated here with that label intact: tree-sitter forms a node only once
every required member has a real or MISSING child, so a required position with
nothing to put in it usually means there is no node to extract from either.
Displacement used to manufacture `Absent` by misaligning the cursor with the
parse, and that is exactly what `8d708f5` removed.

Neither of us claims unreachability. Nine inputs on one simplified grammar say
nothing about supertypes, aliases, or a `Choice` whose alternatives all decline.
The honest state is: reachable in principle by construction of the emitted code,
never observed, and the mechanism that used to reach it is gone. Recorded
upstream as an open question in tsgu `docs/type-oriented-architecture.md`
(`5bec535`), because settling it needs a grammar built deliberately to force a
required position to decline while its node still forms, not another sweep of
plausible inputs.

**Why that distinction matters here.** These two branches may be UNTESTABLE
rather than merely untested, and those are different problems with different
fixes. A guard that never runs reports clean, so an untested branch wants a
test; but if a state cannot occur at a required position, no test can be
written and the answer is to stop representing it there. That is a question
about the generated type, not about this lowering, which is why it belongs
upstream.

What this lowering can do, and now does, is stop distinguishing them: `Error`,
`Unexpected` and `Absent` reach one helper asking one question. So chatter no
longer has three branches to keep honest, whatever the answer turns out to be.

## SCOPE WARNING: this adjudication was made against the 0.1.0 traversal

Everything above was measured with `tree-sitter-node-types` 0.1.0. That
generator ABSORBED an ERROR into whatever position its cursor was at, which is
the defect this document reported upstream and which tsgu fixed (`8d708f5`,
generator 0.2.0). A regeneration was taken to a green build on 2026-08-12 and
deliberately NOT landed; the migration is saved as a patch, and chatter remains
on 0.1.0 at the time of writing.

**A census on one input shows why that matters here.** For
`*CHI:\thello [: world .`, inside the `tier_body` carrier:

| generator | `content_2` | `ending` | `unexpected` | chatter reports |
|---|---|---|---|---|
| 0.1.0 | Present | **Error** | `["utterance_end"]` | E311 "Unclosed replacement bracket" |
| 0.2.0 | Present | Present | `["ERROR"]` | E316 "Unparsable content" |

Under 0.1.0 the ERROR occupied the TERMINATOR's slot and the real
`utterance_end` was displaced into the sink. So that E311 was
`analyze_word_error` reporting on an ERROR sitting in a position whose real
child had been displaced: a specific, correct-LOOKING diagnostic, naming the
right construct at the right span, produced from a corrupted model.

**This is the same defect as Finding 2, from the other side.** Absorption puts
the ERROR in a position and the real child in the sink. A consumer then either
reports the ABSENCE of the displaced child (Finding 2: "missing terminator" on
a line ending in one) or reports on the ERROR AS IF it were that position's
content (E311 here). Both are wrong about the file. Only one looked wrong.

The general trap, which the tsgu session wrote up as
`docs/type-oriented-refactoring-method.md` §5.16, is worth stating here too:
**corruption that yields a plausible output outlives corruption that yields an
absurd one.** The absurd reading was found and fixed in a day. The plausible
one survived, was valued, acquired a regression test, and was defended as a
loss when the corruption was finally fixed. So a downstream change that LOOKS
like a regression earns the same adjudication as one that looks like a fix, and
fixing a defect at its source means going to find the OTHER readers of the same
wrong model.

**What this means for the verdicts above.** The substance stands: real CLAN
CHECK reports an unmatched `[`, so a diagnostic naming that construct is right,
and re2c's silence and wrong-reason were genuine defects independent of any
traversal. What does NOT carry over is the PROVENANCE of chatter's specific
codes. Under 0.2.0 they must be re-earned from ERROR nodes in the sink, which
carry their own byte spans, rather than arriving as a typed slot. Recovering
them there is the correct fix and not a workaround, precisely because the slot
they used to arrive in was the defect.

## The method note worth keeping

Every wrong step here came from reasoning about error CODES instead of running
the input. The codes are suggestive and the behaviour is decisive; CHECK is a
question list, and one `clan-run.sh` invocation answered a question two rounds
of inference had got backwards.
