---
bump: patch
---

domain: state and test that a prefix row is append-only - changing a board's card prefix starts numbering afresh in the new namespace and leaves the old one, and every identifier already minted from it, untouched. No behaviour change; this pins what already happens.
