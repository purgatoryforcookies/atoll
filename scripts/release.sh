#!/bin/sh
# Cuts a release. Usage: scripts/release.sh patch|minor|major
# Bumps Cargo.toml from the last v* tag, drafts notes with `claude -p` for you to edit,
# then commits, tags and pushes; the tag starts .github/workflows/release.yml.
set -eu
cd "$(dirname "$0")/.."

case ${1:-} in patch|minor|major) ;; *) echo "usage: $0 patch|minor|major" >&2; exit 1 ;; esac
[ "$(git branch --show-current)" = main ] || { echo "not on main" >&2; exit 1; }
[ -z "$(git status --porcelain --untracked-files=no)" ] || { echo "uncommitted changes" >&2; exit 1; }
git fetch --quiet --tags origin

PREV=$(git describe --tags --abbrev=0 --match 'v*' 2>/dev/null || echo v0.0.0)
VERSION=$(echo "${PREV#v}" | awk -F. -v bump="$1" '{
  if (bump == "major") print $1+1 ".0.0"; else if (bump == "minor") print $1 "." $2+1 ".0"; else print $1 "." $2 "." $3+1 }')
RANGE=$PREV..HEAD
[ "$PREV" = v0.0.0 ] && RANGE=HEAD

NOTES=$(mktemp)
trap 'rm -f "$NOTES"' EXIT
{ git log --no-merges --format='- %s%n%b' "$RANGE"; git diff --stat "$RANGE" 2>/dev/null || true; } |
  claude -p "These are the commits since Atoll $PREV. Atoll is a macOS menu-bar app that turns the MacBook notch into a Dynamic Island for Claude Code. Write release notes for Atoll $VERSION for its users: a few short markdown bullets grouped under ### headings only if there are several kinds of change, plain language, no internal refactors, CI or docs-only changes unless they matter to users. Output only the notes." >"$NOTES" ||
  git log --no-merges --format='- %s' "$RANGE" >"$NOTES"
${EDITOR:-vi} "$NOTES"
[ -s "$NOTES" ] || { echo "empty notes, aborting" >&2; exit 1; }

sed -i '' "1,/^version = /s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
cargo check --quiet # updates Cargo.lock
git commit --quiet -m "Release $VERSION" Cargo.toml Cargo.lock
# verbatim keeps markdown headings, which git would strip as comments
git tag -a "v$VERSION" -F "$NOTES" --cleanup=verbatim
git push --atomic origin main "v$VERSION"
echo "pushed v$VERSION"
