---
bump: patch
---

tui,view,domain,service,persistence: remove twenty dead `pub fn`s and the unused `PersistenceEvent` enum, found by a whole-tree identifier frequency sweep. Includes four domain mutators (`Card::set_points`, `Column::set_wip_limit`, `Column::update_position`, `Sprint::update_name_index`) whose backing fields are public and written directly by every caller instead. No behavior change; the existing test suite is the regression pin.
