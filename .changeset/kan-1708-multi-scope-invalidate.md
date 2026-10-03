---
bump: patch
---

domain: `Model::invalidate`'s exact card branch now drops every cached column scope that holds an invalidated card, not just the one the scoped card index points at, so a stale copy left in a moved card's old column cannot keep rendering after a narrowed invalidation. Takes effect in this release through KAN-1660, which makes card moves, creates and column-setting updates name their columns.
