---
bump: patch
---

Fixes card numbering being lost through `kanban export --board <X>` and `kanban import`. The export carried the board's cards but not the counters that say which numbers are taken, so the next card created after an import re-used a number already on an imported card. Single-board exports now carry the counters for the namespaces they address, and import merges them by taking the higher of the two so a destination that is already further ahead is never rolled backwards. Files exported by earlier versions carry no counters at all; importing one reconstructs them from the highest number on the imported cards. Exporting a workspace to SQLite from Settings now carries the counters too, which it previously dropped.
