---
bump: minor
---

domain,service: `Model` gains a board-scoped archived-card marker tier
(`board_archived_cards_state`, `archived_cards_state`), applied from a
resolve pass's `archived_cards.by_parent`, cleared on `load_from_snapshot`
and marked `Failed` alongside the rest of the card tier by `mark_failed`.
`LoadedState` gains two new required methods, `archived_card_list` and
`archived_cards_of_board`, implemented on `Model` and `Overlay`, letting a
fetch plan see an already-loaded archived scope and stop refetching it on
a later resolve call within the same process.
