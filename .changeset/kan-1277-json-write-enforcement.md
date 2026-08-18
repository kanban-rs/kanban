---
bump: patch
---

persistence-json: a JSON workspace now refuses a change that would create a card whose prefix namespace has no record, and rolls the whole change back, so a JSON store cannot drift into a shape the SQLite backend would reject.
