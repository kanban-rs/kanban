---
bump: patch
---

tui: ViewScope stops requesting the flat column/card/sprint tiers; the render path and handlers already read the board-scoped ones. Consequence: CardDetail's parents/children panel no longer resolves a spawns relative on a different board unless its body was already fetched, since nothing populates the flat fallback any more.
