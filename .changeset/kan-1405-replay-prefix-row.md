---
bump: patch
---

domain: fix `CreateCard` replay leaving its card's prefix row unbacked. The row was previously only created by the service layer's number allocation, which never runs during command-log replay; `CreateCard::execute` now upserts the row itself, raising its counter to the card's frozen number without re-allocating one.
