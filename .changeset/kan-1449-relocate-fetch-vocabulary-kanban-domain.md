---
bump: minor
---

domain,service: relocate the fetch-planning vocabulary (`FetchPlan`, `FetchRound`, `FetchStatus`, `LoadedState`, `LoadedEntities`, `requestable`) from `kanban-domain` to `kanban-service`. No behavioural change; this is a pure module move. The bump is minor because `kanban-domain` loses seven public items and `kanban-service` gains new public API, and pre-1.0 that qualifies as breaking.
