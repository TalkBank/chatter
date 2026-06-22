#!/usr/bin/env python3
"""Keep the app version in sync across every file that carries it.

ONE canonical source: the root `Cargo.toml` `[workspace.package] version`. Every
crate inherits it via `version.workspace = true`, and `chatter --version` reports
it. Three other files carry the SAME version and must never drift from it:

  * apps/chatter-desktop/src-tauri/tauri.conf.json  "version"  (the desktop BUNDLE
    version: what the .dmg / .exe / .deb filenames and the installed app report)
  * apps/chatter-desktop/package.json               "version"  (the npm side)
  * CHANGELOG.md                                    a `## [X.Y.Z]` section

Why this matters: the desktop release's latest.json (the Tauri auto-updater
manifest) takes its version from the git TAG, while the bundle takes its version
from tauri.conf.json. If those drift, the updater advertises a version the
installed app never reaches and desktop users get a perpetual update loop. The
first v0.1.1 desktop release shipped exactly that (bundle 0.1.0, manifest 0.1.1).
This gate makes the drift a hard failure instead of something a releaser has to
remember.

Usage:
  sync-app-version.py --check                 # CI: exit 1 if any file has drifted
  sync-app-version.py --fix                   # rewrite the version files to match
  sync-app-version.py --release-tag vX.Y.Z    # also assert the tag equals the version

The point: bumping the app version is editing ONE file (Cargo.toml
[workspace.package] version) plus `just app-sync` and a CHANGELOG entry; the
`--check` gate makes a missed file a hard failure, and `--release-tag` makes a
tag that disagrees with the bundle a hard failure at release time.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CARGO = REPO / "Cargo.toml"
TAURI_CONF = REPO / "apps" / "chatter-desktop" / "src-tauri" / "tauri.conf.json"
PACKAGE_JSON = REPO / "apps" / "chatter-desktop" / "package.json"
CHANGELOG = REPO / "CHANGELOG.md"

# A 2-space-indented top-level `"version": "..."` line, the shape both
# tauri.conf.json and package.json use. Anchoring to the indent avoids matching a
# nested object's version.
JSON_VERSION_RE = re.compile(r'(?m)^(  "version"\s*:\s*")[^"]+(")')


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


def changelog_has(version: str) -> bool:
    pattern = rf"^## \[{re.escape(version)}\]"
    return re.search(pattern, CHANGELOG.read_text(), re.MULTILINE) is not None


def process(fix: bool, release_tag: str | None) -> int:
    want = canonical_version()
    drift: list[str] = []

    for path in (TAURI_CONF, PACKAGE_JSON):
        have = json_version(path)
        if have != want:
            drift.append(f"{path.relative_to(REPO)}  {have} -> {want}")
            if fix:
                set_json_version(path, want)

    if not changelog_has(want):
        # A changelog entry is human-written, so this is never auto-fixable.
        drift.append(f"CHANGELOG.md  missing a `## [{want}]` section")

    if release_tag is not None:
        tag_version = release_tag[1:] if release_tag.startswith("v") else release_tag
        if tag_version != want:
            drift.append(f"release tag {release_tag} ({tag_version}) != version {want}")

    if not drift:
        scope = f", tag {release_tag} ok" if release_tag else ""
        print(f"app version in sync: {want}{scope}")
        return 0

    verb = "fixed" if fix else "DRIFTED"
    print(f"{verb} ({len(drift)}): canonical version = {want}")
    for d in drift:
        print(f"  {d}")
    # CHANGELOG and release-tag drift cannot be auto-fixed, so `--fix` still fails
    # when either is wrong; otherwise a successful rewrite is success.
    unfixable = any(d.startswith(("CHANGELOG.md", "release tag")) for d in drift)
    return 1 if (not fix or unfixable) else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    mode = ap.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="fail on drift (CI mode)")
    mode.add_argument("--fix", action="store_true", help="rewrite the version files")
    ap.add_argument(
        "--release-tag",
        metavar="vX.Y.Z",
        help="also assert this release tag equals the canonical version",
    )
    args = ap.parse_args()
    # Default to check semantics so an accidental bare run never mutates files.
    return process(fix=args.fix, release_tag=args.release_tag)


if __name__ == "__main__":
    raise SystemExit(main())
