#!/usr/bin/env bash
# Fails the build if a `..` rest pattern or `Default::default()` appears in the
# factory boundary + DTO conversion modules. These types are Default-free and
# must be destructured/constructed exhaustively so a new field is a compile error
# (the DTO<->persistence<->domain compile-time coverage lock). See KAN-769/KAN-770.
#
# A `..` is allowed where the line is annotated with the audited marker
# `// lock-exempt: <reason>`.
set -euo pipefail

# Path globs the lock covers. Extend as entity factories land.
#   - *_factory.rs : XRecord defs + reconstitute + From<&X> (board_factory.rs, ...).
#                    PIN this naming: future ColumnRecord/SprintRecord land here so
#                    the guard keeps seeing them.
#   - api/v1/**/conversions.rs and response.rs : DTO <-> domain mapping.
GLOBS=(
  'crates/kanban-domain/src/*_factory.rs'
  'crates/kanban-service/src/api/v1/**/conversions.rs'
  'crates/kanban-service/src/api/v1/**/response.rs'
)

# Match either a `..` rest pattern or a Default::default() call.
#   - `\.\.[[:space:]]*[},]` : `..` immediately followed (ignoring whitespace) by
#     `}` or `,` — the struct rest pattern `Foo { a, .. }` / `..Default::default()` /
#     `..base`. Deliberately does NOT match range expressions like `0..10` or `..=`.
#   - `Default::default\(\)` : the banned call, caught even when the `..` heuristic
#     would miss it.
PATTERN='(\.\.[[:space:]]*[},])|Default::default\(\)'

violations=0
for g in "${GLOBS[@]}"; do
  # ripgrep takes the regex positionally (no -E flag exists). Do NOT redirect
  # stderr to /dev/null: a swallowed rg error would turn the guard into a silent
  # no-op. `|| true` keeps `set -e` from aborting on rg's no-match exit code 1.
  m=$(rg -n "$PATTERN" -g "$g" . | grep -v '// lock-exempt:' || true)
  if [ -n "$m" ]; then
    echo "FACTORY LOCK VIOLATION (glob: $g):"
    echo "$m"
    violations=1
  fi
done

if [ "$violations" -ne 0 ]; then
  echo ""
  echo "❌ Factory boundary modules must be Default-free and use no \`..\` rest pattern."
  echo "   Destructure/construct every field explicitly so a new field is a compile error."
  echo "   If a \`..\` is genuinely required, annotate the line with \`// lock-exempt: <reason>\`."
  exit 1
fi
echo "✅ Factory compile-lock guard clean"
