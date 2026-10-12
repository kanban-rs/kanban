---
bump: patch
---

backend-http: the write-parity tests close the seeded SQLite store before copying it, so the copy no longer races a live WAL that Windows keeps locked (an intermittent Test (Windows) failure that also hid later crates' Windows tests).
