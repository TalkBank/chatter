#!/usr/bin/env python3
"""Derive chatter's language-code data from the official ISO 639-3 tables.

ISO 639-3 is administered by SIL International as the ISO registration
authority, which publishes dated code tables at
<https://iso639-3.sil.org/code_tables/download_tables>. This script downloads a
release and writes the small derived file the model actually needs.

Why a derived file rather than the tables themselves
----------------------------------------------------
The tables carry reference names, macrolanguage mappings, scope and type.
chatter needs none of that: it answers one question, "is this a real language
code", and wants a replacement suggestion when a code has been retired. So the
output is two columns of identifiers plus a status, which is what SIL's terms
of use call incorporating the code set into a software product rather than
redistributing the code tables. Attribution is written into the file itself.

Why the file is committed
-------------------------
Build-time data must be local: builds have to work offline, reproducibly, and
in CI without network, and a build that fetches from a URL breaks when the URL
moves. So the derived file is committed and this script is run deliberately,
by a human, when a new release appears.

What goes in, and why each category is there
--------------------------------------------
- `current`: the 7,900-odd codes in `iso-639-3.tab`.
- `retired`: from `iso-639-3_Retirements.tab`. These stay VALID. A CHAT file is
  a historical document, and a transcript from 1995 must not become invalid
  because a code was retired in 2026. The replacement is carried in the
  `change_to` column. NOTE that nothing reads it yet: `build.rs` generates a
  membership set, so validation can say a retired code is fine but cannot say
  "retired in 2009, use `tzo`". Surfacing that needs a decision about whether
  chatter should WARN on a retired code at all, which is a policy question, not
  a plumbing one. The column is recorded now so the answer is not blocked on
  re-downloading the tables later.
- `private_use`: `qaa` through `qtz`, 520 codes the standard reserves for local
  use. They appear in no table because they are reserved rather than assigned,
  so they have to be generated. Dropping them would reject legitimate local
  codes; SIL's terms name this range explicitly.

Usage:
    update_iso639_3.py --zip <iso-639-3_Code_Tables_YYYYMMDD.zip> --out <path>
    update_iso639_3.py --release 20260715 --out <path>     (downloads it)
"""

from __future__ import annotations

import argparse
import collections
import enum
import io
import re
import string
import sys
import urllib.request
import zipfile
from dataclasses import dataclass
from pathlib import Path

DOWNLOAD_URL = (
    "https://iso639-3.sil.org/sites/iso639-3/files/downloads/"
    "iso-639-3_Code_Tables_{release}.zip"
)
ATTRIBUTION = "iso639-3.sil.org"
# The standard reserves this block for local use; see SIL's terms of use.
PRIVATE_USE_FIRST_SECOND = string.ascii_lowercase[:20]  # a..t, giving qaa..qtz

RELEASE_RE = re.compile(r"iso-639-3_Code_Tables_(\d{8})\.zip$")
CODE_RE = re.compile(r"^[a-z]{3}$")


class Status(enum.StrEnum):
    """How the registry currently treats a code. A closed set, so an enum."""

    CURRENT = "current"
    RETIRED = "retired"
    PRIVATE_USE = "private_use"


@dataclass(frozen=True)
class CodeRow:
    """One language code as chatter records it."""

    code: str
    status: Status
    change_to: str | None  # replacement for a retired code; None otherwise


def _table_rows(archive: zipfile.ZipFile, suffix: str) -> list[list[str]]:
    """Return the data rows of the one member whose name ends with `suffix`."""
    names = [n for n in archive.namelist() if n.endswith(suffix)]
    if len(names) != 1:
        raise SystemExit(
            f"expected exactly one member ending {suffix!r}, found {names}"
        )
    text = archive.read(names[0]).decode("utf-8")
    lines = text.splitlines()
    if not lines:
        raise SystemExit(f"{names[0]} is empty")
    return [line.split("\t") for line in lines[1:] if line.strip()]


def derive(archive: zipfile.ZipFile) -> list[CodeRow]:
    """Build the full code inventory from a release archive."""
    rows: dict[str, CodeRow] = {}

    for fields in _table_rows(archive, "iso-639-3.tab"):
        code = fields[0].strip()
        if not CODE_RE.match(code):
            raise SystemExit(f"unexpected code {code!r} in iso-639-3.tab")
        rows[code] = CodeRow(code=code, status=Status.CURRENT, change_to=None)

    for fields in _table_rows(archive, "iso-639-3_Retirements.tab"):
        code = fields[0].strip()
        if not CODE_RE.match(code):
            raise SystemExit(f"unexpected code {code!r} in the retirements table")
        # A code can be retired and later reassigned; the current table wins,
        # because that is what the code means today.
        if code in rows:
            continue
        replacement = fields[3].strip() if len(fields) > 3 else ""
        rows[code] = CodeRow(
            code=code, status=Status.RETIRED, change_to=replacement or None
        )

    for second in PRIVATE_USE_FIRST_SECOND:
        for third in string.ascii_lowercase:
            code = f"q{second}{third}"
            # Never let a generated private-use code shadow a real assignment.
            if code not in rows:
                rows[code] = CodeRow(
                    code=code, status=Status.PRIVATE_USE, change_to=None
                )

    return [rows[c] for c in sorted(rows)]


def render(rows: list[CodeRow], release: str) -> str:
    counts = collections.Counter(r.status for r in rows)
    header = [
        "# ISO 639-3 language codes used by chatter.",
        "#",
        f"# Source: the ISO 639-3 code tables published by {ATTRIBUTION}, the ISO",
        "# registration authority for the standard. This file is a DERIVED product:",
        "# it carries the identifiers chatter needs and nothing else, and it is not",
        "# the code tables. The identifiers themselves are unmodified.",
        "#",
        f"# Release: {release}",
        "# Generated by: scripts/update_iso639_3.py (re-run it when a new release",
        "# appears; never hand-edit this file).",
        "#",
        "# Columns: code<TAB>status<TAB>change_to",
        "#   current      assigned today.",
        "#   retired      no longer assigned, still VALID here because a CHAT file is",
        "#                a historical document; change_to names its replacement.",
        "#   private_use  qaa..qtz, reserved by the standard for local use.",
        "#",
        (
            f"# Totals: {counts[Status.CURRENT]} current, "
            f"{counts[Status.RETIRED]} retired, "
            f"{counts[Status.PRIVATE_USE]} private use, {len(rows)} in all."
        ),
    ]
    body = [f"{r.code}\t{r.status}\t{r.change_to or ''}" for r in rows]
    return "\n".join(header + body) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    src = ap.add_mutually_exclusive_group(required=True)
    src.add_argument("--zip", type=Path, help="a downloaded code-tables zip")
    src.add_argument("--release", help="release stamp, e.g. 20260715; downloads it")
    ap.add_argument("--out", type=Path, required=True, help="derived file to write")
    args = ap.parse_args()

    if args.zip:
        match = RELEASE_RE.search(args.zip.name)
        if not match:
            raise SystemExit(
                f"cannot read a release stamp from {args.zip.name!r}; the release date is "
                "the only version these tables carry, so it must be recorded"
            )
        release = match.group(1)
        data = args.zip.read_bytes()
    else:
        release = args.release
        url = DOWNLOAD_URL.format(release=release)
        print(f"downloading {url}", file=sys.stderr)
        # The host and scheme are fixed constants above, not caller input.
        with urllib.request.urlopen(url) as response:
            data = response.read()

    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        rows = derive(archive)

    args.out.write_text(render(rows, release), encoding="utf-8")
    print(f"wrote {len(rows)} codes to {args.out} (release {release})", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
