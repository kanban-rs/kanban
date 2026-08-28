---
bump: patch
---

persistence: replacing a workspace's contents from a snapshot now stores one prefix row per namespace with a normalised name on every backend, so a file that recorded `KAN` and `kan` separately no longer reads back as two namespaces able to hand out the same card number.
