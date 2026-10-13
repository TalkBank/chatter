"""Regenerate both traversal artifacts from the reviewed, locked TSGU source."""

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from contextlib import ExitStack
from dataclasses import dataclass
from pathlib import Path

import tomllib
from generate_if_changed import publish


class RegenerationError(Exception):
    """A source, build or generated-output admission failed."""


@dataclass(frozen=True, init=False)
class ConsumerInputs:
    """Validated consumer-owned source pin and formatting configuration."""

    root: Path
    revision: str
    edition: str
    toolchain: str

    def __init__(self, root: Path) -> None:
        object.__setattr__(self, "root", root.resolve())
        object.__setattr__(
            self, "revision", (self.root / "tsgu-revision").read_text().strip()
        )
        if re.fullmatch(r"[0-9a-f]{40}", self.revision) is None:
            raise RegenerationError(
                "tsgu-revision must contain a full lowercase Git commit"
            )
        manifest = tomllib.loads((self.root / "Cargo.toml").read_text())
        toolchain = tomllib.loads((self.root / "rust-toolchain.toml").read_text())
        object.__setattr__(self, "edition", manifest["workspace"]["package"]["edition"])
        object.__setattr__(self, "toolchain", toolchain["toolchain"]["channel"])
        if self.edition not in {"2015", "2018", "2021", "2024"}:
            raise RegenerationError("workspace edition is not supported by rustfmt")
        if not isinstance(self.toolchain, str) or not self.toolchain:
            raise RegenerationError("rust-toolchain.toml needs a nonempty channel")


@dataclass(frozen=True, init=False)
class PinnedCheckout:
    """A clean repository root admitted against the consumer's full revision."""

    directory: Path
    revision: str
    description: str

    def __init__(self, directory: Path, inputs: ConsumerInputs) -> None:
        object.__setattr__(self, "directory", directory.resolve())
        object.__setattr__(self, "revision", inputs.revision)
        self.revalidate()
        object.__setattr__(
            self,
            "description",
            self.git("describe", "--always", "--dirty", "--abbrev=12"),
        )

    def git(self, *arguments: str) -> str:
        return subprocess.run(
            ["git", "-C", str(self.directory), *arguments],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()

    def revalidate(self) -> None:
        if Path(self.git("rev-parse", "--show-toplevel")).resolve() != self.directory:
            raise RegenerationError("TSGU_DIR must name the generator repository root")
        actual = self.git("rev-parse", "HEAD")
        if actual != self.revision:
            raise RegenerationError(
                f"generator revision {actual} differs from tsgu-revision {self.revision}"
            )
        if self.git(
            "status",
            "--porcelain",
            "--untracked-files=normal",
            "--ignore-submodules=none",
        ):
            raise RegenerationError("generator checkout has uncommitted changes")
        if not (self.directory / "Cargo.lock").is_file():
            raise RegenerationError("generator checkout needs its committed Cargo.lock")


@dataclass(frozen=True, init=False)
class BuiltGenerator:
    """The executable reported by a successful build of an admitted checkout."""

    checkout: PinnedCheckout
    executable: Path

    def __init__(self, checkout: PinnedCheckout) -> None:
        checkout.revalidate()
        result = subprocess.run(
            [
                "cargo",
                "build",
                "--locked",
                "--release",
                "--message-format=json",
                "--example",
                "generate_typed_traversal",
                "-p",
                "tree-sitter-node-types",
            ],
            cwd=checkout.directory,
            check=True,
            stdout=subprocess.PIPE,
            text=True,
        )
        candidates = []
        for line in result.stdout.splitlines():
            message = json.loads(line)
            if (
                message.get("reason") == "compiler-artifact"
                and message.get("target", {}).get("name") == "generate_typed_traversal"
                and "example" in message.get("target", {}).get("kind", [])
                and message.get("executable") is not None
            ):
                candidates.append(Path(message["executable"]))
        if len(candidates) != 1 or not candidates[0].is_file():
            raise RegenerationError(
                "Cargo did not report exactly one generator executable"
            )
        checkout.revalidate()
        object.__setattr__(self, "checkout", checkout)
        object.__setattr__(self, "executable", candidates[0])

    def regenerate(self, inputs: ConsumerInputs) -> None:
        """Finish both children before publication; unchanged files retain mtimes."""
        self.checkout.revalidate()
        if self.checkout.revision != inputs.revision:
            raise RegenerationError(
                "built generator belongs to a different consumer pin"
            )
        outputs = (
            (
                inputs.root / "crates/talkbank-parser/src/generated_traversal.rs",
                [
                    str(inputs.root / "grammar/src/grammar.json"),
                    str(inputs.root / "grammar/src/node-types.json"),
                    "--edition",
                    inputs.edition,
                    "--toolchain",
                    inputs.toolchain,
                ],
            ),
            (
                inputs.root / "grammar/bindings/rust/tsgu_language_metadata.c",
                ["--metadata-bridge"],
            ),
        )
        with ExitStack() as cleanup:
            staged = []
            for output, arguments in outputs:
                stream = cleanup.enter_context(
                    tempfile.NamedTemporaryFile(
                        dir=output.parent,
                        prefix=f".{output.name}.",
                        delete=False,
                    )
                )
                temporary = Path(stream.name)
                cleanup.callback(temporary.unlink, missing_ok=True)
                subprocess.run(
                    [str(self.executable), *arguments],
                    cwd=inputs.root,
                    check=True,
                    stdout=stream,
                )
                stream.flush()
                stream.close()
                staged.append((temporary, output))
            with staged[0][0].open(encoding="utf-8") as traversal:
                header = traversal.readline().rstrip("\r\n")
            if not header.endswith(f", source {self.checkout.description}"):
                raise RegenerationError(
                    "generated header does not identify the admitted source"
                )
            self.checkout.revalidate()
            for temporary, output in staged:
                publish(temporary, output)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--generator",
        type=Path,
        default=Path(os.environ.get("TSGU_DIR", str(Path.home() / "tsgu"))),
        help="clean generator checkout (default: TSGU_DIR or ~/tsgu)",
    )
    args = parser.parse_args()
    try:
        inputs = ConsumerInputs(Path(__file__).resolve().parent.parent)
        generator = BuiltGenerator(PinnedCheckout(args.generator, inputs))
        print(f"generator source {generator.checkout.revision}: {generator.executable}")
        generator.regenerate(inputs)
    except subprocess.CalledProcessError as error:
        if error.stderr:
            print(error.stderr, file=sys.stderr, end="")
        print(f"traversal regeneration failed: {error}", file=sys.stderr)
        return error.returncode if error.returncode > 0 else 128 - error.returncode
    except (OSError, ValueError, KeyError, TypeError, RegenerationError) as error:
        print(f"traversal regeneration failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
