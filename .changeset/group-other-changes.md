---
bump: patch
---

Group each release's non-card changesets under a single "Other Changes" heading in the changelog instead of emitting one repeated "### Other Changes" header per changeset. The aggregate-changelog script now collects them into one section, and the existing repeated headers in CHANGELOG.md are collapsed the same way.
