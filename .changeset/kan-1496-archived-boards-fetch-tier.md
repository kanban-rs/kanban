---
bump: minor
---

domain,service: add an archived-boards fetch tier mirroring the archived-card tier: `FetchRound.archived_board_list`, `Resolved.archived_boards`, a resolve arm over `DataStore::list_archived_boards`, and an `apply_resolved` path that feeds `Model::archived_boards`/`archived_board_ids` on load and records a Failed read without disturbing previously loaded markers. Also applies the flat `resolved.archived_cards.all` tier into the Model, which was previously fetched but silently dropped, and removes the permissive `Ok(Vec::new())` default on `DataStore::list_archived_boards`, making every backend implement it explicitly.
