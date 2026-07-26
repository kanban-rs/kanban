---
bump: minor
---

Managing a card's children or parents (from either the tasks panel or Card
Detail) now defaults to offering only live cards as candidates. Previously
an archived card could show up in the picker as if it were a normal,
selectable relative, which wasn't particularly useful in the common case.
Managing relationships from an already-archived card (via the archived
tasks view) is unaffected: archived candidates are still fully available
there, matching how every other action on an archived card already works.
