---
bump: patch
---

Internal groundwork with no user-visible effect: backends can now be described by a factory registered against a URI scheme, rather than being named directly by the service layer. Nothing uses the registry yet, so behaviour, storage formats, and commands are unchanged. This is the seam that will later let an application decide for itself which backends exist, including pointing the CLI, TUI, or MCP server at a remote server instead of a local file.
