---
bump: patch
---

Group each release's non-card changesets under a single "Other Changes" heading in the changelog instead of emitting one repeated "### Other Changes" header per changeset, and recognize a card ID (e.g. kan-1046) anywhere in a changeset or branch name rather than only at the start. Conventional-commit-style names like `feat/kan-1046-...` now attribute the change to its card instead of filing it under "Other Changes". The existing repeated headers in CHANGELOG.md are collapsed the same way.
