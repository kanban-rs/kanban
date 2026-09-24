#!/usr/bin/env bash
# Fails the build if `impl DataStore for HttpBackend` silently inherits a
# default-bodied trait method instead of naming it explicitly. An inherited
# default declines under a sibling method's name (or, for `modify_graph`,
# reaches the transport instead of declining at all) — see
# crates/kanban-backend-http/tests/decline_rulings.rs.
#
# Trait method count: every `fn` inside `pub trait DataStore { ... }` in
# crates/kanban-domain/src/data_store.rs, stopping at the trait's own closing
# brace (so its `#[cfg(test)] mod tests` below is excluded).
#
# Impl method count: every `fn` inside `impl DataStore for HttpBackend { ... }`
# in crates/kanban-backend-http/src/data_store.rs, stopping at that block's own
# closing brace. Scoping to the impl block (not the whole file) is load-bearing:
# the file also has an inherent `impl HttpBackend { fn lookup_cards(...) }`
# helper above the trait impl, which a whole-file count would wrongly include.
set -euo pipefail

TRAIT_FILE="crates/kanban-domain/src/data_store.rs"
IMPL_FILE="crates/kanban-backend-http/src/data_store.rs"

trait_count=$(awk '/^pub trait DataStore/,/^}/' "$TRAIT_FILE" | grep -c '    fn ')
impl_count=$(awk '/^impl DataStore for HttpBackend/,/^}/' "$IMPL_FILE" | grep -c '    fn ')

if [ "$trait_count" -ne "$impl_count" ]; then
  echo "❌ DataStore trait/impl method count mismatch: trait has $trait_count method(s), HttpBackend's impl has $impl_count"
  echo "   Every DataStore method must be an explicit impl on HttpBackend, even if it just declines."
  diff \
    <(awk '/^pub trait DataStore/,/^}/' "$TRAIT_FILE" | grep '    fn ' | sed -E 's/^\s*fn ([a-zA-Z0-9_]+).*/\1/' | sort) \
    <(awk '/^impl DataStore for HttpBackend/,/^}/' "$IMPL_FILE" | grep '    fn ' | sed -E 's/^\s*fn ([a-zA-Z0-9_]+).*/\1/' | sort) \
    || true
  exit 1
fi

echo "✅ DataStore explicit-impl guard clean ($trait_count/$impl_count)"
