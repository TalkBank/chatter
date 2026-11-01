#!/usr/bin/env python3
"""Generate grammar artifacts in staging, then check or publish changed files.

The producer is the Tree-sitter CLI built from a clean checkout of the commit
`tree-sitter-revision` pins, selected by `TREE_SITTER_DIR`. It must be a build
that writes `grammar-contract.json`: the traversal generator reads only that
file, so an upstream CLI, which writes everything else, would leave the
contract silently stale. It is therefore a required output, and the producer
is never chosen by `PATH` order.
"""

import argparse
import enum
import filecmp
import subprocess
import sys
import tempfile
from pathlib import Path

from generate_if_changed import exit_status, publish
from pinned_checkout import BuildError, Pin, PinError

ROOT = Path(__file__).resolve().parents[1]
GRAMMAR = ROOT / "grammar"

#: The contract-writing Tree-sitter build, pinned by `tree-sitter-revision` and
#: selected by `TREE_SITTER_DIR`.
TREE_SITTER = Pin(file="tree-sitter-revision", variable="TREE_SITTER_DIR")

#: The grammar contract, the typed traversal generator's one input.
CONTRACT = "grammar-contract.json"

#: Every file the producer must write; a missing one fails closed.
REQUIRED = ("parser.c", "grammar.json", "node-types.json", CONTRACT)


class Mode(enum.Enum):
    """What to do with the staged files."""

    #: Report a stale or missing file without writing anything.
    CHECK = "check"
    #: Replace each changed file; identical files keep their modification time.
    PUBLISH = "publish"


def stage_and_publish(executable: Path, mode: Mode) -> int:
    """Generate into staging, then check or publish. Returns the exit status."""
    with tempfile.TemporaryDirectory(dir=GRAMMAR, prefix=".generated-") as directory:
        staged = Path(directory)
        result = subprocess.run(
            [str(executable), "generate", "--grammar-contract", "--output", str(staged)],
            cwd=GRAMMAR,
            check=False,
        )
        if result.returncode:
            return exit_status(result.returncode)
        for required in REQUIRED:
            if not (staged / required).is_file():
                print(f"generator omitted {required}", file=sys.stderr)
                return 1
        stale = False
        for source in sorted(path for path in staged.rglob("*") if path.is_file()):
            relative = source.relative_to(staged)
            target = GRAMMAR / "src" / relative
            match mode:
                case Mode.CHECK:
                    if not target.is_file() or not filecmp.cmp(source, target, shallow=False):
                        print(f"stale grammar/src/{relative}; run just regen", file=sys.stderr)
                        stale = True
                case Mode.PUBLISH:
                    target.parent.mkdir(parents=True, exist_ok=True)
                    publish(source, target)
        return int(stale)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate grammar artifacts in staging, then check or publish changed files."
    )
    parser.add_argument("--check", action="store_true", help="report staleness without writing")
    args = parser.parse_args()
    mode = Mode.CHECK if args.check else Mode.PUBLISH
    try:
        checkout = TREE_SITTER.checkout(ROOT)
        executable = checkout.build_executable(["-p", "tree-sitter-cli"], "tree-sitter", "bin")
        print(f"producer source {checkout.revision.commit}: {executable}")
        status = stage_and_publish(executable, mode)
        checkout.revalidate()
        return status
    except subprocess.CalledProcessError as error:
        print(f"grammar generation failed: {error}", file=sys.stderr)
        return exit_status(error.returncode)
    except (OSError, ValueError, PinError, BuildError) as error:
        print(f"grammar generation failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
