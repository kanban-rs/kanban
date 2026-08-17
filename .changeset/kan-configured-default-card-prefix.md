---
bump: patch
---

Fixes the `default_card_prefix` setting having no effect. Card numbers were always allocated from the built-in `task` namespace even when another default was configured, while sprint numbers correctly used the configured `default_sprint_prefix`. Cards already created keep the prefix they were stamped with; new cards in a workspace that sets `default_card_prefix` are now numbered in that namespace.
