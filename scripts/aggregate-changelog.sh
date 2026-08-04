#!/usr/bin/env bash
set -euo pipefail

cleanup() {
  rm -f CHANGELOG.md.new
}
trap cleanup EXIT

PR_NUMBER="${1:-}"

CURRENT_VERSION=$(grep -m1 'version = ' Cargo.toml | cut -d'"' -f2)

if [ ! -d ".changeset" ]; then
  echo "No changesets to aggregate"
  exit 0
fi

changeset_count=$(find .changeset -maxdepth 1 -name "*.md" ! -name "README.md" | wc -l | tr -d ' ')
if [ "$changeset_count" -eq 0 ]; then
  echo "No changesets to aggregate"
  exit 0
fi

echo "Aggregating $changeset_count changesets into CHANGELOG.md for version $CURRENT_VERSION"

DATE=$(date +%Y-%m-%d)
CHANGELOG_ENTRIES=""
# Non-card ("OTHER") changesets are collected here and emitted under a single
# "### Other Changes" section, rather than one repeated header per changeset.
OTHER_BODIES=""
for changeset in $(find .changeset -maxdepth 1 -name "*.md" ! -name "README.md" | sort); do
  [ -e "$changeset" ] || continue

  filename=$(basename "$changeset" .md)
  card_id=""
  branch_name=""

  # Recognize a card ID (e.g. kan-1046) ANYWHERE in the filename, not only at the
  # start, so a conventional-commit-style name like
  # "feat-kan-1046-configurable-bind-addr" is attributed to KAN-1046 rather than
  # bucketed under Other Changes. The first "<letters>-<digits>" token wins.
  if [[ "$filename" =~ ([a-zA-Z]+-[0-9]+)-(.+)$ ]]; then
    card_id=$(echo "${BASH_REMATCH[1]}" | tr '[:lower:]' '[:upper:]')
    branch_name=$(echo "${BASH_REMATCH[2]}" | tr '-' ' ' | sed 's/\b\(.\)/\u\1/g')
  elif [[ "$filename" =~ ([a-zA-Z]+-[0-9]+)$ ]]; then
    card_id=$(echo "${BASH_REMATCH[1]}" | tr '[:lower:]' '[:upper:]')
  else
    card_id="OTHER"
  fi

  # Extract the body (everything outside the --- frontmatter). Strip leading and
  # trailing blank lines but PRESERVE internal ones (collapsed to a single blank)
  # so multi-paragraph descriptions keep their paragraph breaks, and demote any
  # markdown headings to level 4+ so a changeset body cannot collide with the
  # version (##) or entry (###) header levels of the changelog outline.
  description=$(sed -n '/^---$/,/^---$/!p' "$changeset" | sed '/^---$/d' \
    | awk '/^[[:space:]]*$/ { if (started) blank=1; next } { if (started && blank) print ""; blank=0; started=1; print }' \
    | sed -E 's/^#{1,3} /#### /')

  if [ "$card_id" = "OTHER" ]; then
    OTHER_BODIES+="$description\n\n"
  elif [ -n "$branch_name" ]; then
    CHANGELOG_ENTRIES+="### $card_id $branch_name ($DATE)\n\n$description\n\n"
  else
    CHANGELOG_ENTRIES+="### $card_id ($DATE)\n\n$description\n\n"
  fi
done

# Prepend a single grouped "Other Changes" section (if any non-card changesets
# were collected), so the release shows one such header rather than one per file.
if [ -n "$OTHER_BODIES" ]; then
  CHANGELOG_ENTRIES="### Other Changes ($DATE)\n\n$OTHER_BODIES$CHANGELOG_ENTRIES"
fi

PR_LINK=""
if [ -n "$PR_NUMBER" ]; then
  REPO_URL=$(git remote get-url origin | sed 's/\.git$//' | sed 's|git@github.com:|https://github.com/|')
  PR_LINK=" ([#$PR_NUMBER]($REPO_URL/pull/$PR_NUMBER))"
fi

if [ ! -f CHANGELOG.md ]; then
  cat > CHANGELOG.md <<'EOF'
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

EOF
fi

# Insert the new version section AFTER the title and preamble (before the first
# existing "## [" version header), so the "# Changelog" title stays at the top of
# the file across releases instead of being pushed down one section each time.
{
  sed '/^## \[/,$d' CHANGELOG.md
  echo "## [$CURRENT_VERSION] - $DATE$PR_LINK"
  echo ""
  printf '%b' "$CHANGELOG_ENTRIES"
  echo ""
  sed -n '/^## \[/,$p' CHANGELOG.md
} > CHANGELOG.md.new
mv CHANGELOG.md.new CHANGELOG.md

find .changeset -maxdepth 1 -name "*.md" ! -name "README.md" -delete

echo "Aggregated changesets into CHANGELOG.md for version $CURRENT_VERSION"
