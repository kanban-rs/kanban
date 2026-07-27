---
bump: patch
---

Internal groundwork for the upcoming HTTP API (no user-facing change). Aligns
how the board create/replace logic will handle PUT requests with the wire
convention the API's design already calls for (sprints already follow it;
columns and cards will be brought in line as their own write-route work
lands), ahead of the routes themselves landing.
