# Code-switch span syntax: historical design alternatives

**Status:** Historical proposal; implemented semantics are owned by the specifications
**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/docs/proposals/csc-span-syntax.md)

## Current contract and scope

Code-switch annotations are implemented, not a pending new syntax proposal.
Use [Word Syntax](../../book/src/chat-format/word-syntax.md#language-markers),
[explicit spans](../../spec/constructs/main_tier/codeswitch_span_explicit.md),
[bare spans](../../spec/constructs/main_tier/codeswitch_span_shortcut.md) and
[word overrides](../../spec/constructs/main_tier/codeswitch_span_word_override.md)
for the maintained contract. The grammar's `code_switch_annotation` rule owns
the bracket syntax.

Important differences from the early draft:

- A single item may take a scoped annotation without angle brackets. There is
  no special minimum-two-words rule for code-switch annotations.
- A word's own language marker wins over an enclosing span. Do not reject it
  as redundant or ambiguous merely because a span marker exists.
- An explicit language code need not appear in `@Languages`, as with the
  corresponding word suffix. Bare-marker resolution has separate contextual
  rules; it is not a missing explicit-code error.
- Parse shape, contextual language resolution and downstream routing are
  distinct contracts. A parsed annotation does not itself qualify every
  downstream language-model operation.

The original draft's counts, corpus excerpts, implementation estimates and
migration commands are not current acceptance evidence. This record preserves
the design alternatives in general terms; it is not an operational runbook or
authority to change CHAT rules.

## Motivation retained from the draft

Per-word suffixes can be verbose for a stretch in another language, while an
utterance-wide precode changes the language context of the whole utterance.
A scoped annotation expresses a local language context without splitting a
speaker turn. The motivation does not establish any claimed prevalence,
transcriber-error rate or universal reduction in annotation effort.

The intended design goals were local scope, reuse of CHAT's annotation syntax,
explicit language when necessary, and composability with existing content.
Compatibility and nesting behavior must be specified and tested rather than
inferred from the small visual change.

## Alternatives considered

| Candidate | Historical reasoning |
| --- | --- |
| `<words> [@s]` / `<words> [@s:code]` | Reuses the scoped-annotation family; selected syntax, with later semantic refinements. |
| `@s< words @s>` | Introduces another delimiter class and word-boundary ambiguities. |
| `first..last@s` | Editing and existing punctuation make the range unclear. |
| XML-like language tags | Adds a different notation family within CHAT. |
| `<words> [- code]` | Conflates utterance-language precodes with local scoped annotations. |

These alternatives explain the original choice; they are not additional
accepted spellings.

## Proposed restrictions that must not be revived

The early draft proposed a minimum span length, rejection of word-level
overrides, declared-language membership for explicit codes, and a fixed nesting
limit. It also listed inconsistent error/warning treatments for a single-item
span. Those proposals are not a validation policy. The maintained specs above
explicitly resolve the first three boundaries; other nesting and annotation
interactions follow their own grammar and specifications.

Likewise, an earlier sketch proposed a separate `CodeSwitchSpan` content
variant and assumed all walkers would gain language semantics automatically.
That sketch is not the current Rust API. Consumers must use the model's actual
annotation and language-resolution owners, preserving which marker supplied
each resolved language. Do not rebuild a parallel resolver from this document.

## Compatibility and migration limits

Existing per-word markers remain useful. A source rewrite that condenses them
into a group needs independently reviewed preservation requirements: annotation
scope and order, nested language overrides, retraces, source coordinates,
serialization and the distinction between resolved and unresolved language.

No conversion command is supplied by this proposal. It does not authorize
rewriting any corpus, inferring intent from partly tagged text, or dropping
markers because a target tool ignores unknown annotations. Acceptance by an
older parser is not evidence that it preserves the intended language semantics.

## Review boundaries

A future syntax or semantic change needs an explicit ruling, source-owned
specifications with legal and invalid controls, both parser implementations,
model/serialization review and affected consumer evidence. Publication of a
proposal alone is not approval. Existing parser comparison, language-resolution
and reference tests are separate evidence; this historical summary reports no
new test run or release qualification.
