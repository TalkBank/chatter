"""For every Conflicting row in the parity baseline, print what each backend says.

The parity gate tells you THAT a spec case diverges and in which shape; it does
not tell you what each side actually reports, which is what decides the work.
This runs the built binary over every Conflicting row's own example with each
backend and prints `row<TAB>ts=CODES<TAB>re2c=CODES`.

Usage, from the chatter checkout, with a RELEASE binary already built:

    python3 scripts/analysis/map_parity_conflicts.py <scratch-dir>

Written 2026-08-27 while working the Conflicting family. It is what showed that
20 of those rows are re2c answering the generic E321 where tree-sitter names a
rule, and that one row (E208) has re2c reporting the MORE specific code, which
is a maintainer adjudication rather than a bug to fix unilaterally.
"""
import re, subprocess, tomllib, sys
from pathlib import Path

# Derived from this file's location, so the script works in any checkout.
root = Path(__file__).resolve().parent.parent.parent
binary = root / 'target/release/chatter'
base = Path(sys.argv[1])
rows = re.findall(r'\("([^"]+)",\s*Conflicting\)', (root / 'crates/talkbank-parser-re2c/tests/integration/error_parity/baseline.rs').read_text())

def codes(path, extra):
    out = subprocess.run([str(binary), 'validate', '--force', *extra, str(path)],
                         capture_output=True, text=True, timeout=60)
    return sorted(set(re.findall(r'error\[(E\d+)\]', out.stdout + out.stderr)))

for row in rows:
    name, _, idx = row.partition('#')
    idx = int(idx) if idx else 0
    spec = root / 'spec/errors' / name
    if not spec.is_file():
        print(f"{row}\tMISSING SPEC"); continue
    try:
        d = tomllib.loads(spec.read_text().split('+++')[1])
    except Exception as e:
        print(f"{row}\tUNPARSED {e}"); continue
    ex = d.get('example', [])
    if idx >= len(ex):
        print(f"{row}\tNO SUCH EXAMPLE"); continue
    f = base / f"{row.replace('#','_').replace('.md','')}.cha"
    f.write_text(ex[idx]['chat'])
    print(f"{row}\tts={','.join(codes(f, []))}\tre2c={','.join(codes(f, ['--parser=re2c']))}")
