# Migrating to Chatter 0.33

**Last modified:** {{git-dates:page}}

This guide covers changes from 0.32.1 to 0.33.0. Check the
[release page](https://github.com/TalkBank/chatter/releases) for published
artifacts. If upgrading from an earlier version, apply the
[0.32 migration guide](migration-0.32.md) first. Pin one exact tag or commit
across the CHAT library crates and rebuild dependent applications.

## Shared model and JSON

`Word::suffix: Option<WordSuffix>` replaces independently mutable `form_type`
and `lang` fields. Choose `WordSuffix::Form(form)` or
`WordSuffix::Language(marker)`, and replace the slot with `with_suffix`.
The `with_form_type`, `with_lang` and `with_language_shortcut` builders are
removed. Read through `form_type()` and `language_marker()`; edit an existing
language payload through `language_marker_mut()`.

Use `WordSuffix::from_markers` when admitting an external field pair. It refuses
a conflict without giving either field precedence. JSON retains the `form_type`
and `lang` property names, but two non-null values now reject. Null still means
absence. Native word carriers use the same suffix owner; their existing
`form_marker()` and `lang()` inspection methods remain available.

`FormType::UserDefined` carries `FormLabel`, replacing `String`. Admit text with
`FormLabel::new(label)?`: labels must be nonempty ASCII letters, digits or
underscores. Accepted `{"z":"label"}` JSON and CHAT spelling are unchanged.
`FormType::Undeclared` retains rejected text for diagnostics. Undeclared and
repeated form-marker recovery remains E203-invalid after JSON readback.
Rendering or deserialization alone does not establish validity.

The shared model now owns E203 for these recovery values, avoiding duplicate
parser/model reports. A repeated run names the one-suffix restriction. Full-file
parsing retains recovery without E203 in its parser sink; validate the model
before accepting it. Standalone-word admission invokes the same model rule and
retains its existing suffix-refusal contract. That finding points at the whole
word, as model validation does, rather than a separately retained suffix token.

`Word::part_of_speech` is now `Option<WordPosTag>`, and
`with_part_of_speech` accepts `WordPosTag`. Use `WordPosTag::new` to admit a
nonempty hint containing only ASCII letters and colons. JSON still uses strings;
case, colons and the open hint vocabulary are preserved. Do not trim rejected
text, discard characters or invent an empty hint. Native word carriers retain
the same admitted tag.

Freecode/postcode payloads distinguish classified text from unparsed source.
Their JSON and CHAT spelling remain compatible, but exhaustive matches,
structured semantic-diff paths and debug-output consumers must account for the
new internal grouping.

## Diagnostics and transforms

`validate_to_level` now takes `&CompletedDiagnostics`, replacing a raw diagnostic
slice. Consume the full, unsuppressed producer batch through
`CompletedDiagnostics::admit` first. Handle `InternalFailure` separately,
including internal findings with warning severity. Ordinary input findings
remain accessible through `diagnostics()`. Completion does not itself prove
CHAT validity or bind a model to its producer diagnostics.

Strict and lenient transform parsing both refuse internal parser failures.
Lenient parsing still permits its documented ordinary input recovery, but
retains the rejected parse's diagnostic context. Do not classify an internal
failure as a malformed transcript or as successful recovery.

Replace command strings passed to `validate_output` with the closed policy:

| Policy | Partial checks |
| --- | --- |
| `OutputChecks::Terminators` | Terminators, retaining the CA exemption |
| `OutputChecks::MorphologyCounts` | Terminators plus existing MOR position counts |
| `OutputChecks::UtteranceTiming` | Terminators plus terminal timing |

Choose the intended policy explicitly. These checks do not certify full CHAT
validity, transform preservation or alignment quality.

`SanitizedDocument::write_chat` accepts `W: ChatWriter` and returns
`Result<(), W::Error>`. Preserve the destination's associated error type in
adapters; existing `std::fmt::Write` destinations remain supported. Use the
infallible owned-string path when rendering a string.

Owned-string roundtrip serialization no longer has a formatting-error state.
Remove `SerializationPass` and matches on `RoundtripFailure::Serialization`.
Match `RoundtripFailure::Reparse { diagnostics }` for rejected serialized text
and `RoundtripFailure::Mismatch { diff }` for differing passes. Internal parser
failure remains distinct from both data outcomes.

## Low-level parser consumers

Tree-sitter `parse_wor_tier` now consumes
`HealthyTier<'tree, 'source, WorDependentTierNode<'tree>>`, as MOR lowering
already requires for its own node type. Match `TierAdmission::admit` and pass
its healthy evidence. Source binding alone does not rule out recovery. Retain
rejection/recovery instead of manufacturing a healthy carrier. The result
remains `Result<ParseOutcome<WorTier>, CstFailure>`; the input-based
`ChatParser` interface is unchanged.

Native marker-chain events and actions carry `ParsedEvent` and `ZeroAction`,
replacing mutable shared events and raw zero text. Inspect retained `span()`,
`event_type()` or `lexeme()` where provided; use `ParsedEvent::to_model()` to
lower an event. Enclosure delimiters are private: use `open()`, `close()` and
`span()`. `Retrace::marker` and retrace annotations carry `RetraceMarker`, with
`lexeme()` and `span()` accessors. Obtain these carriers from native parsing;
do not reconstruct extents from text lengths or fabricate source-free values.
Existing wire spelling and equality contracts remain unchanged. Spelling
equality does not establish equal source positions.

`Token::Word::lang_suffix` carries `Option<WordLanguagePayload>`. A present
suffix is `Shortcut` or `Codes(text)`: empty explicit code text rejects instead
of becoming bare `@s`. Native token debugging JSON now uses
`{"kind":"shortcut"}` or `{"kind":"codes","codes":"eng"}`, replacing
empty/code strings. Absence remains null. This is distinct from the shared
model's unchanged CHAT spelling and compatible suffix property names.

Generated traversal retains selected terminal-sequence decisions and removes
the absent state from fixed members of those selected sequences. Keep the
generated slot's absence type through shared lowering helpers. Lexical MISSING
and ERROR recovery remain explicit; these local guarantees do not establish
whole-parser correctness. Regenerate through the
[grammar workflow](grammar-workflow.md), never by editing generated source.

## Observable behavior

- Shortening-only `(xxx)`, `(yyy)` and `(www)` report E209 in
  standard CHAT. Bare markers and CA omission normalization retain their rules.
- A duration header with an empty comma-separated segment retains the entire
  unsupported text. `TimeDurationValue::segments()` no longer exposes a partial
  clock list. Do not replace this state with a valid prefix.
- MOR mismatch previews put terminators on a separate `end:` row. Consumers
  should not parse the human-readable preview as a stable machine protocol.
- File-location admission rejects trailing directory separators and `.`
  components. Deleted-directory cache prefixes still accept those spellings
  with their existing exact scope.
- LSP incremental edits reject nonexistent lines and endpoints inside a UTF-16
  surrogate pair. The document loses synchronization until full text arrives.
  Columns beyond an existing line still clamp to its end.
- Command discovery adds `exit_codes`, sourced from the same typed process
  contract as help and execution. Account for internal failure (3), required
  review (4) and forced interruption (130) as well as success/input/usage
  outcomes. `--log-format` controls stderr logging, not result encoding.
  Both text and JSON trace logs go to stderr when verbosity or `RUST_LOG`
  enables logging; stdout remains the command's result stream.

## Build and acceptance limits

Spec tooling uses Tera 2 with explicit filesystem template loading. Ordinary
builds consume committed traversal and metadata artifacts without the generator
checkout. Regenerating them still requires the exact maintainer-provided source
named by `tsgu-revision`; the public checkout alone does not reproduce those
files. See [setup](setup.md) and [grammar workflow](grammar-workflow.md).

This release does not declare parser completeness, independent public grammar
regeneration, crates.io availability or acceptance of every packaged platform.
Use the maintained [release checks](ci-and-release.md) for the exact candidate.
