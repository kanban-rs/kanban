# Changesets

When creating a PR, add a changeset file to describe your changes.

## Creating a Changeset

```bash
nix run .#changeset
```

Or create a file `.changeset/<descriptive-name>.md` manually:

```md
---
bump: patch
---

Brief description of changes for the changelog
```

## Bump Types

- `patch` - Bug fixes, small changes (0.1.0 → 0.1.1)
- `minor` - New features, backwards compatible (0.1.0 → 0.2.0)
- `major` - Breaking changes (0.1.0 → 1.0.0)

On merge to master, changesets are aggregated and the highest bump type determines the version increment.

### While the project is pre-1.0

Use `minor` for breaking changes, not `major`. Under Cargo's semver rules a
`0.x` minor bump already breaks callers pinned to `^0.x`, so `minor` is the
strongest signal available without declaring the API stable. Reserve `major`
for the deliberate 1.0.0 release.

This applies to anything that breaks a downstream crate: removing or renaming a
public item, changing a public function or trait method signature, and removing
a trait's default implementation (which breaks every out-of-tree implementor,
even though nothing in this workspace notices).

Every crate in `crates/` publishes to crates.io, so "downstream" means real
external users, not just this repository.
