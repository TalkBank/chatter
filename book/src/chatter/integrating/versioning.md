# What a Version Bump Promises

**Status:** Current
**Last modified:** {{git-dates:page}}

If you depend on chatter, distinguish at least two kinds of compatibility:

- the **Rust API**, which decides whether your code still compiles;
- the **validation verdict**, which decides whether files you already have
  still pass.

A release can be source-compatible and still change which CHAT files
`validate` accepts. An intentional rule correction and an unintended regression
can both change a verdict; compilation alone cannot distinguish them. This page
records the pre-1.0 policy, not a guarantee that every behavioral difference is
correct. The [1.0 readiness contract](../../contributing/one-zero-readiness.md)
still requires the stable surfaces and their compatibility promises to be defined.

## The Rust API follows SemVer

Chatter is pre-1.0, so a minor bump may break the Rust API. Pin a release tag or
commit for Git dependencies and review the changelog before advancing it. For
registry dependencies, pin the exact published package version when repeatable
resolution is required; first verify that the package/version actually exists.
See [Crates.io Publication](../../contributing/crates-io-publication.md) for the
publication boundary. A version constraint is not evidence of registry availability.

## The validation verdict follows a different rule

**Any release may change which files validate, including a patch release.**
Validation verdicts are not frozen before 1.0: parser and validation rules are
still being corrected against the CHAT specification and reviewed policy.
That permits corrections; it does not make the tool's verdict its own authority.

The release-note requirement is:

> Every release whose validation verdicts move opens its `CHANGELOG.md` entry
> with a bold **Validation behaviour** note, naming the codes that were added,
> removed, retired or made stricter, and what kind of file is affected.

Read that note before upgrading a pipeline that gates on `validate`. Its absence
is not an exhaustive equivalence proof: a regression or an omitted release note
can still exist. The requirement to document verdict changes remains; an
unannounced observed difference needs investigation, not automatic data repair.

## Which direction a change can go

Both, and they are not symmetric:

- **Stricter** (a new code, or an existing one reaching more inputs) means
  files that used to pass now fail. Newly rejected files need adjudication
  against the CHAT specification and
  the chosen validation policy. A parser or validator defect must be fixed in
  the tool, not worked around by changing correct transcript data.
- **Looser** (a code retired, or a false positive fixed) means files that used
  to fail now pass. Retired codes are never reused for a different rule.

Under the pre-1.0 policy above, either direction may occur in a patch release
without changing Rust signatures. That does not make the change harmless to an
application: validation verdicts, diagnostics and serialized output can be
behavioral dependencies. Do not infer a post-1.0 compatibility promise from this
pre-1.0 rule.

### Retiring a code is not a clean removal

A code names a rule at a moment in time, and downstream records cite it: an
adjudication log, a repair ledger, a review note all say "this edit was made
because E754 fired", and that remains true after the rule is withdrawn. So a
retired code keeps two obligations. It is never reused, as above. And a
consumer holding historical citations should NOT validate them against the
current code set, because doing so makes a correct old record un-loadable over
a rule that was withdrawn for reasons that have nothing to do with that record.

If you keep such citations, validate them as a closed list that includes the
retirements you know about, so a typo still fails and a NEW retirement fails
loudly until someone records why. That is the check worth having; checking
against today's live set is not.

Example: a ledger that cites E754 (`LetterFormMultipleLetters`, a retired
code) for a repair that is still correct: a digit zero typed for the letter `o` in `0@l`. The rule went away
because it counted characters and a digraph is one letter written with two;
the repair it surfaced was right either way.

## What to do about it

If you gate on `validate` in CI over a fixed corpus, treat a chatter upgrade
the way you would treat a linter upgrade: pin it, upgrade deliberately, and
review the verdicts over your own files rather than assuming. Chatter's
acceptance evidence comes from reviewed specifications and a finite reference
corpus exercised through public workflows, including deliberate invalid
variants. A full production-corpus differential run is not a release gate.
New findings require CHAT-policy adjudication; matching another validator's
diagnostic code, count or wording is not the acceptance criterion. See the
[CHECK assessment](../../architecture/errors-and-validation/check-parity-audit.md)
for its scope and reopening rules.

If you consume parsed models without calling `validate`, still review parser,
recovery and serialization changes that affect your application. A compiling
adapter can receive different field values or diagnostics. The
[diagnostic contract](diagnostic-contract.md) and
[JSON output guide](json-output.md) describe those interfaces; they are not
substituted by a source-compatibility check.

## Why this page exists

An integrator pinning chatter found that the two halves were not distinguished
anywhere, and hit the case that makes the distinction concrete: a release that
was perfectly source-compatible (their adapter compiled unchanged, their whole
suite passed) while validation moved in both directions at once, one file
newly rejected in a `bad` corpus and one newly rejected in a `good` one. The
practice of announcing verdict changes already existed by then and had been
followed for several releases; it was simply not written down anywhere a
consumer would look.
