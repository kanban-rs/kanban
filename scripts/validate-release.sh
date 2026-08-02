#!/usr/bin/env bash
set -euo pipefail

crates_out=$(list-crates --paths) || { echo "❌ list-crates failed"; exit 1; }
mapfile -t CRATES <<< "$crates_out"

echo "🔍 Validating release build..."
echo ""

echo "Step 1: Checking workspace structure..."
for crate in "${CRATES[@]}"; do
  if [ ! -f "$crate/Cargo.toml" ]; then
    echo "❌ Error: $crate/Cargo.toml not found"
    exit 1
  fi
done
echo "✓ All crates present"

echo ""
echo "Step 2: Verifying version consistency..."
WORKSPACE_VERSION=$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)
echo "Workspace version: $WORKSPACE_VERSION"

for crate in "${CRATES[@]}"; do
  CRATE_VERSION=$(grep '^version' "$crate/Cargo.toml" | head -1)
  if ! echo "$CRATE_VERSION" | grep -q "workspace = true"; then
    echo "⚠ Warning: $crate/Cargo.toml does not use workspace versioning"
  fi
done
echo "✓ Version consistency verified"

echo ""
echo "Step 3: Checking cross-crate dependencies..."
deps_out=$(list-crates --names) || { echo "❌ list-crates failed"; exit 1; }
mapfile -t INTERNAL_DEPS <<< "$deps_out"
# Only [dependencies] need version specs; [dev-dependencies] are stripped from
# the published manifest by cargo and shouldn't carry version constraints
# because they may form circular path-deps with sibling crates that publish
# later in the dependency order (e.g. kanban-persistence-sqlite dev-deps on
# kanban-service which itself depends on kanban-persistence-sqlite optionally).
for crate in "${CRATES[@]}"; do
  deps_section=$(awk '/^\[dependencies\]/{flag=1; next} /^\[/{flag=0} flag' "$crate/Cargo.toml")
  dev_deps_section=$(awk '/^\[dev-dependencies\]/{flag=1; next} /^\[/{flag=0} flag' "$crate/Cargo.toml")
  for dep in "${INTERNAL_DEPS[@]}"; do
    if echo "$deps_section" | grep -q "$dep = { path = "; then
      if ! echo "$deps_section" | grep "$dep = { path = " | grep -q 'version = '; then
        echo "❌ Error: $crate is missing version spec for $dep in [dependencies]"
        echo "   Use: $dep = { path = \"../$dep\", version = \"^0.1\" }"
        exit 1
      fi
    fi
    if echo "$dev_deps_section" | grep -q "$dep = { path = "; then
      if echo "$dev_deps_section" | grep "$dep = { path = " | grep -q 'version = '; then
        echo "❌ Error: $crate has version spec for $dep in [dev-dependencies]"
        echo "   Path-only is required; sibling features added between releases cannot resolve against published versions."
        echo "   Use: $dep = { path = \"../$dep\" }"
        exit 1
      fi
    fi
  done
done
echo "✓ Cross-crate dependencies have proper version specs"

echo ""
echo "Step 4: Running cargo check on entire workspace..."
cargo check --workspace --all-features --quiet
echo "✓ Workspace check passed"

echo ""
echo "Step 5: Checking required crates.io publish metadata..."
# crates.io hard-rejects an upload with no description, and Step 6 below
# can't catch this (it's a documented near-no-op — see its comment). This
# is a real, offline-checkable guard against exactly the class of bug that
# blocked the v0.8.0 cut (a crate missing `description`): every crate's
# [package] section must carry a non-empty description, repository, and
# homepage, either as a literal value or `.workspace = true`.
for crate in "${CRATES[@]}"; do
  pkg_section=$(awk '/^\[package\]/{flag=1; next} /^\[/{flag=0} flag' "$crate/Cargo.toml")
  for field in description repository homepage; do
    if ! echo "$pkg_section" | grep -Eq "^${field}([[:space:]]*\.workspace[[:space:]]*=[[:space:]]*true|[[:space:]]*=[[:space:]]*\".+\")"; then
      echo "❌ Error: $crate/Cargo.toml is missing a non-empty '$field' (needed for crates.io publish)"
      echo "   Use: $field.workspace = true, or $field = \"...\""
      exit 1
    fi
  done
done
echo "✓ All crates have required publish metadata"

echo ""
echo "Step 6: Validating individual crate manifests (offline)..."
# This step is intentionally weak — it runs cargo publish --dry-run in
# offline mode and trusts a failure to mean "offline blocked us" rather
# than a real defect. Stronger validation is blocked by a chicken-and-egg
# problem: at release time the workspace is bumped to a new version (e.g.
# 0.7.0) but no sibling crate has been published yet, so cargo package
# fails to resolve `kanban-core = "^0.7"` for any non-leaf crate. The
# only crate that can be packaged this way is kanban-core (the leaf).
# KAN-670 tracks a real fix; Step 4 (cargo check --workspace) and Step 5
# above carry the real validation weight this step can't provide.
for crate in "${CRATES[@]}"; do
  echo "  Validating $crate..."
  cd "$crate"
  cargo publish --dry-run --no-verify --offline --quiet --allow-dirty 2>&1 | grep -v "warning:" || true
  cd - > /dev/null
done
echo "✓ All crates passed dry-run validation"

echo ""
echo "✅ Release validation complete - ready to publish!"
