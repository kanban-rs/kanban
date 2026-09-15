---
bump: patch
---

tui: a failed store read during a model reload now raises the error banner instead of only logging a warning, so a transient read failure cannot leave the user staring at stale data with no visible feedback.
