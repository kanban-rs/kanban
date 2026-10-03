---
bump: patch
---

domain: `Model::invalidate`'s exact card branch now also drops the column a card is currently cached in, so a producer that names only a move's destination cannot leave a stale copy in the source column; the four archived-card tiers now drop on every card invalidation, and `EntityIds::archival_changed` is documented as unread. No producer populates `card_columns` yet, so behaviour is unchanged today.
