---
bump: patch
---

Board archiving is now usable through the service layer (no surface UI yet): a board can be archived, restored, and its archived records listed, all undoable. Permanent deletion works on a board whether it is live or archived, so applications can offer a safe archive-then-delete flow. Undoing a permanent delete restores the board to the state it was deleted from (archived boards come back archived), along with its columns, cards, and sprints.
