#!/usr/bin/env python3
"""Keep the Rust version pins in CI workflows in sync with their sources of truth.

There are two distinct version concepts, each with ONE canonical source:

  * the CI/dev toolchain  -> `rust-toolchain.toml` `channel`   (e.g. 1.96.0)
  * the workspace MSRV    -> root `Cargo.toml` `rust-version`  (e.g. 1.89.0)

Every numeric Rust-version pin in `.github/workflows/*.yml` must equal one of
these. A pin tracks the MSRV when its line carries a `# rust-msrv` marker
comment; otherwise it tracks the toolchain. Non-numeric refs (`@stable`,
`@nightly`, a commit SHA) are intentionally floating and are left alone, so the
weekly `clippy-rolling.yml` drift job and the `@stable` book job keep working.

Usage:
  sync-rust-versions.py --check   # CI mode: exit 1 if any pin has drifted
  sync-rust-versions.py --fix     # rewrite drifted pins to match their source

The point: bumping Rust is editing ONE file (rust-toolchain.toml, or Cargo.toml
for the MSRV) plus `--fix`; the `--check` gate makes drift a hard failure
instead of a thing someone has to remember.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
WORKFLOWS = REPO / ".github" / "workflows"

# A numeric semver pin in the two shapes CI uses: the action tag
# `dtolnay/rust-toolchain@X.Y.Z` and the action input `toolchain: X.Y.Z`.
PIN_RE = re.compile(
    r"(?P<prefix>dtolnay/rust-toolchain@|toolchain:\s*)(?P<ver>\d+\.\d+\.\d+)"
)
MSRV_MARKER = "rust-msrv"


def read_channel() -> str:
    """The canonical CI/dev toolchain from rust-toolchain.toml."""
    text = (REPO / "rust-toolchain.toml").read_text()
    m = re.search(r'^\s*channel\s*=\s*"([^"]+)"', text, re.MULTILINE)
    if not m:
        sys.exit("error: no `channel` in rust-toolchain.toml")
    return m.group(1)


def read_msrv() -> str | None:
    """The canonical workspace MSRV from the root Cargo.toml, or None.

    MSRV is optional: the workspace declares `rust-version` only when there is
    a deliberate minimum-supported-Rust promise (currently there is none, since
    crates.io publication is deferred). A `# rust-msrv`-marked pin without a
    `rust-version` to track is an error, reported in `process`.
    """
    text = (REPO / "Cargo.toml").read_text()
    m = re.search(r'^\s*rust-version\s*=\s*"([^"]+)"', text, re.MULTILINE)
    return m.group(1) if m else None


def expected_for(line: str, channel: str, msrv: str | None) -> str:
    """A pin tracks the MSRV iff its line is marked, else the toolchain."""
    if MSRV_MARKER in line:
        if msrv is None:
            sys.exit(
                "error: a pin is marked `# rust-msrv` but Cargo.toml declares "
                "no `rust-version` to track"
            )
        return msrv
    return channel


def process(fix: bool) -> int:
    channel, msrv = read_channel(), read_msrv()
    drifted: list[str] = []
    for wf in sorted(WORKFLOWS.glob("*.yml")):
        original = wf.read_text()
        new_lines = []
        changed = False
        for lineno, line in enumerate(original.splitlines(keepends=True), 1):
            m = PIN_RE.search(line)
            if not m:
                new_lines.append(line)
                continue
            want = expected_for(line, channel, msrv)
            if m.group("ver") != want:
                where = f"{wf.relative_to(REPO)}:{lineno}"
                kind = "msrv" if MSRV_MARKER in line else "toolchain"
                drifted.append(f"{where}  {m.group('ver')} -> {want}  ({kind})")
                line = line[: m.start("ver")] + want + line[m.end("ver") :]
                changed = True
            new_lines.append(line)
        if changed and fix:
            wf.write_text("".join(new_lines))

    msrv_note = f", msrv={msrv}" if msrv else " (no MSRV declared)"
    if not drifted:
        print(f"rust versions in sync (toolchain={channel}{msrv_note})")
        return 0
    verb = "fixed" if fix else "DRIFTED"
    print(f"{verb} ({len(drifted)}): toolchain={channel}{msrv_note}")
    for d in drifted:
        print(f"  {d}")
    # In --fix mode a successful rewrite is success; in --check it is failure.
    return 0 if fix else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    g = ap.add_mutually_exclusive_group()
    g.add_argument("--check", action="store_true", help="fail on drift (CI mode)")
    g.add_argument("--fix", action="store_true", help="rewrite drifted pins")
    args = ap.parse_args()
    # Default to --check so an accidental bare run never mutates files.
    return process(fix=args.fix)


if __name__ == "__main__":
    raise SystemExit(main())
