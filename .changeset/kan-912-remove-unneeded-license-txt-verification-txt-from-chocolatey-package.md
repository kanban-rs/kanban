---
bump: patch
---

Repository housekeeping with no user-visible behaviour change. The Chocolatey package source (`packaging/chocolatey/tools/`) no longer ships `LICENSE.txt` or `VERIFICATION.txt` — Chocolatey moderation flagged both as unnecessary for a download-only package with no embedded binary payload, since `chocolateyinstall.ps1` already fetches the release ZIP with an inline SHA256 checksum. The release workflow and the packaging README were updated to match.
