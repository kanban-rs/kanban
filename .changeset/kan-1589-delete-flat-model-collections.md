---
bump: patch
---

domain,backend-http: delete the flat column/card/sprint Model collections, leaving scoped and per-id fetches as the only source of truth. User-visible: a CardDetail relation card on another board now has its body fetched (previously it silently vanished from the parents/children panel), and four HttpBackend DataStore methods now decline under their own names instead of a sibling's.
