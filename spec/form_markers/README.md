# Form Marker Registry

**Status:** Current
**Last updated:** 2026-08-11 16:20 EDT

`form_marker_registry.json` is the single owner of the CHAT special-form marker
set: the `@` suffix a word can carry (`gumma@c`, `b@l`, `word@z:rtfd`).

## What is generated from it

| Site | File |
|------|------|
| The `FormType` enum, its per-variant docs, `from_payload`, `to_chat_marker`, the E203 suggestion | `crates/talkbank-model/src/generated/form_markers.rs` |
| The re2c lexer's marker code set | `crates/talkbank-parser-re2c/src/generated_form_markers.re` |
| The book's marker table | `book/src/chat-format/generated/form-markers.md` |

```bash
just form-markers-gen
```

Two follow-ups the generator cannot do, and says so on every run:

1. The re2c output feeds a VENDORED lexer, and **nothing checks that the
   committed lexer matches it**. No CI workflow installs re2c, so no CI job can;
   `build.rs` used to claim one existed, which was worse than saying nothing.
   Run `just verify-vendored-lexer` (under a second) in the same commit.
2. A change to the enum's shape or doc comments changes the JSON Schema, which
   is embedded at compile time. Regenerate it in the same commit:
   `cargo test -p talkbank-transform --tests generate_schema`.

## The gate

The gate covers three of the four hops. The fourth, `generated_form_markers.re`
to the vendored `lexer.rs`, has no automated check at all; see the follow-up
above.

`generated_form_marker_sites_are_current` in `spec/tools/src/form_markers/`
iterates `render::OUTPUTS`, the same list the generator writes, and asserts each
committed artifact equals what its renderer produces. It calls the RENDERER, not
a second description of the output, so the gate has nothing of its own to drift
from.

It lives in the `spec/` workspace, which `just test` does NOT reach: it runs
under `just test-spec`, `just test-all` and CI
(`cargo test --manifest-path spec/Cargo.toml --workspace`). So a hand-edit to a
generated file survives the inner loop and is caught at the pre-push gate.

Proven to fail in both directions before being believed: hand-editing a
generated file fails it naming that file, and editing the registry without
regenerating fails it too.

## Adding or retiring a marker

Edit the JSON, run `just form-markers-gen`, do the two follow-ups. Row order in
the file does not matter; the loader sorts by marker code.

Retiring one is the case this registry was built for. `@a` took two years and
four coordinated hand-edits in one file plus the book, and was still advertised
in a test's doc comment afterwards. It is now one deleted row.

`committed_registry_matches_the_sanctioned_set` will fail, deliberately: it
pins the marker set against `depfile.cut` and is the one assertion here that a
reviewer should stop and think about. Update it in the same commit, with the
evidence in the commit message.

## Authorities, in order

1. **`clan-info/lib/depfile.cut`**, the main-line `*@...` entries. This is what
   the corpus authority SANCTIONS and is more current than the manual. It also
   settles the label question structurally: bare `*@x` beside `*@s:*` and
   `*@z:*`.
2. **The CHAT manual's "Special Form Markers" table**
   (<https://talkbank.org/0info/manuals/CHAT.html#Special_Form_Markers>) for
   MEANINGS, with a per-marker anchor for each. Fetch it; do not trust a gloss
   already in the codebase. On 2026-08-11 six of them were fabricated, each a
   plausible expansion of the marker's letters rather than its meaning: `@k`
   read "kinship" (it is "kana", multiple letters), `@sl` read "slang" (it is
   signed language, which matters because TalkBank hosts sign corpora).
   The manual is also WRONG in one place recorded here: it writes `@x:*` in the
   Letters column while its own Example column writes bare `stuff@x`. The
   `manual_disagreement` field on that row records it.
3. **The corpus**, which is the strongest single signal but needs an exhaustive
   probe. A main-tier-only search for `@a` returned 0 while a full-line search
   returned 16 files, all on `%xpho` tiers.
4. **The corpus authority's own decisions**, which are the reason a marker
   exists or stops existing and are not always visible in any of the above. On
   2024-09-03 `@a`, `@e` and `@lp` were eliminated from every file as "either
   not used or used rarely". `@fp` and `@x` were proposed for removal in the
   same decision and kept, because the corpus work was judged too extensive.

Do NOT pin glosses to a checked-in copy of the manual. That would make a dated
source authoritative, and it would have blessed `@a`.

## Why the example is a stem

A row stores `example_stem`, not a whole example, and the example is built as
`<stem>@<marker>` (plus `:<label>` where the policy requires one). A stored
example is free to name a different marker, and in a table of twenty-two
near-identical rows that is the mistake to expect. Deriving it removes the
error rather than checking for it.
