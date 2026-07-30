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
  'crates/kanban-api/src/v1/**/conversions.rs'
  'crates/kanban-api/src/v1/**/response.rs'
)

# Match either a `..` rest pattern or a Default::default() call.
#   - `\.\.[[:space:]]*[},]` : `..` immediately followed (ignoring whitespace) by
#     `}` or `,` — the struct rest pattern `Foo { a, .. }` / `..Default::default()` /
#     `..base`. Deliberately does NOT match range expressions like `0..10` or `..=`.
#   - `Default::default\(\)` : the banned call, caught even when the `..` heuristic
#     would miss it.
PATTERN='(\.\.[[:space:]]*[},])|Default::default\(\)'

# Strip the comment portion of a Rust line before the guard inspects it, so prose
# in doc comments (e.g. "no `Default::default()`" or "{ .. }") can never trip the
# pattern — only CODE is checked. We walk the line char-by-char tracking whether we
# are inside a string/char literal (respecting backslash escapes); the first `//`
# seen OUTSIDE a literal starts a line comment and everything from there is dropped.
# The `// lock-exempt:` escape hatch is preserved: such lines are emitted verbatim
# so the downstream `grep -v` can filter them, while every other line is emitted as
# CODE-only (comment removed). Output keeps `filename:lineno:` so rg's -n style
# violation reporting is unchanged.
strip_comments() {
  awk '
    /\/\/ lock-exempt:/ { print FILENAME ":" FNR ":" $0; next }
    {
      n = length($0)
      out = ""
      in_str = 0   # 0=code, 34=in "..", 39=in '\''..'\''
      esc = 0
      for (i = 1; i <= n; i++) {
        c = substr($0, i, 1)
        if (in_str) {
          out = out c
          if (esc) { esc = 0 }
          else if (c == "\\") { esc = 1 }
          else if ((in_str == 34 && c == "\"") || (in_str == 39 && c == "'\''")) { in_str = 0 }
          continue
        }
        if (c == "\"") { in_str = 34; out = out c; continue }
        if (c == "'\''") { in_str = 39; out = out c; continue }
        if (c == "/" && substr($0, i + 1, 1) == "/") { break }  # line comment
        out = out c
      }
      print FILENAME ":" FNR ":" out
    }
  ' "$@"
}

violations=0
for g in "${GLOBS[@]}"; do
  # Resolve the glob to real files, strip comments from each, then run the pattern
  # over the CODE-only stream. Do NOT redirect stderr to /dev/null anywhere: a
  # swallowed error would turn the guard into a silent no-op. `|| true` keeps
  # `set -e` from aborting on rg's no-match exit code 1.
  files=$(rg --files -g "$g" . || true)
  [ -z "$files" ] && continue
  # shellcheck disable=SC2086
  m=$(strip_comments $files | rg "$PATTERN" | grep -v '// lock-exempt:' || true)
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
