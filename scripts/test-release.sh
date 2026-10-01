#!/usr/bin/env bash
# Run release.sh in a throwaway clone pushing to a throwaway bare remote.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
fail() { echo "FAIL: $*" >&2; exit 1; }
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
git init -q --bare "$work/remote.git"
git -C "$work/remote.git" config receive.shallowUpdate true   # CI checks out a shallow clone
git clone -q "$root" "$work/clone"
cd "$work/clone"
git checkout -q -B main
cp "$root/scripts/release.sh" scripts/release.sh
git add scripts/release.sh
git -c user.name=t -c user.email=t@t commit -qm "test: current release.sh" >/dev/null || true
git remote set-url origin "$work/remote.git"
git push -q origin main
export CST_RELEASE_SKIP_TESTS=1 GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t

# refusals change nothing
if scripts/release.sh 1.2 >/dev/null 2>&1; then fail "malformed version accepted"; fi
echo x > dirty.txt
if scripts/release.sh 9.9.9 >/dev/null 2>&1; then fail "dirty tree accepted"; fi
rm dirty.txt
git checkout -q -b other
if scripts/release.sh 9.9.9 >/dev/null 2>&1; then fail "non-main branch accepted"; fi
git checkout -q main

# local main behind origin/main: refuse before changing anything
git clone -q "$work/remote.git" "$work/other"
(cd "$work/other" && git checkout -q main && echo x >> README.md && git -c user.name=t -c user.email=t@t commit -qam "remote edit" && git push -q origin main)
before="$(git rev-parse HEAD)"
if scripts/release.sh 9.9.9 >/dev/null 2>&1; then fail "out-of-date main accepted"; fi
[ "$(git rev-parse HEAD)" = "$before" ] || fail "refusal committed"
git diff --quiet || fail "refusal modified files"
git pull -q --ff-only origin main

# dry run changes nothing
head="$(git rev-parse HEAD)"
scripts/release.sh 9.9.9 --dry-run >/dev/null
[ "$(git rev-parse HEAD)" = "$head" ] || fail "dry run committed"
git diff --quiet || fail "dry run modified files"
git rev-parse -q --verify refs/tags/v9.9.9 >/dev/null && fail "dry run tagged"

# releasing the version already in Cargo.toml tags without a commit
cur="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
scripts/release.sh "$cur" >/dev/null || fail "release of current version $cur"
[ "$(git rev-parse HEAD)" = "$head" ] || fail "same-version release committed"
git -C "$work/remote.git" rev-parse -q --verify "refs/tags/v$cur" >/dev/null || fail "v$cur not pushed"

# real run: bump, commit, tag, push
scripts/release.sh 9.9.9 >/dev/null
grep -q '^version = "9.9.9"' Cargo.toml || fail "Cargo.toml not bumped"
git -C "$work/remote.git" rev-parse -q --verify refs/tags/v9.9.9 >/dev/null || fail "tag not pushed"
[ "$(git -C "$work/remote.git" show v9.9.9:Cargo.toml | grep -c '^version = "9.9.9"')" = 1 ] || fail "tag content"
if scripts/release.sh 9.9.9 >/dev/null 2>&1; then fail "existing tag accepted"; fi
echo "release script OK"
