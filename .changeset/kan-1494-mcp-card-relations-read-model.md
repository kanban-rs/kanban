---
bump: patch
---

mcp: retarget the four card_relations tools (set/remove card parent, list card parents/children) at the call-scoped Model instead of the raw KanbanContext. Card identifier resolution now goes through helpers::model_read::resolve_card, and the parent/child edge read goes through Model::graph_state() via require_loaded instead of ctx.list_parents_of/list_children_of. A NotLoaded or Failed card list or dependency graph tier now surfaces as an error naming the collection instead of the tool reporting a false "not found" or silently returning an empty relation list.
