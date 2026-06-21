#!/usr/bin/env python3
"""Neutralize real corpus names in the @ID headers of CONSTRUCTED reference fixtures.

The reference corpus under ``corpus/reference/`` is hand-built test data. A number
of constructed fixtures cosmetically carry a real CHILDES corpus name in their
``@ID`` header's corpus field (e.g. ``@ID: deu|Caroline|...``), which makes them
look like shipped named-corpus data even though the transcript content is invented.
This rewrites that corpus field to the neutral placeholder ``corpus`` (the
placeholder the other reference fixtures already use), touching ONLY the ``@ID``
corpus field and leaving every other byte unchanged.

Idempotent. Run from the chatter repo root. After running, regenerate the affected
insta snapshots and reference-XML goldens (see the session notes), since the
corpus name flows into both.
"""

from __future__ import annotations

import pathlib
import sys

PLACEHOLDER = "corpus"
# Corpus-field values that are already neutral and must be left alone.
NEUTRAL = {"corpus", "sample", ""}
# The @ID corpus field is the second pipe-delimited field after the ``@ID:`` tag.
CORPUS_FIELD_INDEX = 1


def neutralize_id_line(line: str) -> str | None:
    """Return the rewritten ``@ID`` line, or ``None`` if it needs no change."""
    if not line.startswith("@ID:"):
        return None
    tag, _, rest = line.partition("\t")
    if not rest:
        return None
    fields = rest.split("|")
    if len(fields) <= CORPUS_FIELD_INDEX:
        return None
    if fields[CORPUS_FIELD_INDEX].strip() in NEUTRAL:
        return None
    fields[CORPUS_FIELD_INDEX] = PLACEHOLDER
    return f"{tag}\t{'|'.join(fields)}"


def neutralize_file(path: pathlib.Path) -> bool:
    """Rewrite ``path`` in place. Return True if any line changed."""
    original = path.read_text(encoding="utf-8")
    out_lines = []
    changed = False
    for line in original.splitlines(keepends=True):
        stripped = line.rstrip("\n")
        newline = "\n" if line.endswith("\n") else ""
        rewritten = neutralize_id_line(stripped)
        if rewritten is None:
            out_lines.append(line)
        else:
            out_lines.append(rewritten + newline)
            changed = True
    if changed:
        path.write_text("".join(out_lines), encoding="utf-8")
    return changed


def main() -> int:
    root = pathlib.Path("corpus/reference")
    if not root.is_dir():
        print("run from the chatter repo root (corpus/reference not found)", file=sys.stderr)
        return 2
    changed = [p for p in sorted(root.rglob("*.cha")) if neutralize_file(p)]
    print(f"neutralized @ID corpus field in {len(changed)} fixture(s):")
    for p in changed:
        print(f"  {p}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
