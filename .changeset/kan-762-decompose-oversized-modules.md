---
bump: patch
---

Internal code-quality refactor with no user-visible behaviour change. Six of the largest modules in the codebase were broken up from single multi-thousand-line files into focused, single-responsibility submodules: the terminal UI application loop, the SQLite storage backend, the MCP server, the service-layer context, the in-memory store, and the card command set. All public interfaces, commands, and runtime behaviour are unchanged; this is purely a maintainability and readability improvement that makes the code easier to navigate and extend. Nothing to do as a user.
