# Python development tools

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/scripts/README.md)

This directory is a locked uv project. Its standalone files retain their paths
because generation, release and mdBook recipes use them as executable inputs.
The project owns their interpreter, dependencies and development checks.

From the repository root:

```sh
uv sync --locked --project scripts --all-groups
uv run --locked --project scripts python scripts/sync-app-version.py --check
just python-tools-check
```

The Just, release and mdBook callers use this environment. Child processes
using `sys.executable` inherit it. CI installs uv and runs the same Ruff,
strict mypy and pytest checks. These checks do not generate parser artifacts,
publish a release or replace the release gate.

The checks include the actual book command and an offline Cargo dependency
boundary. Use a full-history checkout for Git-derived book dates. The recipe
fetches the spec workspace's locked Cargo metadata before the offline control;
it does not build the Rust targets. CI provisions the declared Rust toolchain
and full history rather than relying on another job's cache.

Coverage exports and source-range inventories are admitted through typed wire
schemas. A missing function identity or truncated region is rejected before
coverage attribution; source-bound byte ranges retain their digest checks.
