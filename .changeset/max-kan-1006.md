---
bump: patch
---

Fixed an intermittent test failure and closed a related edge case in how the TUI detects file changes made by other processes. Own writes are now identified by comparing a stamped identifier in the saved file against the running instance, rather than guessing from how many filesystem events arrived in a short window — a guess that could occasionally be wrong under heavy load, causing the app to briefly (and incorrectly) treat its own save as an external change.
