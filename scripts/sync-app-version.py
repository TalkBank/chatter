#!/usr/bin/env python3
"""Keep the app version in sync across every file that carries a LITERAL copy.

ONE canonical source: the root `Cargo.toml` `[workspace.package] version`. Every
crate inherits it via `version.workspace = true`, and `chatter --version` reports
it. The desktop BUNDLE version (the .dmg / .exe / .deb filenames and the installed
app) ALSO inherits from that crate version: `tauri.conf.json` deliberately carries
NO "version" field, so Tauri falls back to the crate's `Cargo.toml` version. That
removal is what keeps the bundle from drifting, so this script does NOT read or
write `tauri.conf.json`. The npm manifests and changelog carry literal copies and
must never drift from it:

  * apps/chatter-desktop/package.json  "version"  (the npm side)
  * apps/chatter-desktop/package-lock.json  root and packages[""].version
  * CHANGELOG.md                       a `## [X.Y.Z]` section AND its
                                       matching `[X.Y.Z]:` link reference

Why this matters: the desktop release's latest.json (the Tauri auto-updater
manifest) takes its version from the git TAG, while the bundle takes its version
from the crate version. The first v0.1.1 desktop release shipped a bundle/manifest
mismatch (bundle 0.1.0, manifest 0.1.1) back when `tauri.conf.json` carried its own
version; removing that field fixed the bundle side. This gate guards the literal
copies that remain (the npm manifest/lockfile and the CHANGELOG section) so a missed
edit is a hard failure instead of something a releaser has to remember.

The workspace-dependencies table also carries version copies: every internal crate's
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
which runs `--bump` and refreshes both Rust lockfiles) plus a human-written
CHANGELOG section; the `--check` gate makes a missed file a hard failure, and
`--release-tag` makes a tag that disagrees with the bundle a hard failure at
release time.
"""

from __future__ import annotations

import argparse
import json
import re
import stat
import sys
import tempfile
import tomllib
from contextlib import ExitStack
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CARGO = REPO / "Cargo.toml"
PACKAGE_JSON = REPO / "apps" / "chatter-desktop" / "package.json"
PACKAGE_LOCK = PACKAGE_JSON.with_name("package-lock.json")
CHANGELOG = REPO / "CHANGELOG.md"


class VersionIdentityError(ValueError):
    """A requested or stored identity cannot authorize version-file writes."""


@dataclass(frozen=True)
class AppVersion:
    """The command's normal X.Y.Z release version, admitted without normalization."""

    value: str

    def __post_init__(self) -> None:
        if re.fullmatch(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)", self.value) is None:
            raise VersionIdentityError(
                f"expected X.Y.Z with ASCII digits and no leading zeros, got {self.value!r}"
            )


@dataclass(frozen=True)
class VersionIdentity:
    """An admitted version agreeing with the optional caller-supplied release tag."""

    version: AppVersion
    release_tag: str | None

    def __post_init__(self) -> None:
        if self.release_tag is not None:
            tag_version = AppVersion(self.release_tag.removeprefix("v"))
            if tag_version != self.version:
                raise VersionIdentityError(
                    f"release tag {self.release_tag} != version {self.version.value}"
                )


class SyncMode(Enum):
    CHECK = "check"
    FIX = "fix"
    BUMP = "bump"


@dataclass(frozen=True)
class VersionFieldCandidate:
    """A candidate literal span; the complete edit must still pass TOML comparison."""

    key: tuple[str, ...]
    start: int
    end: int


def table_body(source: str, name: str) -> tuple[int, str]:
    """Locate the supported explicit table spelling without crossing a table."""
    headers = list(re.finditer(
        rf"(?m)^[ \t]*\[{re.escape(name)}\][ \t]*(?:#[^\r\n]*)?\r?$", source,
    ))
    if len(headers) != 1:
        raise VersionIdentityError(f"expected one explicit [{name}] table in Cargo.toml")
    start = headers[0].end()
    following = re.search(r"(?m)^[ \t]*\[", source[start:])
    end = start + following.start() if following is not None else len(source)
    return start, source[start:end]


def literal_field(
    key: tuple[str, ...], offset: int, text: str, pattern: str, value: str,
) -> VersionFieldCandidate:
    """Require one source literal agreeing with the parsed field's value."""
    matches = list(re.finditer(pattern, text, re.MULTILINE))
    if len(matches) != 1 or matches[0].group("value") != value:
        raise VersionIdentityError(f"unsupported or ambiguous literal layout for {'.'.join(key)}")
    match = matches[0]
    return VersionFieldCandidate(key, offset + match.start("value"), offset + match.end("value"))


@dataclass(frozen=True)
class CargoVersionDocument:
    """Parsed Cargo input and candidate source locations for its owned keys."""

    source: str
    version: AppVersion = field(init=False)
    fields: tuple[VersionFieldCandidate, ...] = field(init=False)

    @classmethod
    def read(cls, path: Path) -> CargoVersionDocument:
        """Preserve source line endings rather than normalizing through text I/O."""
        try:
            source = path.read_bytes().decode("utf-8")
        except (OSError, UnicodeError) as error:
            raise VersionIdentityError(f"cannot read canonical Cargo.toml: {error}") from error
        return cls(source)

    def __post_init__(self) -> None:
        try:
            document = tomllib.loads(self.source)
            version = document["workspace"]["package"]["version"]
        except (tomllib.TOMLDecodeError, KeyError, TypeError) as error:
            raise VersionIdentityError(f"invalid canonical Cargo.toml: {error}") from error
        if not isinstance(version, str):
            raise VersionIdentityError("workspace.package.version must be a string")
        object.__setattr__(self, "version", AppVersion(version))
        offset, body = table_body(self.source, "workspace.package")
        fields = [literal_field(
            ("workspace", "package", "version"), offset, body,
            r'''^[ \t]*version[ \t]*=[ \t]*(?P<quote>["'])(?P<value>[^"'\r\n]*)(?P=quote)[ \t]*(?:#[^\r\n]*)?\r?$''',
            version,
        )]
        dependencies = document["workspace"].get("dependencies")
        if dependencies is not None:
            if not isinstance(dependencies, dict):
                raise VersionIdentityError("workspace.dependencies must be a table")
            for name, dependency in dependencies.items():
                if not isinstance(dependency, dict):
                    continue
                path = dependency.get("path")
                if path is None:
                    continue
                if not isinstance(path, str):
                    raise VersionIdentityError(f"dependency {name} has a non-string path")
                if not path.startswith("crates/"):
                    continue
                pin = dependency.get("version")
                if not isinstance(pin, str):
                    raise VersionIdentityError(f"internal dependency {name} needs a literal version")
                offset, body = table_body(self.source, "workspace.dependencies")
                entries = list(re.finditer(
                    rf"(?m)^[ \t]*{re.escape(name)}[ \t]*=[ \t]*(?P<entry>\{{[^\r\n]*\}})[ \t]*(?:#[^\r\n]*)?\r?$", body,
                ))
                if len(entries) != 1:
                    raise VersionIdentityError(f"unsupported inline dependency layout for {name}")
                entry = entries[0]
                fields.append(literal_field(
                    ("workspace", "dependencies", name, "version"),
                    offset + entry.start("entry"), entry.group("entry"),
                    r'''(?:\{|,)[ \t]*version[ \t]*=[ \t]*(?P<quote>["'])(?P<value>[^"'\r\n]*)(?P=quote)''',
                    pin,
                ))
        object.__setattr__(self, "fields", tuple(fields))


@dataclass(frozen=True)
class CargoVersionUpdate:
    """An edit checked against the parsed keys it is allowed to change."""

    original: CargoVersionDocument
    identity: VersionIdentity
    text: str = field(init=False)

    def __post_init__(self) -> None:
        expected = tomllib.loads(self.original.source)
        updated = self.original.source
        for location in sorted(self.original.fields, key=lambda item: item.start, reverse=True):
            updated = updated[:location.start] + self.identity.version.value + updated[location.end:]
            parent = expected
            for key in location.key[:-1]:
                parent = parent[key]
            parent[location.key[-1]] = self.identity.version.value
        try:
            actual = tomllib.loads(updated)
        except tomllib.TOMLDecodeError as error:
            raise VersionIdentityError(f"version edit would invalidate Cargo.toml: {error}") from error
        if actual != expected:
            raise VersionIdentityError("version edit would change unrelated Cargo.toml fields")
        object.__setattr__(self, "text", updated)


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


@dataclass(frozen=True)
class PreparedFileWrite:
    """Bytes produced by admitted version-file preparation, with no later parsing."""

    path: Path
    contents: bytes


def unique_json_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    document: dict[str, object] = {}
    for key, value in pairs:
        if key in document:
            raise VersionIdentityError(f"duplicate JSON key {key!r}")
        document[key] = value
    return document


def reject_json_constant(value: str) -> None:
    raise VersionIdentityError(f"invalid JSON constant {value}")


@dataclass(frozen=True)
class JsonVersionUpdate:
    """An entirely admitted npm document; partial preparation cannot write it."""

    path: Path
    locations: tuple[tuple[str, ...], ...]
    version: AppVersion
    writes: tuple[PreparedFileWrite, ...] = field(init=False)
    drift: tuple[Drift, ...] = field(init=False)

    def __post_init__(self) -> None:
        try:
            document = json.loads(
                self.path.read_text(), object_pairs_hook=unique_json_object,
                parse_constant=reject_json_constant,
            )
        except (OSError, UnicodeError, json.JSONDecodeError) as error:
            raise VersionIdentityError(f"cannot read {self.path.name}: {error}") from error
        drift: list[Drift] = []
        for location in self.locations:
            parent = document
            for key in location[:-1]:
                if not isinstance(parent, dict) or key not in parent:
                    raise VersionIdentityError(f"{self.path.name}: missing object at {location!r}")
                parent = parent[key]
            key = location[-1]
            if not isinstance(parent, dict) or key not in parent or not isinstance(parent[key], str):
                raise VersionIdentityError(f"{self.path.name}: expected version string at {location!r}")
            have = AppVersion(parent[key])
            if have != self.version:
                drift.append(Drift(
                    f"{self.path.relative_to(REPO)} {location!r}  {have.value} -> {self.version.value}",
                    Fixability.AUTO,
                ))
                parent[key] = self.version.value
        writes = (PreparedFileWrite(
            self.path, (json.dumps(document, indent=2, ensure_ascii=False) + "\n").encode("utf-8"),
        ),) if drift else ()
        object.__setattr__(self, "writes", writes)
        object.__setattr__(self, "drift", tuple(drift))


def changelog_gaps(version: AppVersion) -> list[str]:
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
    required = headings | definitions | {version.value}

    gaps: list[str] = []
    for missing in sorted(required - headings):
        gaps.append(f"a `## [{missing}]` section")
    for missing in sorted(required - definitions):
        gaps.append(f"a `[{missing}]:` link reference")
    return gaps


@dataclass(frozen=True)
class PreparedVersionSync:
    """All version inputs and changelog observations admitted before any write."""

    original: CargoVersionDocument
    identity: VersionIdentity
    writes: tuple[PreparedFileWrite, ...] = field(init=False)
    drift: tuple[Drift, ...] = field(init=False)
    gaps: tuple[str, ...] = field(init=False)

    def __post_init__(self) -> None:
        cargo = CargoVersionUpdate(self.original, self.identity)
        drift: list[Drift] = []
        document = tomllib.loads(self.original.source)
        for location in self.original.fields:
            have = document
            for key in location.key:
                have = have[key]
            if have != self.identity.version.value:
                drift.append(Drift(
                    f"Cargo.toml {'.'.join(location.key)}  {have} -> {self.identity.version.value}",
                    Fixability.AUTO,
                ))
        writes = [PreparedFileWrite(CARGO, cargo.text.encode("utf-8"))] if drift else []
        for update in (
            JsonVersionUpdate(PACKAGE_JSON, (("version",),), self.identity.version),
            JsonVersionUpdate(PACKAGE_LOCK, (("version",), ("packages", "", "version")), self.identity.version),
        ):
            writes.extend(update.writes)
            drift.extend(update.drift)
        try:
            gaps = changelog_gaps(self.identity.version)
        except (OSError, UnicodeError) as error:
            raise VersionIdentityError(f"cannot read CHANGELOG.md: {error}") from error
        object.__setattr__(self, "writes", tuple(writes))
        object.__setattr__(self, "drift", tuple(drift))
        object.__setattr__(self, "gaps", tuple(gaps))


class VersionWriteError(ValueError):
    """A staging or publication failure, distinct from invalid input."""


@dataclass(frozen=True)
class StagedFileWrite:
    destination: Path
    replacement: Path


@dataclass(frozen=True)
class StagedVersionSync:
    """Complete, closed replacement files owned by the caller's cleanup scope."""

    writes: tuple[StagedFileWrite, ...] = field(init=False)

    def __init__(self, prepared: PreparedVersionSync, owner: ExitStack) -> None:
        staged: list[StagedFileWrite] = []
        for write in prepared.writes:
            try:
                metadata = write.path.lstat()
                if not stat.S_ISREG(metadata.st_mode):
                    raise VersionWriteError(f"cannot stage {write.path}: expected a regular file")
                directory = owner.enter_context(tempfile.TemporaryDirectory(
                    prefix=".version-sync-", dir=write.path.parent,
                ))
                replacement = Path(directory) / write.path.name
                # Closing the file must succeed before it enters the staged state.
                replacement.write_bytes(write.contents)
                replacement.chmod(stat.S_IMODE(metadata.st_mode))
            except OSError as error:
                raise VersionWriteError(f"cannot stage {write.path}: {error}") from error
            staged.append(StagedFileWrite(write.path, replacement))
        object.__setattr__(self, "writes", tuple(staged))

    def publish(self) -> None:
        for write in self.writes:
            try:
                write.replacement.replace(write.destination)
            except OSError as error:
                raise VersionWriteError(
                    f"cannot replace {write.destination}: {error}; "
                    "earlier replacements may have completed; inspect the version files before retrying"
                ) from error


def process(prepared: PreparedVersionSync, mode: SyncMode) -> int:
    identity = prepared.identity
    want = identity.version
    drift = list(prepared.drift)
    if mode in (SyncMode.FIX, SyncMode.BUMP):
        with ExitStack() as owner:
            StagedVersionSync(prepared, owner).publish()
    gaps = prepared.gaps
    if gaps:
        if mode is SyncMode.BUMP:
            # `--bump` runs BEFORE the human writes the section; the `--check`
            # gate (CI, and the tag script) still enforces it afterwards.
            print(f"next: write {' and '.join(gaps)} in CHANGELOG.md")
        else:
            # A changelog entry is human-written, so this is never auto-fixable.
            for gap in gaps:
                drift.append(Drift(f"CHANGELOG.md  missing {gap}", Fixability.MANUAL))

    if not drift:
        scope = f", tag {identity.release_tag} ok" if identity.release_tag is not None else ""
        print(f"app version in sync: {want.value}{scope}")
        return 0

    verb = "DRIFTED" if mode is SyncMode.CHECK else "fixed"
    print(f"{verb} ({len(drift)}): canonical version = {want.value}")
    for d in drift:
        print(f"  {d.message}")
    # Changelog prose still requires its author; admitted identity cannot drift here.
    unfixable = any(d.fixability is Fixability.MANUAL for d in drift)
    return 1 if (mode is SyncMode.CHECK or unfixable) else 0


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
    try:
        if args.bump is not None:
            mode = SyncMode.BUMP
            version = AppVersion(args.bump)
        else:
            # A bare invocation remains read-only.
            mode = SyncMode.FIX if args.fix else SyncMode.CHECK
        original = CargoVersionDocument.read(CARGO)
        if mode is not SyncMode.BUMP:
            version = original.version
        identity = VersionIdentity(version, args.release_tag)
        return process(PreparedVersionSync(original, identity), mode)
    except (VersionIdentityError, VersionWriteError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
