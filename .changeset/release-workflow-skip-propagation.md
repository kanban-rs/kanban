---
bump: patch
---

Fixed `build-windows`/`publish-chocolatey` still being skipped on a manual
`workflow_dispatch` recovery run even when their own `if` condition was
true. GitHub Actions auto-skips a job when *any* of its `needs` jobs was
skipped, overriding a custom `if` unless that condition includes
`always()` — `release` is legitimately skipped on `workflow_dispatch`
(it only runs on a merged-PR trigger), which was silently defeating the
recovery path added in the previous changeset.
