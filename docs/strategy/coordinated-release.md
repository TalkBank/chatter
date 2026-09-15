# Coordinated release publication

**Last modified:** 2026-09-15 12:06 EDT

The cargo-dist configuration places GitHub publication in the final announce
phase. Its required reusable desktop job creates an unpublished candidate,
builds signed/notarized macOS installers and signed updater bundles, uploads
desktop assets, and checks the updater's version, platform set and asset
references. CLI artifact builds must already have succeeded before this job
runs. Only then does cargo-dist upload its CLI assets and publish the draft.

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

Local verification includes the candidate-verifier tests and workflow syntax
validation. A real authorized release rehearsal is still needed to verify
GitHub draft/asset behavior, signing/notarization credentials, installation,
updating, and the public download endpoints. Local tests do not certify these
external boundaries.
