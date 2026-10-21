---
bump: patch
---

Restructure the release workflow into fail-fast, per-leg re-runnable jobs. A new mutation-free preflight job classifies each run as release, resume, or noop and probes every release credential (deploy key, crates.io token, AUR key, Homebrew tap key, winget token, Chocolatey key) in its own step before anything is pushed, so a dead credential now fails the run before the version bump instead of after it. The monolithic release job is split into prepare, publish-crates, tag-release, publish-aur, publish-homebrew, and sync-develop, each idempotent and re-runnable on its own, and resume mode lets a full re-run finish a release that died after the master push (previously a silent no-op once the changesets were consumed). build-windows now depends on tag-release instead of racing it, and resolve-version keeps the workflow_dispatch recovery path for the Windows legs unchanged.
