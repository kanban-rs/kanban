# winget manifest reference set

This directory holds a reference/fallback copy of the `fulsomenko.kanban`
winget manifest. It is **not** the canonical source: winget manifests
cannot live in this repo the way the AUR `PKGBUILD` or the Chocolatey
nuspec do — the single source of truth for a real release is the
per-version manifest folder merged into `microsoft/winget-pkgs`.

The `publish-winget` job in `.github/workflows/release.yml` generates
and submits that real manifest on every release, via
[`vedantmgoyal9/winget-releaser`](https://github.com/vedantmgoyal9/winget-releaser)
(`komac` under the hood). It downloads the release's Windows zip,
computes the SHA256, and opens a PR against `microsoft/winget-pkgs`
from the `fulsomenko/winget-pkgs` fork.

The files under `manifests/f/fulsomenko/kanban/0.0.0/` exist so that:

- `winget validate` can be run against a well-formed example locally
  or in CI, independent of any real release.
- A maintainer has a manual fallback to hand-submit with
  `wingetcreate`/`komac` if the automated job is ever unavailable.

`0.0.0` and the installer's SHA256 are placeholders — they are never
published as-is. Do not bump this version by hand; the action
generates the real per-version manifest directly in the upstream PR.

Both binaries the Windows release zip ships (`kanban.exe` and
`kanban-mcp.exe`, staged flat at the zip root — see `build-windows` in
`.github/workflows/release.yml`) are declared as portable nested
installer files so both land on `PATH` after `winget install`.
