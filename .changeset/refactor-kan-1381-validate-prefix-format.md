---
bump: patch
---

core: rename `validate_branch_prefix` to `validate_prefix_format`. It validates card and sprint prefixes, not git branches; the old name was leftover vocabulary that made the function hard to find and easy to misread. No behavior change.
