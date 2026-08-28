---
bump: patch
---

Repoint the release pipeline at the kanban-rs org after the repo move: the release workflow's push remote, the tarball SHA URLs for the AUR and Homebrew bumps, the aur-publish repository guard and tarball URL, the AUR PKGBUILD/.SRCINFO source URLs, and the Chocolatey install URL and nuspec project URLs now reference kanban-rs/kanban. The Homebrew tap repo and the winget identifier and fork-user deliberately stay under fulsomenko. The "Sync develop with master" step now fails loudly (exit 1) on a merge conflict instead of warning and exiting 0, so a conflicted develop can no longer pass silently.
