---
bump: patch
---

propagate the JSON backend's swallowed load error through flush, delete the dead ended-sprint scan and the write-only SaveCoordinator file watcher, reuse the resolver's board list in MCP sprint carry-over, cap the global sprint and column NotFound enumerations, and narrow Model::set_cards_of_column to crate-private
