---
bump: patch
---

backend-http: shut the owned runtime down in the background on drop, fixing the exit-101 abort when an http:// locator is dropped from inside an async context
