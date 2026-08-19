---
bump: minor
---

domain: Resolved gains a uniform Collection<T> tier. boards, columns, cards and sprints each carry both an all: LoadState<Vec<T>> for the whole collection and a by_id: HashMap<Uuid, LoadState<T>> for individual entities, so a resolve pass can now say that a collection loaded and is genuinely empty rather than only that a given entity was not fetched. graph stays a bare LoadState<DependencyGraph> because it is a singleton with no id to key on.
