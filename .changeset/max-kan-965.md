---
bump: patch
---

The archived-boards view now always defaults to recency order (most recently archived first), independent of whatever sort preference is saved for the live boards list. Previously, changing the live list's default sort (for example to sort by name) also silently changed the archived view's default, since both shared the same saved preference. The two are now independent: setting a live-view sort no longer affects how the archived view is ordered by default.
