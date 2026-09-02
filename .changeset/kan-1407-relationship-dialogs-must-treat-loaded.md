---
bump: patch
---

tui: relationship dialogs (manage parents, manage children, manage children from list) now refuse to open and raise an error banner instead of silently treating an unloaded dependency graph as empty, which previously disabled cycle filtering and turned every checkbox press into an attach. The card detail view's relationship panel now shows a not-loaded/failed marker instead of a misleading "No parents"/"No children" when the graph tier hasn't loaded yet.
