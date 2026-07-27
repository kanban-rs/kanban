---
bump: patch
---

`kanban-server -V`/`--version`/`--help` no longer crash. The binary previously had no argument parsing at all, so any flag was ignored and it tried to open its data file before doing anything else — now these flags print immediately and exit without touching any file.
