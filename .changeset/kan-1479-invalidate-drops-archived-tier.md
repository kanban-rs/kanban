---
bump: minor
---

domain,service: `Model::invalidate` now drops the board-scoped `archived_cards_by_board` tier alongside the sibling `columns_by_board`/`sprints_by_board`/`cards_by_column` tiers, since a `cards` or `boards` id in `EntityIds` previously left it stale and could serve a just-restored card as still archived until the whole model was reset. `kanban_service::test_helpers::contract::cache` gains a new public `ArchivedByBoardPlan` and two contract functions, registered in `cache_contract_tests!`, that hold the fix and the board-scoping of `list_archived_cards_by_board` to one spec across the in-memory, JSON and SQLite backends. This is `minor`, not `patch`, because it adds new public surface (`ArchivedByBoardPlan`, two `pub async fn` contract functions, two macro arms) on a crate published to crates.io, following the precedent set by KAN-1426.
