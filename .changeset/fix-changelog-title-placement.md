---
bump: patch
---

Fix the release tooling that had been slowly corrupting CHANGELOG.md. `aggregate-changelog` prepended each new release above the entire existing file, including the `# Changelog` title, so the title sank one section per release and had ended up near the bottom after 35 releases. The script now inserts each new version after the title and preamble, demotes any markdown headings inside a changeset body so they cannot collide with the version and entry header levels, and preserves paragraph breaks in multi-paragraph changeset descriptions instead of collapsing them into a single block. CHANGELOG.md itself is repaired in the same change: the title is restored to the top and the stray body-level headings are demoted. No release history was lost.
