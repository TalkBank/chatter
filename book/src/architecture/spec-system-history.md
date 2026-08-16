# Why the Spec System Looks Like That

**Status:** Current
**Last modified:** 2026-08-16 12:39 EDT

[Spec System](spec-system.md) says what the spec files contain and what checks
them. This page answers the questions that page raises and does not settle, all
of which a new contributor hits in the first hour:

- Why are two thirds of the files named `_auto`?
- Why does `E202_auto.md` contain an example that expects **E316**?
- Why do eleven codes have two spec files?
- Why do so many descriptions say "Auto-generated from corpus"?

The short answer to all four is the same, and it is worth stating plainly
because it is a defect being worked off rather than a design:

> **A large part of `spec/errors/` was written by a machine that recorded what
> chatter DID, in files whose job is to say what chatter SHOULD do.**

Everything below is measured. Reproduce any of it with
`python3 scripts/analysis/spec_system_audit/audit.py` in the workspace repo, or
with the command named beside each number.

## The numbers, as of 2026-08-15

| Fact | Value |
|---|---|
| Error spec files | 238 |
| of those, named `*_auto.md` | **152 (64%)** |
| still carrying `Review and enhance this specification as needed` | **91** |
| whose Description is the literal `Auto-generated from corpus.` | 39 |
| still carrying `[Add link to relevant CHAT manual section]` | 68 |
| codes with NO example producing the code the spec is named for | **54** |
| codes claimed by more than one spec file | 11 |

## Where `_auto` came from

A bootstrap tool, `corpus_to_specs`, converted a directory of error-corpus
`.cha` fixtures into spec files. For each example it wrote an
`**Expected Error Codes**` line, which is a claim about what the validator DID
on that input.

Its only source for that claim was an `expectations.json` beside the corpus.
**That file does not exist and never has** (`fd expectations.json` finds none;
`git log --diff-filter=D` shows none was ever deleted). Three silent defaults
carried the gap to the page: a missing file became an empty map, an unparseable
file became an empty map, a per-file miss became an empty code list, and the
emitter turned an empty code list into *the spec's own filename code*.

So every such line the tool ever wrote asserts, as a measurement, the answer the
filename already implied. Since 2026-08-15 the tool refuses to run rather than
fabricate, but the 152 files it produced are still there.

## Why an E202 spec can expect E316

The format allows an example to declare codes other than the spec's own, and
`ERROR_SPEC_FORMAT.md` documents it as intended, for the case where "a spec's
input triggers a different error code than the spec itself documents."

That single sentence merges two facts of completely different kinds:

- **normative**: "this input is illegal under E202", a decision about CHAT that
  the implementation must satisfy;
- **observed**: "chatter reports E316 on this input today", a fact about our
  binary that changes when we change the binary.

When they are the same field, a spec that documents a GAP is indistinguishable
from a spec that documents a RULE, and no gate can count the difference. That
is how 54 codes reached a state where nothing demonstrates them.

**What this means for you as a reader:** an `Expected Error Codes` line tells
you what chatter emits, not necessarily what the rule is. Read the spec's
Description and title for the rule, and treat a mismatch between the title code
and the example's codes as an open question rather than as a specification.

**What it means for you as an author:** do not add new examples in that shape.
If an input violates the rule your spec is about, the example should produce
that rule's code; if it does not, chatter has a gap, and the gap is the finding.

## Why some codes have two spec files

Eleven do, and in every case the pair is one `_auto.md` plus one hand-written
file:

```
E202_auto.md  +  E202_missing_form_type.md
E241_auto.md  +  E241_illegal_untranscribed_marker.md
E519_auto.md  +  E519_l1_of_language_code.md  +  E519_word_level_language_code.md
```

Somebody hit a useless machine-written spec, wrote a real one beside it, and had
no way to retire the machine's version. Nothing declares which is authoritative;
nothing forbids a third. The hand-written ones are good, and
`E522_undefined_participant.md` is the model: a real description, a `Kind`, a
`Status`, and an example that declares its own code and emits it.

The one thing that IS enforced across a pair is `Kind`: it is a property of the
code, so the `DiagnosticKind` generator refuses to run when two files disagree
about it.

## What is genuinely reliable today

Not everything is suspect, and it matters to know which parts you can lean on:

- **The code sets agree exactly.** Every `ErrorCode` variant has a spec file and
  every spec-named code has a variant, in both directions, enforced by the
  `DiagnosticKind` generator refusing to emit on divergence.
- **Enforcement status has one answer.** The enum's `#[status(planned)]` and the
  spec's `**Status**` are reconciled by a gate that fails in either direction.
  It was built after `--list-checks` was found wrong about 15 of 225 codes.
- **Every generated artifact is checked.** `just spec-check` compares all four
  byte-for-byte against what the specs produce now.
- **Every example that declares codes emits them.** That is `error_spec_codes`,
  and it is a real check; it simply cannot see whether the codes declared are
  the ones the spec is about.

## Where this is going

The direction is recorded in the workspace repo's design note
(`docs/design/2026-08-15-spec-system-redesign.md`); the parts that change what
you write are:

- an example will carry a typed **claim** (`violates` / `legal` /
  `subsumed by E###`) instead of a free-text code list, so a normative decision
  and an observation stop being the same field, and so a spec that demonstrates
  nothing becomes unwritable;
- `legal` is new capability: today a spec cannot assert that a code must NOT
  fire, which is exactly the shape of every false-positive question;
- metadata moves to frontmatter with one parser and one validation command, so
  an agent writing a spec can check it without running the suite;
- the machine-written residue is worked off per code, as a count that may only
  shrink.

Until then, this page is the honest description of what you are looking at.
