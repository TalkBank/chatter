# Coordinated release publication

**Last modified:** [Git history](https://github.com/TalkBank/chatter/commits/main/docs/strategy/coordinated-release.md)

The cargo-dist configuration places GitHub publication in the final announce
phase. Its required reusable desktop job creates an unpublished candidate,
builds signed/notarized macOS installers and signed updater bundles, uploads
desktop assets, and checks the updater's version, platform set and asset
references. CLI artifact builds must already have succeeded before this job
runs. Only then does cargo-dist upload its CLI assets and publish the draft.

### Built inputs are not uploaded assets

The global CLI artifact from the same workflow run owns the current installer
pair. Before announcement, those files are **built inputs**, not GitHub Release
assets: the generated announcer owns their single upload. The desktop job
downloads that Actions artifact, admits its exact release identity, derives
legacy installer wrappers, uploads only those wrappers, then downloads and
compares their bytes with the producer-derived expectations.

`VerifiedInstallerAliases` is constructible only from the complete verified
pair and retains its release tag. The pre-announcement verifier requires this
owner, uploaded alias sizes, all desktop installers and valid updater references.
It refuses originals already uploaded prematurely, preventing a later duplicate
upload. It neither invents uploaded-asset metadata nor permits a flag to skip
installer evidence. The normal announcer subsequently uploads the original
CLI/LSP artifacts and publishes; it does not overwrite the alias assets.

Tag pushes dispatch the same `release.yml` workflow used for manual releases.
For a manual release, select the tag as the workflow ref and give that same tag
as the input. Desktop publication verifies that the tag resolves to the exact
checkout. Direct manual dispatch of `release-desktop.yml` remains build-only.

Missing desktop jobs, failed builds, missing installer aliases, missing updater
bundles/signatures, or a mismatched updater version prevent publication.
Prereleases also require desktop completion. Published releases are refused as
build targets. Uploads do not overwrite existing assets: a partial draft is
retained for explicit recovery, not automatically replaced. The previous
complete public latest release is not deliberately modified during a build.

The website should use stable asset aliases through `releases/latest/download`
once the currently published release is complete. No page version edit is
needed for subsequent complete releases. An incomplete already-published release
is a separate recovery action; changing workflows does not repair it retroactively.

### Recovering an unpublished partial draft

Preserve the version tag, successful builds and existing draft assets. First
repair a process defect in its maintained scripts, gate and instructions; blindly
rerunning an immutable tag repeats the old workflow. A recovery is separately
reviewed publication work, not proof that the failed workflow passed.

Bind retained Actions artifacts to the exact tag commit and inspect producer
checksums and successful signing/notarization jobs. Derive installer aliases with
the producer plan, never by copying another release's installers. Inventory
existing assets and upload only missing files. If an existing filename has
different bytes, stop for explicit recovery; do not use blanket `--clobber`.
Download actual alias bytes for compatibility verification and obtain fresh draft
metadata and its updater manifest. The strict full-population check is:

```sh
node scripts/release/verify-desktop-candidate.mjs complete \
  release.json latest.json vX.Y.Z owner/repository plan-dist-manifest.json
```

This admits every CLI/LSP artifact and checksum declared by the producer in
addition to desktop/update assets and both installer-name pairs. It is inventory
admission, not provenance or cryptographic certification. Never synthesize an
uploaded-asset inventory to make it pass. Publish only after these independent
checks succeed; verify public downloads, updater references and installation
before announcing completion. Keep failed-run and recovery receipts distinct.

Local verification includes all `scripts/release/*.test.mjs` contracts and workflow
syntax validation. The publication-order test executes the actual preparation,
verification and generated announcement shell blocks against a stateful transport:
an empty draft cannot provide pending originals, duplicate uploads fail, and
corrupt alias downloads cannot authorize publication. These run through
`just release-verifier-test` in both the final gate and CI. A real authorized
release remains necessary to verify
GitHub draft/asset behavior, signing/notarization credentials, installation,
updating, and the public download endpoints. Local tests do not certify these
external boundaries.
