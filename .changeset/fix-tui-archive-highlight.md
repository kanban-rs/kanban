---
bump: patch
---

tui: archiving a project from the Projects panel keeps the highlight at the vacated position again instead of snapping to the first project. The index was pinned against the pre-archive board list, so the next frame's identity-preserving resync moved the selection to the top; the handler now resyncs the list before pinning, matching the restore and permanent-delete handlers.
