---
bump: minor
---

domain,view,tui: `ModelChanged` now carries whether the mutator that minted it actually wrote, exposed as `any()`, and `merge` folds two receipts by disjunction instead of discarding the second one. `apply_resolved` of an untouched `Resolved` and `invalidate` of an empty `EntityIds` report unchanged, while `load_from_snapshot`, `invalidate(Invalidation::All)` and every touched `apply_resolved` still report changed. `Controller::resync` skips rebuilding the archived-at side map and both live/archived partitions when the receipt reports unchanged, so a keystroke that mutates nothing no longer clones every `Card` and `Board` and re-sorts both board partitions. `Model` gains `replace_with`, and `App::reload_model`'s failure-rollback now restores the pre-reload model through that receipt-returning call instead of a direct field assignment paired with a no-op `apply_resolved` used purely to force a rebuild.
