---
bump: patch
---

Added a `workflow_dispatch` entry point to the release workflow so
`build-windows` and `publish-chocolatey` can be re-run for an
already-published version. Previously they were gated on the changesets
found during the original PR-merge run; once that run consumed and
committed the changesets, no rerun could satisfy the gate again, leaving
no recovery path if either job was skipped by an earlier job's failure
(as happened during the v0.8.0 release, where AUR's own maintenance
outage caused these two jobs to be skipped).
