---
bump: minor
---

domain: add `EntityIds`/`Invalidation` and `invalidation_from_inverse` to derive the set of boards/columns/cards/sprints/graph/prefixes a captured inverse command batch touched, plus a pure `Command::touched_entities()` across every command leaf. `KanbanContext::execute_with` computes the derived invalidation on every batch (currently unused, laying groundwork for a future cache-invalidation consumer); no observable behaviour changes.
