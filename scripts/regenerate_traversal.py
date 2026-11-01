"""Regenerate both traversal artifacts from the reviewed, locked TSGU source."""

import subprocess
import sys
import tempfile
from contextlib import ExitStack
from dataclasses import dataclass
from pathlib import Path

import tomllib
from generate_grammar import CONTRACT
from generate_if_changed import exit_status, publish
from pinned_checkout import BuildError, Pin, PinError, PinnedCheckout

#: The traversal generator, pinned by `tsgu-revision` and selected by `TSGU_DIR`.
TSGU = Pin(file="tsgu-revision", variable="TSGU_DIR")


class RegenerationError(Exception):
    """A source, build or generated-output admission failed."""


@dataclass(frozen=True, init=False)
class ConsumerInputs:
    """Validated consumer-owned formatting configuration."""

    root: Path
    edition: str
    toolchain: str

    def __init__(self, root: Path) -> None:
        object.__setattr__(self, "root", root.resolve())
        manifest = tomllib.loads((self.root / "Cargo.toml").read_text())
        toolchain = tomllib.loads((self.root / "rust-toolchain.toml").read_text())
        object.__setattr__(self, "edition", manifest["workspace"]["package"]["edition"])
        object.__setattr__(self, "toolchain", toolchain["toolchain"]["channel"])
        if self.edition not in {"2015", "2018", "2021", "2024"}:
            raise RegenerationError("workspace edition is not supported by rustfmt")
        if not isinstance(self.toolchain, str) or not self.toolchain:
            raise RegenerationError("rust-toolchain.toml needs a nonempty channel")


@dataclass(frozen=True, init=False)
class BuiltGenerator:
    """The executable reported by a successful build of an admitted checkout."""

    checkout: PinnedCheckout
    executable: Path

    def __init__(self, checkout: PinnedCheckout) -> None:
        object.__setattr__(self, "checkout", checkout)
        object.__setattr__(
            self,
            "executable",
            checkout.build_executable(
                ["--example", "generate_typed_traversal", "-p", "tree-sitter-node-types"],
                "generate_typed_traversal",
                "example",
            ),
        )

    def regenerate(self, inputs: ConsumerInputs) -> None:
        """Finish both children before publication; unchanged files retain mtimes."""
        self.checkout.revalidate()
        outputs = (
            (
                inputs.root / "crates/talkbank-parser/src/generated_traversal.rs",
                [
                    str(inputs.root / "grammar/src" / CONTRACT),
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
            if not header.endswith(f", source {self.checkout.describe()}"):
                raise RegenerationError(
                    "generated header does not identify the admitted source"
                )
            self.checkout.revalidate()
            for temporary, output in staged:
                publish(temporary, output)


def main() -> int:
    try:
        inputs = ConsumerInputs(Path(__file__).resolve().parent.parent)
        generator = BuiltGenerator(TSGU.checkout(inputs.root))
        print(f"generator source {generator.checkout.revision.commit}: {generator.executable}")
        generator.regenerate(inputs)
    except subprocess.CalledProcessError as error:
        if error.stderr:
            print(error.stderr, file=sys.stderr, end="")
        print(f"traversal regeneration failed: {error}", file=sys.stderr)
        return exit_status(error.returncode)
    except (OSError, ValueError, KeyError, TypeError, PinError, BuildError, RegenerationError) as error:
        print(f"traversal regeneration failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
