---
bump: patch
---

Pressing `e` (edit) through the in-app Help overlay now works everywhere it
should. Previously, selecting "Edit card" from the Help menu (or pressing `?`
then jumping straight to `e`) silently did nothing in every context: the card
list, card detail (title, description, or metadata), sprint detail, and the
settings configuration editor. Pressing `e` directly, outside the Help menu,
already worked correctly in all of these places.

The two paths now share one dispatch, so editing a card through the Help
menu opens the same editor as pressing `e` directly, in every mode.
