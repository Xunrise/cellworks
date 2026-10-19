#!/usr/bin/env bash
# Prints the version a push to main releases (without the leading v).
# Cargo.toml's version when it is ahead of the latest tag (a manual minor or
# major bump); otherwise the latest tag's patch number plus one.
#
# The deliver stage checks out one commit without tags, so with GH_TOKEN set
# the tags are read from the repository through the API instead.
set -euo pipefail

base=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
if [ -n "${GH_TOKEN:-}" ] && [ -n "${GITHUB_REPOSITORY:-}" ]; then
  tags=$(gh api "repos/$GITHUB_REPOSITORY/tags?per_page=100" --paginate --jq '.[].name')
else
  tags=$(git tag --list)
fi
latest=$(printf '%s\n' "$tags" | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' | sort -rV | head -1 | sed 's/^v//' || true)
if [ -n "$latest" ] && [ "$(printf '%s\n%s\n' "$base" "$latest" | sort -V | tail -1)" = "$latest" ]; then
  IFS=. read -r major minor patch <<<"$latest"
  echo "$major.$minor.$((patch + 1))"
else
  echo "$base"
fi
