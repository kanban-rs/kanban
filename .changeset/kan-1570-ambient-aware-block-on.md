---
bump: patch
---

backend-http: bridge block_on through block_in_place so reads work when a Tokio runtime already drives the calling thread
