# Postcodes (`[+ ...]`)

**Status:** Reference
**Last updated:** 2026-05-11 23:01 EDT

A **postcode** is a tagged annotation token that attaches to an
*utterance as a whole* and appears after the terminator. The
canonical CHAT syntax is `[+ <text>]`. Postcodes carry researcher /
analysis tags about the utterance, whether it should be excluded
from analysis, how it should be coded, what kind of speech act it
represents, without modifying the utterance's word content.

## Syntax and Scope

```text
*CHI:   I want cookie .  [+ exc]
*MOT:   what did you say ?  [+ imp]
*CHI:   no I don't want it !  [+ neg] [+ trn]
```

Three structural facts to internalize:

1. **Postcodes attach to the utterance, not to a word.** They sit
   after the terminator, on the main tier, alongside (but distinct
   from) any utterance-level bullet. Unlike word-scoped annotations
   (`[: ...]` replacement, `[% ...]` comment, `[= ...]` explanation,
   `[* ...]` error code), a postcode does not modify the
   interpretation of any single word, it tags the whole utterance.
2. **Multiple postcodes may follow a single terminator.** They are
   ordered, but the order is not semantically privileged.
3. **The body is free-form text.** The CHAT word grammar is *not*
   applied to postcode contents. Researchers can write arbitrary
   tags, codes, descriptions, comments, or analytic notes. The model
   stores the raw text and leaves interpretation to downstream
   tooling and conventions.

## Common Postcodes, Empirical Survey

The postcode vocabulary is **open-ended**: the CHAT format imposes no
closed set, and an audit of every `[+ ...]` token across a
JSON-mirrored snapshot of the TalkBank corpora (~99k files, 23+
data-repo families) found **488 distinct values** in active use.

The findings split into three tiers ranked by *repo spread* (in how
many distinct corpus families the code appears), the more useful
ranking than raw count, because high-count codes can be concentrated
in a single corpus.

### Tier 1, Cross-corpus codes (in 7+ repos)

These are the conventions every CHAT consumer should expect to
encounter across collections:

| Postcode | Repo spread | Total occurrences | Meaning |
|---|---|---|---|
| `[+ gram]` | 13 | ~3,100 | Grammatical, utterance is grammatically well-formed for purposes of the analysis. |
| `[+ exc]` | 9 | ~26,900 | Exclude utterance from analysis. The utterance is preserved in the transcript but tagged so analytic tools (CLAN's `freq`, `mlu`, etc.) skip it. |
| `[+ bch]` | 9 | ~10,000 | Backchannel, listener-side acknowledgement (`mhm`, `yeah`) that should not be counted as a substantive turn. |
| `[+ trn]` | 7 | ~3,800 | Translation utterance. |

### Tier 2, Multi-corpus protocol codes (in 4-6 repos)

Codes deployed across several CHILDES sub-collections, typically
encoding picture-narration / story-reading / imitation experimental
conditions. Substantial raw counts (often tens of thousands), but
their meaning is set by the originating protocol, consult per-corpus
documentation rather than assuming a global definition:

| Postcode | Repo spread | Total occurrences |
|---|---|---|
| `[+ SR]` | 5 | ~31,000 |
| `[+ IN]` | 5 | ~24,500 |
| `[+ PI]` | 5 | ~22,700 |
| `[+ R]` | 4 | ~16,200 |
| `[+ I]` | 4 | ~10,500 |
| `[+ nv]` | 4 | ~3,300 |
| `[+ imit]` | 4 | ~3,200 |

### Tier 3, Single-corpus and long-tail codes

About 80% of the 488 distinct values appear in one repo only. The
single-corpus codes include high-volume protocol vocabularies (e.g.
`[+ uncued]` ~19,500 in one repo, `[+ NAC]` ~3,500 in one repo,
`[+ diary]` ~2,800 in a Romance/Germanic diary-study collection,
`[+ noatt]` ~2,300 in one repo, `[+ inter-utter-switch]` ~720
flagging code-switching turns).

The long tail also includes researcher-private notes, typos that
survived `check`, and per-study coding schemes. Tooling MUST treat
any unknown postcode value as opaque text, the corpus author may
know what it means, the format does not.

### Caveats

- Numbers are from a snapshot audit and will drift as corpora are
  added or revised. Treat the broad shape (open vocabulary, ~4 truly
  cross-corpus codes, ~10 multi-corpus protocol codes, ~hundreds of
  single-corpus or long-tail codes) as the load-bearing finding, not
  the exact counts.
- "Repo spread" counts data-repo families, not individual files.
  Two corpora curated by the same group inside one data-repo count
  as one for spread; researchers using the same code in two
  different family-of-corpora packages count as two.
- The CHAT manual remains the source of truth for *standard*
  conventions. The empirical survey above shows what is *actually*
  deployed; when ingesting a new corpus, consult its own
  documentation for the postcodes in use.

## What Postcodes Are NOT

Postcodes are easy to confuse with several other CHAT annotation
forms because they all use square brackets. The differences are
substantive and load-bearing.

| Form | Scope | Body validation | Purpose |
|---|---|---|---|
| `[+ ...]` | **Utterance-level** (this doc) | None, free text | Researcher / analysis tag attached to the whole utterance |
| `[: ...]` | **Word-level** | Replacement words ARE validated as CHAT words | Sanctioned-form correction of the preceding word (see `replacements.md`) |
| `[% ...]` | **Word-level** | None, free text | Free-form comment about the preceding word or local span |
| `[= ...]` | **Word-level** | None, free text | Explanation of unclear / non-standard speech (often paired with `xxx` / `yyy` placeholders) |
| `[* ...]` | **Word-level** | None, error code text | Error coding for the preceding word, optionally with a structured code |

Two consequences worth pinning down explicitly:

- **A postcode cannot carry per-word semantics.** If you want to
  attach a comment, replacement, or error code to a single word, use
  the appropriate word-scoped form. Stretching a postcode to mean
  "this word is X" loses the per-word position downstream tools
  depend on.
- **A word-scoped annotation cannot tag an utterance.** If you want
  to mark an entire utterance for exclusion or translation, use a
  postcode. A `[% exclude this]` after a word does not mean "exclude
  the utterance" to any consumer.

## Not Postcodes: Quotation Markers

Quotation marking in CHAT is **not a postcode form**. The constructs
`+"/.` (quotation end), `+"/`, and `+"` (quotation linkers /
continuations) are tier-level **terminators and linkers**, not
`[+ ...]` postcodes, the grammar rule `postcode` in
`grammar/grammar.js` is strictly `[+ <text>]`, and the quotation
forms live under separate grammar rules (`quoted_new_line`,
`linker_quotation_follows`).

See [Utterances → Terminators](utterances.md#terminators) for the
syntactic forms, and the
`talkbank-model::validation::cross_utterance` validator family
(gated by `ValidationContext::enable_quotation_validation`) for the
cross-utterance balance checks.

A walker in `talkbank-model::validation::utterance::quotation`
(`check_quotation_balance`) does scan the postcode list for text
`"/` and `"/.`, but a sweep over the data-json corpus mirror
(101,414 files, 2026-05-11) returned zero such postcodes, that
code path is effectively dead, retained presumably as defence
against hand-edited oddities. The real quotation-balance work
happens in the cross-utterance family above.

## Position in the AST

An utterance's main tier is `MainTier`, whose `content: TierContent`
field carries the actual tier payload, including postcodes, as a
typed list:

```rust,ignore
pub struct MainTier {
    pub speaker: SpeakerCode,
    pub content: TierContent,
    // spans omitted for brevity
}

pub struct TierContent {
    pub linkers: TierLinkers,                  // utterance-leading +<, ++, etc.
    pub language_code: Option<LanguageCode>,   // [- code]
    pub content: TierContentItems,             // word-level items (newtype over Vec<UtteranceContent>)
    pub terminator: Option<Terminator>,        // ., ?, !, +..., etc.
    pub postcodes: TierPostcodes,              // [+ ...] tokens after the terminator
    pub bullet: Option<Bullet>,                // optional terminal media bullet
    // content_span omitted for brevity
}
```

(See `talkbank-model/src/model/content/main_tier.rs` and
`tier_content.rs` for the exact shape; the same `TierContent` type is
shared by dependent tiers, so the postcode slot exists on every tier
even though only main-tier postcodes are conventional.)

Because postcodes live at the utterance level, the per-word
traversal helpers (`walk_words`, `walk_words_mut`) do not visit
them. Code that needs to read or rewrite postcodes accesses the
list directly.

The model stores postcode text as `SmolStr` and preserves it
verbatim through CHAT roundtrips. Downstream tooling, including
CLAN command implementations such as `freq`, `mlu`, `kideval`, is
responsible for interpreting individual postcode values per its own
conventions.

## Tooling Rules

Tools that emit or consume CHAT must respect the scope distinction.

- **Emitters:** when adding a researcher tag to an utterance, attach
  a `Postcode` to the utterance's `MainTierContent`, not a
  `ContentAnnotation` to a word. Both serialize, but only the former
  reaches downstream consumers as utterance-level metadata.
- **Consumers:** when reading utterance-level tags (e.g.,
  implementing an "exclude" filter), iterate `main.content.postcodes`
  on each utterance, not the word-level annotations in
  `UtteranceContent`. The two lists are populated by different
  parser branches and have different semantics.
- **Round-trip preservers** (extract→modify→inject pipelines such as
  the NLP injection passes in `crates/batchalign-*`): preserve the
  postcode list unchanged. None of the standard NLP passes have a
  reason to add, remove, or reorder postcodes.

## References

- CHAT manual: [Postcodes](https://talkbank.org/0info/manuals/CHAT.html#Postcodes)
- CHAT manual: [Excluded Utterance Postcode](https://talkbank.org/0info/manuals/CHAT.html#ExcludedUtterancePostcode)
- CHAT manual: [Included Utterance Postcode](https://talkbank.org/0info/manuals/CHAT.html#IncludedUtterancePostcode)
- Model: `talkbank-model/src/model/content/postcode.rs`
- Quotation validator: `talkbank-model/src/validation/utterance/quotation.rs`
