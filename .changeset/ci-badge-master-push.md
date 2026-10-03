---
bump: patch
---

Run CI on pushes to master so the README CI badge reflects the actual state of the default branch. The badge previously showed the latest run whose head branch was master, which was a failed develop-sync PR from the v0.8.0 era; with syncs now done by direct push, no newer run ever replaced it and the badge stayed red permanently. A concurrency group cancels the superseded run when the release pipeline pushes master twice in quick succession (release commit, then the AUR bump).
