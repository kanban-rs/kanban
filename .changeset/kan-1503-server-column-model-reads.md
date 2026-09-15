---
bump: patch
---

server: the column read routes (list, get, and the flat get) now read the shared session Model instead of calling the KanbanContext directly, keeping every response byte-identical while populating the Model for later reads in the same request.
