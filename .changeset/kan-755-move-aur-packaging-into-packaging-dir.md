---
bump: patch
---

Repository housekeeping with no user-visible behaviour change. The AUR package sources (`PKGBUILD`, `.SRCINFO`) moved from the top-level `aur/` directory into `packaging/aur/`, making them a sibling of the existing `packaging/chocolatey/` sources. The release workflows were updated to the new path, and `packaging/` now carries an index README plus a per-package README for the AUR sources.
