---
name: regen-json-schema
description: Regenerate the embedded CHAT JSON Schema after changing any ChatFile model type. Use after ANY change to a talkbank-model type's shape, fields, enum variants, serde attributes, or doc comments (doc comments feed schemars!).
allowed-tools: Bash, Read, Grep
---

# JSON Schema Regeneration

The canonical schema (`schema/chat-file.schema.json`) is generated
from the talkbank-model types and EMBEDDED into the binary at compile
time (`include_str!`). It never regenerates itself, and `chatter
to-json` validates its own output against the embedded copy, so a
stale schema makes to-json reject valid files.

**Decision test:** did the change touch a model type's shape, fields,
variants, serde attributes, OR `#[doc]`/`///` comments? Doc comments
feed schemars descriptions, so pure doc edits STILL require
regeneration (a 2026-07 CI red proved it).

Sequence, in the SAME change:

1. `cargo test -p talkbank-transform --test generate_schema`
   (rewrites the schema file; run it twice: first writes, second
   verifies green).
2. `cargo build -p chatter` (the rebuild is not optional; the schema
   is compile-time embedded).
3. Commit the regenerated `schema/chat-file.schema.json` alongside
   the model change.

Guard: `committed_schema_matches_model` fails the suite on a
forgotten regeneration. Wire-format note: `string_newtype!` types are
serde/schemars-transparent, so newtyping a string field should leave
the schema byte-identical; verify with `git diff schema/`.
