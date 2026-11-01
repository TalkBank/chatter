"""Admit a generator's source checkout against a full commit pinned in this repo.

Both generated-artifact producers that Chatter does not vendor, the traversal
generator (`tsgu-revision`) and the Tree-sitter build that writes the grammar
contract (`tree-sitter-revision`), are pinned by identity, never by location:
the pin is a full commit, and the checkout is a working copy an environment
variable selects, with no default. A checkout is admitted only at the pinned
commit, clean, at its repository root, with its committed `Cargo.lock`, and it
is revalidated before and after every build and generation step, so an edit
made while a step ran cannot slip into a published artifact.

The steps are typed: a `Pin` reads its `PinnedRevision` (which only a pin can
make, so one pin's commit cannot be handed to another pin's checkout), the
revision admits a `PinnedCheckout`, and only an admitted checkout can build
an executable.
"""

import json
import os
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path


class PinError(Exception):
    """A pin or the checkout it names cannot be admitted."""


class BuildError(Exception):
    """An admitted checkout's build did not report the one executable asked for."""


@dataclass(frozen=True)
class Pin:
    """One pinned producer: the file holding its commit, and the variable
    naming its checkout."""

    file: str
    variable: str

    def revision(self, root: Path) -> "PinnedRevision":
        """The full lowercase commit this repository pins."""
        commit = (root / self.file).read_text().strip()
        if re.fullmatch(r"[0-9a-f]{40}", commit) is None:
            raise PinError(f"{self.file} must contain a full lowercase Git commit")
        return PinnedRevision(self, commit, _token=_PIN_ONLY)

    def checkout(self, root: Path) -> "PinnedCheckout":
        """Admit the checkout this pin's variable names, at the commit `root`
        pins. An unset variable is refused: there is no default location."""
        selected = os.environ.get(self.variable)
        if not selected:
            raise PinError(
                f"{self.variable} must name a clean checkout at the commit {self.file} pins"
            )
        return PinnedCheckout(Path(selected), self.revision(root))


#: Only `Pin.revision` holds this, so only it makes a `PinnedRevision`.
_PIN_ONLY = object()


@dataclass(frozen=True)
class PinnedRevision:
    """A commit read from its pin's file, carrying the pin it came from."""

    pin: Pin
    commit: str
    _token: object

    def __post_init__(self) -> None:
        if self._token is not _PIN_ONLY:
            raise TypeError("a PinnedRevision is made only by Pin.revision")


@dataclass(frozen=True, init=False)
class PinnedCheckout:
    """A clean repository root admitted against a pinned revision."""

    directory: Path
    revision: PinnedRevision

    def __init__(self, directory: Path, revision: PinnedRevision) -> None:
        object.__setattr__(self, "directory", directory.resolve())
        object.__setattr__(self, "revision", revision)
        self.revalidate()

    def git(self, *arguments: str) -> str:
        return subprocess.run(
            ["git", "-C", str(self.directory), *arguments],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()

    def describe(self) -> str:
        """The source identity a generator stamps into its output."""
        return self.git("describe", "--always", "--dirty", "--abbrev=12")

    def revalidate(self) -> None:
        pin = self.revision.pin
        if Path(self.git("rev-parse", "--show-toplevel")).resolve() != self.directory:
            raise PinError(f"{pin.variable} must name the repository root")
        actual = self.git("rev-parse", "HEAD")
        if actual != self.revision.commit:
            raise PinError(
                f"checkout revision {actual} differs from {pin.file} {self.revision.commit}"
            )
        if self.git(
            "status",
            "--porcelain",
            "--untracked-files=normal",
            "--ignore-submodules=none",
        ):
            raise PinError(f"the {pin.variable} checkout has uncommitted changes")
        if not (self.directory / "Cargo.lock").is_file():
            raise PinError(f"the {pin.variable} checkout needs its committed Cargo.lock")

    def build_executable(self, cargo_arguments: list[str], name: str, kind: str) -> Path:
        """Build with the committed lockfile and return the one executable
        Cargo reports for target `name` of kind `kind` (`bin`, `example`).

        The executable is Cargo's report, never an assumed path, so a target
        directory configured anywhere cannot make an old binary run. The
        checkout is revalidated before and after the build.
        """
        self.revalidate()
        result = subprocess.run(
            ["cargo", "build", "--locked", "--release", "--message-format=json", *cargo_arguments],
            cwd=self.directory,
            check=True,
            stdout=subprocess.PIPE,
            text=True,
        )
        candidates = []
        for line in result.stdout.splitlines():
            message = json.loads(line)
            target = message.get("target", {})
            if (
                message.get("reason") == "compiler-artifact"
                and target.get("name") == name
                and kind in target.get("kind", [])
                and message.get("executable") is not None
            ):
                candidates.append(Path(message["executable"]))
        if len(candidates) != 1 or not candidates[0].is_file():
            raise BuildError(f"Cargo did not report exactly one {name} executable")
        self.revalidate()
        return candidates[0]
