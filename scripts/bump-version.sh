#!/usr/bin/env bash
set -euo pipefail

PR_NUMBER="${1:-}"

CURRENT_VERSION=$(grep -m1 'version = ' Cargo.toml | cut -d'"' -f2)

if [ -z "$(ls -A .changeset/*.md 2>/dev/null)" ]; then
  echo "Error: No changesets found in .changeset/"
  exit 1
fi

BUMP_TYPE="patch"
CHANGELOG_ENTRIES=""

for changeset in .changeset/*.md; do
  [ -e "$changeset" ] || continue

  bump=$(grep -A1 "^---$" "$changeset" | grep "^bump:" | cut -d' ' -f2 | tr -d '\r\n')
  description=$(sed -n '/^---$/,/^---$/!p' "$changeset" | sed '/^---$/d' | sed '/^$/d')

  if [ "$bump" = "major" ]; then
    BUMP_TYPE="major"
  elif [ "$bump" = "minor" ] && [ "$BUMP_TYPE" != "major" ]; then
    BUMP_TYPE="minor"
  fi

  CHANGELOG_ENTRIES+="- $description\n"
done

IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"

case "$BUMP_TYPE" in
  major)
    NEW_VERSION="$((MAJOR + 1)).0.0"
    ;;
  minor)
    NEW_VERSION="${MAJOR}.$((MINOR + 1)).0"
    ;;
  patch)
    NEW_VERSION="${MAJOR}.${MINOR}.$((PATCH + 1))"
    ;;
esac

echo "Bumping version: $CURRENT_VERSION → $NEW_VERSION (type: $BUMP_TYPE)"

DATE=$(date +%Y-%m-%d)
PR_LINK=""
if [ -n "$PR_NUMBER" ]; then
  PR_LINK=" ([#$PR_NUMBER](https://github.com/fulsomenko/kanban/pull/$PR_NUMBER))"
fi

if [ ! -f CHANGELOG.md ]; then
  echo "# Changelog" > CHANGELOG.md
  echo "" >> CHANGELOG.md
fi

{
  echo "## [$NEW_VERSION] - $DATE$PR_LINK"
  echo ""
  echo -e "$CHANGELOG_ENTRIES"
  echo ""
  cat CHANGELOG.md
} > CHANGELOG.md.new
mv CHANGELOG.md.new CHANGELOG.md

sed "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml > Cargo.toml.tmp
mv Cargo.toml.tmp Cargo.toml

for crate in crates/*/Cargo.toml; do
  sed "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" "$crate" > "$crate.tmp"
  mv "$crate.tmp" "$crate"
done

cargo update --workspace

rm -f .changeset/*.md

echo "Version bumped to $NEW_VERSION"
echo "CHANGELOG.md updated"
echo "Changesets processed and deleted"

# Commit the version bump
git add .
git commit -m "chore: bump version to $NEW_VERSION"
