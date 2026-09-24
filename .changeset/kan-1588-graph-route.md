---
bump: minor
---

server adds GET /v1/graph returning the whole workspace dependency graph including archived edges; backend-http fetches it instead of declining, and a 404 from an older server is reported as a loud unsupported error rather than an empty graph
