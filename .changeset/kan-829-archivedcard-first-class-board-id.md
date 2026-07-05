---
bump: patch
---

Internal domain change with no user-visible behaviour yet. `ArchivedCard` now carries its own `board_id` as a first-class field (D2), so board scoping is a direct lookup rather than loading every column and post-filtering. The archive command captures the board from the card's column best-effort, tolerating a dangling column by recording a nil board id rather than failing the archive. The field is additive with `#[serde(default)]`, so existing snapshots still load (nil until the persistence migration backfills a correct board id in a later slice).
