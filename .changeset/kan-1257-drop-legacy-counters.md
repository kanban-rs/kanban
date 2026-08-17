---
bump: minor
---

Card and sprint numbering is now held solely by the shared prefix rows. The per-board counters that preceded them are removed from storage: the SQLite `boards.card_counter` column and `board_sprint_counters` table (schema 12) and the JSON `card_counter` / `sprint_counters` board keys (format V17). Existing files upgrade on open, after the prefix rows are seeded from the old counters, so numbering continues without a gap.

Also fixes a related loss on the `kanban migrate <json> sqlite` path, where a full-snapshot write omitted the prefix rows entirely and left every namespace restarting at 1.
