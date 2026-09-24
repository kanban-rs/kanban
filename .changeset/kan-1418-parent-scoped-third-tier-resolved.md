---
bump: minor
---

domain: `Collection<T>` in `resolved.rs` gains a third, parent-keyed tier, `by_parent: HashMap<Uuid, LoadState<Vec<T>>>`, alongside the existing `all` and `by_id`. It lets a resolve pass describe the whole child set of one parent (columns of a board, cards of a column, sprints of a board) without loading the entire collection, which is what makes lazy loading actually lazy. `Default` and `is_untouched` account for the new tier and the hand-written `Default` still works for a `T` that is not `Default`. The change is purely additive: nothing fetches into the tier and nothing consumes it yet. It is a `minor` bump because adding a public field to a public struct that is not `#[non_exhaustive]` is both new public API and breaking for any exhaustive struct literal.
