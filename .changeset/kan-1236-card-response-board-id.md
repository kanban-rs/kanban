---
bump: patch
---

api: card read responses now carry the owning board's id, so a client reading a card over HTTP no longer needs a second request to find out which board it belongs to.
