---
bump: minor
---

domain,service: `Resolved` gains a fourth tier, `archived_cards: Collection<ArchivedCard>`, and `FetchRound` gains the two request fields that fill it, `archived_card_list: bool` and `archived_cards_by_board: Vec<Uuid>`. The resolver populates the flat tier from `DataStore::list_archived_cards` and the board-scoped tier from `list_archived_cards_by_board`, mapping a read error to `LoadState::Failed` rather than collapsing it to an empty list. Nothing in `Model`, `LoadedState` or `Overlay` changes, so nobody consumes the new tier yet. It is a `minor` bump because adding public fields to public, non-`#[non_exhaustive]` structs on crates that publish to crates.io is both new public API and breaking for any exhaustive struct literal.
