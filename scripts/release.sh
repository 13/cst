#!/usr/bin/env bash
# Cut a release: bump Cargo.toml, commit, tag vX.Y.Z and push; the tag
# triggers .github/workflows/release.yml.
#   scripts/release.sh X.Y.Z [--dry-run]
set -euo pipefail
cd "$(dirname "$0")/.."

ver="${1:-}"
dry=0
[ "${2:-}" = --dry-run ] && dry=1
fail() { echo "release: $*" >&2; exit 1; }
[[ "$ver" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "usage: scripts/release.sh X.Y.Z [--dry-run]"
[ "$(git rev-parse --abbrev-ref HEAD)" = main ] || fail "not on main"
[ -z "$(git status --porcelain)" ] || fail "working tree not clean"
if git rev-parse -q --verify "refs/tags/v$ver" >/dev/null; then fail "tag v$ver already exists"; fi
if [ "${CST_RELEASE_SKIP_TESTS:-0}" != 1 ]; then
  cargo test -q >/dev/null 2>&1 || fail "cargo test fails"
fi

run() {
  echo "+ $*"
  if [ "$dry" = 0 ]; then "$@"; fi
}
run sed -i "0,/^version = \".*\"/s//version = \"$ver\"/" Cargo.toml
run cargo update -q --workspace --offline
if [ "$dry" = 1 ] || ! git diff --quiet; then run git commit -qam "release v$ver"; fi   # same version: tag only
run git tag -a "v$ver" -m "cst v$ver"
run git push -q origin main "v$ver"
[ "$dry" = 1 ] && echo "(dry run: nothing changed)"
echo "pushed v$ver — follow the release at https://github.com/13/cst/actions"
