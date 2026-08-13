#!/usr/bin/env python3
"""Keep the app version in sync across every file that carries a LITERAL copy.

ONE canonical source: the root `Cargo.toml` `[workspace.package] version`. Every
crate inherits it via `version.workspace = true`, and `chatter --version` reports
it. The desktop BUNDLE version (the .dmg / .exe / .deb filenames and the installed
app) ALSO inherits from that crate version: `tauri.conf.json` deliberately carries
NO "version" field, so Tauri falls back to the crate's `Cargo.toml` version. That
removal is what keeps the bundle from drifting, so this script does NOT read or
write `tauri.conf.json`. Two files still carry a literal copy of the version and
must never drift from it:

  * apps/chatter-desktop/package.json  "version"  (the npm side)
  * CHANGELOG.md                       a `## [X.Y.Z]` section AND its
                                       matching `[X.Y.Z]:` link reference

Why this matters: the desktop release's latest.json (the Tauri auto-updater
manifest) takes its version from the git TAG, while the bundle takes its version
from the crate version. The first v0.1.1 desktop release shipped a bundle/manifest
mismatch (bundle 0.1.0, manifest 0.1.1) back when `tauri.conf.json` carried its own
version; removing that field fixed the bundle side. This gate guards the literal
copies that remain (the npm `package.json` and the CHANGELOG section) so a missed
edit is a hard failure instead of something a releaser has to remember.

The workspace-dependencies table is a third carrier: every internal crate's
`path = "crates/...", version = "X.Y.Z"` pin (kept literal for crates.io
publication readiness) must match the workspace version, or `cargo check`
fails the moment the workspace version moves. `--bump` rewrites them; the
2026-07-30 v0.5.0 release stumbled on exactly this class (pins found by a
failed check, package.json found by failed CI, because the bump was a
hand-performed multi-file edit).

Usage:
  sync-app-version.py --check                 # CI: exit 1 if any file has drifted
  sync-app-version.py --fix                   # rewrite the version files to match
  sync-app-version.py --bump X.Y.Z            # set the canonical version and
                                              # rewrite EVERY literal copy
  sync-app-version.py --release-tag vX.Y.Z    # also assert the tag equals the version

The point: bumping the app version is ONE command (`just release-bump X.Y.Z`,
which runs `--bump` and refreshes both lockfiles) plus a human-written
CHANGELOG section; the `--check` gate makes a missed file a hard failure, and
`--release-tag` makes a tag that disagrees with the bundle a hard failure at
release time.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from enum import Enum
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CARGO = REPO / "Cargo.toml"
PACKAGE_JSON = REPO / "apps" / "chatter-desktop" / "package.json"
CHANGELOG = REPO / "CHANGELOG.md"

# A 2-space-indented top-level `"version": "..."` line, the shape package.json
# uses. Anchoring to the indent avoids matching a nested object's version.
JSON_VERSION_RE = re.compile(r'(?m)^(  "version"\s*:\s*")[^"]+(")')

# The canonical `[workspace.package] version` line in the root Cargo.toml.
WORKSPACE_VERSION_RE = re.compile(
    r'(?ms)(\[workspace\.package\].*?^version\s*=\s*")[^"]+(")'
)

# An internal path-dep pin in `[workspace.dependencies]`:
# `name = { path = "crates/...", version = "X.Y.Z" }`. These stay literal for
# crates.io publication readiness and must track the workspace version.
PATH_DEP_PIN_RE = re.compile(r'(path = "crates/[^"]+", version = ")[^"]+(")')

# A release version: three dot-separated integers, no leading `v`.
SEMVER_RE = re.compile(r"^\d+\.\d+\.\d+$")


def canonical_version() -> str:
    """The single source of truth: `[workspace.package] version` in Cargo.toml."""
    text = CARGO.read_text()
    section = re.search(r"\[workspace\.package\](.*?)(?:\n\[|\Z)", text, re.DOTALL)
    if not section:
        sys.exit("error: no [workspace.package] section in Cargo.toml")
    ver = re.search(r'^\s*version\s*=\s*"([^"]+)"', section.group(1), re.MULTILINE)
    if not ver:
        sys.exit("error: no version in [workspace.package]")
    return ver.group(1)


def json_version(path: Path) -> str:
    """The top-level "version" of a JSON file (validates that it parses)."""
    return json.loads(path.read_text())["version"]


def set_json_version(path: Path, version: str) -> None:
    """Rewrite only the top-level "version" line, preserving all other bytes."""
    text = path.read_text()
    new, n = JSON_VERSION_RE.subn(rf"\g<1>{version}\g<2>", text, count=1)
    if n != 1:
        sys.exit(f'error: no top-level "version" field in {path}')
    path.write_text(new)


class Fixability(Enum):
    """Whether `--fix` can repair one drift item, or a human must."""

    AUTO = "auto"
    """A version copy this script rewrites."""

    MANUAL = "manual"
    """Prose or a tag; `--fix` must still exit non-zero."""


@dataclass(frozen=True)
class Drift:
    """One thing that disagrees with the canonical version.

    Carries its own fixability instead of leaving the caller to recover it by
    prefix-matching the message. The exit code used to be computed with
    `d.startswith(("CHANGELOG.md", "release tag"))` over the display strings
    this same function had just built, so rewording a message silently made
    `--fix` exit 0 on a state it cannot fix.
    """

    message: str
    fixability: Fixability


def changelog_gaps(version: str) -> list[str]:
    """Everything wrong with the CHANGELOG, as human-readable phrases.

    ONE invariant, checked in both directions: every release named anywhere in
    the file has BOTH a `## [X.Y.Z]` section and a matching `[X.Y.Z]: <url>`
    link reference. The canonical version is folded into the required set, so
    "the version being cut has a section" is not a separate check but the
    single-element case of the same comparison.

    That fold is the point. The two used to be separate, their domains
    overlapped by construction, and one missing heading produced TWO drift
    lines while `len(drift)` is printed as a count. Suppressing the duplicate
    at the reporting layer fixed the symptom and left the overlap for whoever
    adds a third check.

    A heading whose definition is missing renders as literal bracketed text
    rather than as a broken link, so the book's lychee pass reports zero errors
    and a reader sees only slightly odd punctuation. Five consecutive releases
    (v0.6.0 through v0.9.1) shipped that way, which is why this compares SETS
    rather than looking up the version being released: the latter catches the
    sixth and is blind to the five already there, and blind entirely to the
    reverse error.
    """
    text = CHANGELOG.read_text()
    headings = set(re.findall(r"^## \[([^\]]+)\]", text, re.MULTILINE)) - {"Unreleased"}
    definitions = set(re.findall(r"^\[([^\]]+)\]: \S", text, re.MULTILINE)) - {"Unreleased"}
    required = headings | definitions | {version}

    gaps: list[str] = []
    for missing in sorted(required - headings):
        gaps.append(f"a `## [{missing}]` section")
    for missing in sorted(required - definitions):
        gaps.append(f"a `[{missing}]:` link reference")
    return gaps


def bump_canonical(version: str) -> None:
    """Set `[workspace.package] version` and every internal path-dep pin.

    The path-dep pins live in the same Cargo.toml, so one read-modify-write
    covers both; the remaining literal copies (package.json) then follow via
    the ordinary fix pass against the new canonical version.
    """
    if not SEMVER_RE.match(version):
        sys.exit(f"error: --bump takes X.Y.Z (no leading v), got {version!r}")
    text = CARGO.read_text()
    text, n_ws = WORKSPACE_VERSION_RE.subn(rf"\g<1>{version}\g<2>", text, count=1)
    if n_ws != 1:
        sys.exit("error: no [workspace.package] version line in Cargo.toml")
    text, n_pins = PATH_DEP_PIN_RE.subn(rf"\g<1>{version}\g<2>", text)
    CARGO.write_text(text)
    print(f"canonical version -> {version} (workspace + {n_pins} path-dep pins)")


def process(fix: bool, release_tag: str | None, changelog_reminder_only: bool = False) -> int:
    want = canonical_version()
    drift: list[Drift] = []

    for path in (PACKAGE_JSON,):
        have = json_version(path)
        if have != want:
            drift.append(Drift(f"{path.relative_to(REPO)}  {have} -> {want}", Fixability.AUTO))
            if fix:
                set_json_version(path, want)

    gaps = changelog_gaps(want)
    if gaps:
        if changelog_reminder_only:
            # `--bump` runs BEFORE the human writes the section; the `--check`
            # gate (CI, and the tag script) still enforces it afterwards.
            print(f"next: write {' and '.join(gaps)} in CHANGELOG.md")
        else:
            # A changelog entry is human-written, so this is never auto-fixable.
            for gap in gaps:
                drift.append(Drift(f"CHANGELOG.md  missing {gap}", Fixability.MANUAL))

    if release_tag is not None:
        tag_version = release_tag[1:] if release_tag.startswith("v") else release_tag
        if tag_version != want:
            drift.append(
                Drift(f"release tag {release_tag} ({tag_version}) != version {want}", Fixability.MANUAL)
            )

    if not drift:
        scope = f", tag {release_tag} ok" if release_tag else ""
        print(f"app version in sync: {want}{scope}")
        return 0

    verb = "fixed" if fix else "DRIFTED"
    print(f"{verb} ({len(drift)}): canonical version = {want}")
    for d in drift:
        print(f"  {d.message}")
    # CHANGELOG and release-tag drift cannot be auto-fixed, so `--fix` still fails
    # when either is wrong; otherwise a successful rewrite is success.
    unfixable = any(d.fixability is Fixability.MANUAL for d in drift)
    return 1 if (not fix or unfixable) else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    mode = ap.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="fail on drift (CI mode)")
    mode.add_argument("--fix", action="store_true", help="rewrite the version files")
    mode.add_argument(
        "--bump",
        metavar="X.Y.Z",
        help="set the canonical version and rewrite every literal copy",
    )
    ap.add_argument(
        "--release-tag",
        metavar="vX.Y.Z",
        help="also assert this release tag equals the canonical version",
    )
    args = ap.parse_args()
    if args.bump:
        bump_canonical(args.bump)
        return process(fix=True, release_tag=args.release_tag, changelog_reminder_only=True)
    # Default to check semantics so an accidental bare run never mutates files.
    return process(fix=args.fix, release_tag=args.release_tag)


if __name__ == "__main__":
    raise SystemExit(main())
