---
bump: patch
---

The workspace `default_card_prefix` and `default_sprint_prefix` settings are now respected everywhere a prefix is resolved. Card search and identifier lookup previously fell back to the built-in `task` regardless of configuration, so a workspace with its own default could not find its cards by the name it displayed for them. Sprint and board prefixes now resolve through one shared rule, so the two can no longer disagree.
