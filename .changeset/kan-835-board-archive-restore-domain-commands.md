---
bump: patch
---

Internal domain layer for board archiving (no user-facing change yet): a board can now be archived and restored as a discrete first-class record. Archiving moves the board head out of the live set into a separate archived collection while its columns, cards, and sprints stay in place; restore moves it back losslessly, and both operations undo symmetrically. The service wiring, persistence, and UI arrive in later slices.
