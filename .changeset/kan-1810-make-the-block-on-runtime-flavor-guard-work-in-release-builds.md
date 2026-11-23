---
bump: patch
---

HttpBackend now raises its intended "HttpBackend requires a multi-threaded Tokio runtime" message when used under a current_thread runtime in all build profiles, instead of the generic tokio panic message release builds previously produced
