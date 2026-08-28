---
bump: patch
---

domain: `find_cards_by_identifier` derives its column-id lookup from the pair projection it already builds, rather than scanning `columns` a second time to build the same data twice. Also corrects a comment that named `resolve_card_prefix` in a path that calls `resolve_card_prefix_by_ids`.
