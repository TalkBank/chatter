#!/usr/bin/env python3
"""Stage generator stdout; replace its target only on success and byte changes."""

import argparse
import filecmp
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile


def generate(output: Path, command: list[str]) -> None:
    """A failed child never overwrites the target; identical bytes retain mtime."""
    with tempfile.NamedTemporaryFile(
        dir=output.parent, prefix=f".{output.name}.", delete=False
    ) as staged:
        temporary = Path(staged.name)
        try:
            subprocess.run(command, stdout=staged, check=True)
            staged.flush()
            publish(temporary, output)
        finally:
            temporary.unlink(missing_ok=True)


def publish(staged: Path, output: Path) -> None:
    """Publish completed output on the same filesystem, preserving equal files."""
    if output.exists() and filecmp.cmp(staged, output, shallow=False):
        return
    mode = stat.S_IMODE(output.stat().st_mode) if output.exists() else 0o644
    os.chmod(staged, mode)
    os.replace(staged, output)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if not args.command:
        parser.error("a generator command is required")
    try:
        generate(args.output, args.command)
    except subprocess.CalledProcessError as error:
        return error.returncode if error.returncode > 0 else 128 - error.returncode
    except OSError as error:
        print(f"generation failed for {args.output}: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
