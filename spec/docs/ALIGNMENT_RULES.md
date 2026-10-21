# Dependent Tier Alignment Reference

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/spec/docs/ALIGNMENT_RULES.md)

This spec-authoring reference routes alignment cases to their maintained
model and book owners. It does not define a second membership table or error
registry.

## Choose the relationship before writing a case

| Relationship | Owner and distinction |
|---|---|
| Main tier → `%mor` | Morphological positions: replacements and retraces differ from spoken-word domains |
| `%mor` → `%gra` | Mor chunks, including clitics and a present terminator, not just Mor items |
| Main tier → `%pho` / `%mod` | Phonological positions, including pauses and atomic phonological groups |
| Main tier → `%sin` | Sign-domain positions and grouping |
| Phon tier → tier | Derived syllabification and alignment views; one-sided pauses advance source tiers independently |
| Main tier with `%wor` | Separate timing-sidecar projection, not structural count validity |

The [Alignment chapter](../../book/src/architecture/alignment.md) owns the
structural overview, domain table, provenance gates and downstream consumers.
[Dependent tiers](../../book/src/chat-format/dependent-tiers.md) describes the
format; [Phon tiers](../../book/src/chat-format/phon-tiers.md) describes their
reconstruction and source-position contracts.

## Domain and index ownership

`TierDomain` names Mor, Pho, Sin and Wor membership for shared walks.
`PositionalDomain` contains only Mor, Pho and Sin: structural counting and
extraction do not own an independent Wor implementation.
`WorMainTierProjection` owns the versioned Wor selection.

Counts alone are insufficient to choose source indices. Mor items, Mor chunks,
main-tier domain positions and Phon source-tier positions are distinct spaces.
Use the model's derived pairs and typed indices; never assume the same raw
index addresses both source tiers after a one-sided pause.

## Parse health and recomputation

`Utterance::compute_alignments(&ValidationContext)` recomputes derived metadata
without consuming the preserved dependent tiers. It consults provenance for
each relationship. Recovery on one tier prevents trusting that tier's pair;
unknown provenance, including ordinary JSON import, cannot authorize alignment.

Explicit checked construction is a separate admission path, not parser
provenance or source-span authority. Applying recovery taint only withdraws
trust. Missing or warning-bearing alignment metadata must not be interpreted
as successful correspondence. A skipped check can report E600 as a warning;
that is not a count-mismatch verdict or evidence that the input is valid.

## Wor timing is a separate contract

[Wor timing semantics](../../book/src/architecture/wor-timing.md) owns membership
and the stronger evidence transitions:

- binding: `Missing | Drifted | CountMatched`;
- token corroboration: `Uncorroborated | Corroborated`;
- sequence assessment: `Empty | Rejected | Complete`.

Count or token drift is not a structural CHAT alignment error. Equal counts
alone expose no positional timing; corroboration is required. A complete
sequence exposes a timing hull but does not certify acoustic accuracy.
Physical word bullets remain timing evidence for media-linkage checks even
when optional alignment processing is not requested.

## Authoring and reviewing alignment cases

1. Select the actual relationship, membership policy and parse-health state.
2. Include legal controls and isolate the intended mismatch or recovery
   interaction. For Mor/Gra, distinguish item counts from chunk indices.
3. For Phon pauses, inspect each source position independently. For Wor,
   distinguish count drift, same-count token drift and missing timing.
4. Review the intended claim against the rule before inspecting or updating
   generated observations. Unknown provenance and skipped checks are separate
   cases from valid aligned inputs.
5. Follow [Spec Workflow](../../book/src/contributing/spec-workflow.md) for
   generation and affected behavioral checks.

Code identifiers, descriptions and implementation status belong to
`spec/codes/error-codes.toml`; examples and claims belong to `spec/errors/`.
Use the generated error reference instead of a second hard-coded code table.
A finite passing matrix is not complete alignment or recovery coverage.
