# cst Distribution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish `cst` as the public GitHub repo `13/cst` with CI, tag-driven releases (static x86_64 binary + pacman package), CI-pushed AUR `cst-bin`, a one-line install/uninstall script and a polished README; ship v0.1.0.

**Architecture:** Repository files only (workflows, shell scripts, a PKGBUILD template, a stdlib-only Python ANSI→SVG converter, Markdown); no Rust code changes beyond `Cargo.toml` metadata. Every script has a local test that CI also runs; the release workflow is verified for real by tagging v0.1.0.

**Tech Stack:** GitHub Actions, bash, shellcheck, makepkg, Python 3 (stdlib), gh CLI.

**Spec:** `docs/superpowers/specs/2026-10-02-cst-distribution-design.md`

## Global Constraints

- Repo `13/cst`, public; URLs use `https://github.com/13/cst`.
- Only `x86_64`; release binary target `x86_64-unknown-linux-musl`.
- Release asset names carry no version: `cst-x86_64-linux.tar.gz`, `cst-x86_64-linux.tar.gz.sha256`; pacman package `cst-bin-<ver>-1-x86_64.pkg.tar.zst`.
- AUR package name `cst-bin`; CI pushes only when secret `AUR_SSH_KEY` is set, else warns and succeeds.
- Tags `vX.Y.Z` must equal `Cargo.toml` `version`.
- Licence MIT, holder "ben", 2026.
- All shell scripts pass `shellcheck` and use `set -euo pipefail`.
- Python scripts use the standard library only.

## Review Focus

1. `install.sh` piped into bash from a different working directory, without `~/.local/bin` on PATH, or on a machine with only `wget`: still installs, warns about PATH. Test: Task 2 `test-install.sh` (PATH without the target dir, run from `/`).
2. A corrupted or mismatched `.sha256` must abort before anything is installed. Test: Task 2 (`bad checksum` case).
3. `--uninstall` must never touch a pacman-owned `/usr/bin/cst` and must not fail when nothing is installed. Test: Task 2 (`uninstall twice` case).
4. `release.sh` on a dirty tree, an existing tag, a non-main branch or a malformed version must refuse without changing anything. Test: Task 3 `test-release.sh`.
5. The tag/version mismatch must fail the release before anything is published: the workflow's first step. Not exercised by pushing a bad tag (public side effect); the reviewer checks the step reads `Cargo.toml` correctly and runs before `gh release create`.

## File Structure

```
LICENSE                              MIT (Task 1)
Cargo.toml                           package metadata (Task 1)
.github/workflows/ci.yml             clippy, tests, shellcheck, script tests (Task 1, extended in 2, 3, 5)
install.sh                           install / uninstall (Task 2)
scripts/test-install.sh              local install/uninstall test (Task 2)
scripts/release.sh                   cut a version (Task 3)
scripts/test-release.sh              release.sh against a throwaway clone + bare remote (Task 3)
packaging/aur/PKGBUILD.in            cst-bin template (Task 4)
packaging/aur/render.sh              PKGBUILD.in → PKGBUILD (Task 4)
.github/workflows/release.yml        tag → release, pacman pkg, AUR (Task 4)
docs/RELEASING.md                    one-time AUR setup + release steps (Task 4)
scripts/ansi2svg.py                  ANSI capture → SVG (Task 5)
scripts/screenshot.sh                real TUI → docs/screenshot.svg (Task 5)
docs/screenshot.svg                  generated (Task 5)
README.md                            landing page (Task 6)
```

---

### Task 1: Licence, package metadata, CI workflow, local tools

**Files:**
- Create: `LICENSE`, `.github/workflows/ci.yml`
- Modify: `Cargo.toml` (`[package]` metadata)

**Interfaces:**
- Produces: CI job `test` that later tasks extend with one `run:` line each.

- [ ] **Step 1: Local tools.** `shellcheck` is not installed. Fetch the static binary into `~/.local/bin` (no sudo):

```bash
v=v0.10.0
curl -fsSL "https://github.com/koalaman/shellcheck/releases/download/$v/shellcheck-$v.linux.x86_64.tar.xz" | tar -xJ -C /tmp
install -Dm755 "/tmp/shellcheck-$v/shellcheck" ~/.local/bin/shellcheck
shellcheck --version | head -2
```

Expected: a version line. (If that release name 404s, take the newest `linux.x86_64` asset from https://github.com/koalaman/shellcheck/releases.)

- [ ] **Step 2: `LICENSE`** — standard MIT text, first line `MIT License`, copyright line `Copyright (c) 2026 ben`.

- [ ] **Step 3: `Cargo.toml` metadata** — add under `[package]`:

```toml
description = "Keyboard shortcut cheatsheet for your terminal apps, in the style of an awesome WM popup"
license = "MIT"
repository = "https://github.com/13/cst"
readme = "README.md"
keywords = ["cheatsheet", "keybindings", "tui", "shortcuts"]
categories = ["command-line-utilities"]
```

- [ ] **Step 4: `.github/workflows/ci.yml`**

```yaml
name: ci
on:
  push:
  pull_request:
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - name: Clippy
        run: cargo clippy --all-targets -- -D warnings
      - name: Tests
        run: cargo test
```

- [ ] **Step 5: Verify locally** — `cargo clippy --all-targets -- -D warnings && cargo test -q 2>&1 | grep "test result" | head -1` — Expected: clippy silent, `72 passed`. `cargo metadata --no-deps --format-version 1 | python3 -c "import json,sys; p=json.load(sys.stdin)['packages'][0]; print(p['license'], p['repository'])"` — Expected: `MIT https://github.com/13/cst`.

- [ ] **Step 6: Commit** — `git add -A && git commit -m "build: MIT licence, package metadata, CI workflow"`

---

### Task 2: `install.sh` (install + uninstall) with a local test

**Files:**
- Create: `scripts/test-install.sh` (first), `install.sh`
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- `install.sh [--version vX.Y.Z] [--system] [--uninstall] [--help]`
- Test hooks (environment): `CST_INSTALL_BASE_URL` (default `https://github.com/13/cst/releases`; layout `<base>/latest/download/<asset>` and `<base>/download/<tag>/<asset>`), `CST_INSTALL_DIR` (default `~/.local/bin`).

- [ ] **Step 1: Write the failing test** `scripts/test-install.sh`:

```bash
#!/usr/bin/env bash
# Install cst from a locally built tarball into a throwaway HOME, check it,
# then uninstall. Usage: scripts/test-install.sh [path/to/cst]
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
bin="${1:-$root/target/release/cst}"
[ -x "$bin" ] || { echo "build first: cargo build --release" >&2; exit 1; }
fail() { echo "FAIL: $*" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
dl="$work/rel/latest/download"
mkdir -p "$dl" "$work/stage/cst-x86_64-linux" "$work/home"
cp "$bin" "$work/stage/cst-x86_64-linux/cst"
for f in README.md LICENSE; do [ -f "$root/$f" ] && cp "$root/$f" "$work/stage/cst-x86_64-linux/"; done
tar -czf "$dl/cst-x86_64-linux.tar.gz" -C "$work/stage" cst-x86_64-linux
(cd "$dl" && sha256sum cst-x86_64-linux.tar.gz > cst-x86_64-linux.tar.gz.sha256)

export HOME="$work/home" CST_INSTALL_BASE_URL="file://$work/rel"
export PATH="/usr/bin:/bin"          # target dir deliberately not on PATH
cd /

out="$(bash "$root/install.sh" 2>&1)" || fail "install exited non-zero: $out"
[ -x "$HOME/.local/bin/cst" ] || fail "binary not installed"
[[ "$("$HOME/.local/bin/cst" --version)" == cst\ * ]] || fail "--version"
[[ "$out" == *"not on your PATH"* ]] || fail "missing PATH warning: $out"

# a bad checksum aborts before installing anything
rm "$HOME/.local/bin/cst"
echo "0000000000000000000000000000000000000000000000000000000000000000  cst-x86_64-linux.tar.gz" > "$dl/cst-x86_64-linux.tar.gz.sha256"
if bash "$root/install.sh" >/dev/null 2>&1; then fail "bad checksum accepted"; fi
[ ! -e "$HOME/.local/bin/cst" ] || fail "installed despite bad checksum"
(cd "$dl" && sha256sum cst-x86_64-linux.tar.gz > cst-x86_64-linux.tar.gz.sha256)

# uninstall, then uninstall again (nothing there) must still succeed
bash "$root/install.sh" >/dev/null
bash "$root/install.sh" --uninstall >/dev/null || fail "uninstall exited non-zero"
[ ! -e "$HOME/.local/bin/cst" ] || fail "still installed after --uninstall"
out="$(bash "$root/install.sh" --uninstall 2>&1)" || fail "second uninstall exited non-zero"
[[ "$out" == *"no cst found"* ]] || fail "second uninstall message: $out"

# bad flag
if bash "$root/install.sh" --bogus >/dev/null 2>&1; then fail "--bogus accepted"; fi
echo "install/uninstall OK"
```

- [ ] **Step 2: Run it to see it fail** — `chmod +x scripts/test-install.sh && cargo build --release -q && scripts/test-install.sh` — Expected: `FAIL: install exited non-zero: bash: …/install.sh: No such file or directory`.

- [ ] **Step 3: Write `install.sh`**

```bash
#!/usr/bin/env bash
# cst installer — https://github.com/13/cst
#   curl -fsSL https://raw.githubusercontent.com/13/cst/main/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/13/cst/main/install.sh | bash -s -- --uninstall
set -euo pipefail

REPO="13/cst"
ASSET="cst-x86_64-linux.tar.gz"
BASE_URL="${CST_INSTALL_BASE_URL:-https://github.com/$REPO/releases}"

usage() {
  cat <<'EOF'
Install or remove cst, the keyboard shortcut cheatsheet.

  install.sh [--version vX.Y.Z] [--system]   install (default: latest release into ~/.local/bin)
  install.sh --uninstall                     remove ~/.local/bin/cst and /usr/local/bin/cst
  install.sh --help

  --system   install into /usr/local/bin (uses sudo)
EOF
}

info() { printf 'cst-install: %s\n' "$*"; }
die() { printf 'cst-install: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "needs '$1'"; }

version="latest"
system=0
action="install"
while [ $# -gt 0 ]; do
  case "$1" in
    --version) [ $# -ge 2 ] || die "--version needs a value"; version="$2"; shift 2 ;;
    --version=*) version="${1#*=}"; shift ;;
    --system) system=1; shift ;;
    --uninstall) action="uninstall"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown option '$1' (see --help)" ;;
  esac
done

user_dir="${CST_INSTALL_DIR:-$HOME/.local/bin}"
system_dir="/usr/local/bin"

as_root() {
  if [ "$(id -u)" = 0 ]; then "$@"; else sudo "$@"; fi
}

fetch() {
  if command -v curl >/dev/null 2>&1; then curl -fsSL "$1" -o "$2"
  elif command -v wget >/dev/null 2>&1; then wget -qO "$2" "$1"
  else die "needs curl or wget"
  fi
}

do_install() {
  [ "$(uname -s)" = Linux ] || die "only Linux is supported (this is $(uname -s))"
  [ "$(uname -m)" = x86_64 ] || die "only x86_64 is supported (this is $(uname -m))"
  need tar
  need sha256sum
  need install
  if [ -f /etc/arch-release ]; then
    info "on Arch you can also use the AUR (yay -S cst-bin) or the .pkg.tar.zst from the release page"
  fi
  local url dir tmp
  if [ "$version" = latest ]; then url="$BASE_URL/latest/download"; else url="$BASE_URL/download/$version"; fi
  if [ "$system" = 1 ]; then dir="$system_dir"; else dir="$user_dir"; fi
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  info "downloading $url/$ASSET"
  fetch "$url/$ASSET" "$tmp/$ASSET" || die "download failed: $url/$ASSET"
  fetch "$url/$ASSET.sha256" "$tmp/$ASSET.sha256" || die "download failed: $url/$ASSET.sha256"
  (cd "$tmp" && sha256sum -c --status "$ASSET.sha256") || die "checksum mismatch, nothing installed"
  tar -xzf "$tmp/$ASSET" -C "$tmp"
  if [ "$system" = 1 ]; then
    as_root install -Dm755 "$tmp/cst-x86_64-linux/cst" "$dir/cst"
  else
    install -Dm755 "$tmp/cst-x86_64-linux/cst" "$dir/cst"
  fi
  info "installed $("$dir/cst" --version) to $dir/cst"
  case ":$PATH:" in
    *":$dir:"*) ;;
    *) info "note: $dir is not on your PATH" ;;
  esac
}

do_uninstall() {
  local found=0 cur
  cur="$(command -v cst 2>/dev/null || true)"
  if [ -n "$cur" ] && command -v pacman >/dev/null 2>&1 && pacman -Qo "$cur" >/dev/null 2>&1; then
    info "$cur belongs to a pacman package; remove that with: sudo pacman -R cst-bin"
  fi
  if [ -f "$user_dir/cst" ]; then rm -f "$user_dir/cst"; info "removed $user_dir/cst"; found=1; fi
  if [ -f "$system_dir/cst" ]; then as_root rm -f "$system_dir/cst"; info "removed $system_dir/cst"; found=1; fi
  if [ "$found" = 0 ]; then info "no cst found in $user_dir or $system_dir"; fi
}

if [ "$action" = uninstall ]; then do_uninstall; else do_install; fi
```

- [ ] **Step 4: Run the test** — `chmod +x install.sh && scripts/test-install.sh` — Expected: `install/uninstall OK`. Then `shellcheck install.sh scripts/test-install.sh` — Expected: no output.

- [ ] **Step 5: CI** — append to the `test` job in `ci.yml`:

```yaml
      - name: Shellcheck
        run: shellcheck install.sh scripts/*.sh
      - name: Installer test
        run: cargo build --release && scripts/test-install.sh
```

- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat: install.sh for install and uninstall, with a local test"`

---

### Task 3: `scripts/release.sh` with a local test

**Files:**
- Create: `scripts/test-release.sh` (first), `scripts/release.sh`
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- `scripts/release.sh X.Y.Z [--dry-run]`; env `CST_RELEASE_SKIP_TESTS=1` skips `cargo test` (used only by the test script, which runs in a fresh clone).

- [ ] **Step 1: Write the failing test** `scripts/test-release.sh`:

```bash
#!/usr/bin/env bash
# Run release.sh in a throwaway clone pushing to a throwaway bare remote.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
fail() { echo "FAIL: $*" >&2; exit 1; }
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
git init -q --bare "$work/remote.git"
git clone -q "$root" "$work/clone"
cd "$work/clone"
git checkout -q -B main
cp "$root/scripts/release.sh" scripts/release.sh
git add scripts/release.sh
git -c user.name=t -c user.email=t@t commit -qm "test: current release.sh" || true
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
```

- [ ] **Step 2: Run it to see it fail** — `chmod +x scripts/test-release.sh && scripts/test-release.sh` — Expected: `cp: cannot stat '…/scripts/release.sh'` (non-zero exit).

- [ ] **Step 3: Write `scripts/release.sh`**

```bash
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
```

`cargo update --workspace --offline` rewrites only this package's entry in `Cargo.lock`, without touching the network.

- [ ] **Step 4: Run the test** — `chmod +x scripts/release.sh && scripts/test-release.sh` — Expected: `release script OK`. `shellcheck scripts/release.sh scripts/test-release.sh` — Expected: silent.

- [ ] **Step 5: CI** — append to `ci.yml` `test` job:

```yaml
      - name: Release script test
        run: scripts/test-release.sh
```

- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat: scripts/release.sh to cut a tagged version, with a local test"`

---

### Task 4: PKGBUILD template, release workflow, RELEASING.md

**Files:**
- Create: `packaging/aur/PKGBUILD.in`, `packaging/aur/render.sh`, `.github/workflows/release.yml`, `docs/RELEASING.md`

**Interfaces:**
- `packaging/aur/render.sh VERSION SHA256 > PKGBUILD`

- [ ] **Step 1: Failing check** — `packaging/aur/render.sh 0.1.0 abc` — Expected: `No such file or directory`.

- [ ] **Step 2: `packaging/aur/PKGBUILD.in`**

```bash
# Maintainer: ben <13@users.noreply.github.com>
pkgname=cst-bin
pkgver=@VERSION@
pkgrel=1
pkgdesc="Keyboard shortcut cheatsheet for your terminal apps (awesome, tmux, zellij, kitty, ...)"
arch=('x86_64')
url="https://github.com/13/cst"
license=('MIT')
provides=('cst')
conflicts=('cst')
source=("cst-x86_64-linux-$pkgver.tar.gz::$url/releases/download/v$pkgver/cst-x86_64-linux.tar.gz")
sha256sums=('@SHA256@')

package() {
  install -Dm755 cst-x86_64-linux/cst "$pkgdir/usr/bin/cst"
  install -Dm644 cst-x86_64-linux/LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
  install -Dm644 cst-x86_64-linux/README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
```

- [ ] **Step 3: `packaging/aur/render.sh`**

```bash
#!/usr/bin/env bash
# Render the cst-bin PKGBUILD: packaging/aur/render.sh VERSION SHA256 > PKGBUILD
set -euo pipefail
[ $# -eq 2 ] || { echo "usage: render.sh VERSION SHA256" >&2; exit 2; }
[[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "render.sh: bad version '$1'" >&2; exit 2; }
[[ "$2" =~ ^[0-9a-f]{64}$ ]] || { echo "render.sh: bad sha256 '$2'" >&2; exit 2; }
sed -e "s/@VERSION@/$1/" -e "s/@SHA256@/$2/" "$(dirname "$0")/PKGBUILD.in"
```

- [ ] **Step 4: Verify the package builds locally** (static musl binary, real makepkg):

```bash
chmod +x packaging/aur/render.sh
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
file target/x86_64-unknown-linux-musl/release/cst | grep -o "statically linked"
w=$(mktemp -d); mkdir -p "$w/cst-x86_64-linux"
cp target/x86_64-unknown-linux-musl/release/cst LICENSE "$w/cst-x86_64-linux/"
cp README.md "$w/cst-x86_64-linux/" 2>/dev/null || touch "$w/cst-x86_64-linux/README.md"   # README lands in Task 6
tar -czf "$w/cst-x86_64-linux-0.1.0.tar.gz" -C "$w" cst-x86_64-linux
packaging/aur/render.sh 0.1.0 "$(sha256sum "$w/cst-x86_64-linux-0.1.0.tar.gz" | cut -d' ' -f1)" > "$w/PKGBUILD"
(cd "$w" && makepkg -f --noconfirm >/dev/null && makepkg --printsrcinfo > .SRCINFO && ls ./*.pkg.tar.zst && tar -tf ./*.pkg.tar.zst | grep -E "usr/bin/cst|licenses" && grep -E "pkgver|sha256sums" .SRCINFO)
packaging/aur/render.sh 0.1 abc; echo "exit $?"
rm -rf "$w"
```

Expected: `statically linked`; `cst-bin-0.1.0-1-x86_64.pkg.tar.zst`; `usr/bin/cst` and the licence path; `.SRCINFO` with `pkgver = 0.1.0` and the hash; then `render.sh: bad version '0.1'` and `exit 2`. If `file` reports dynamic linking, the musl target is not self-contained here — install `musl` (`pacman -S musl`, ask the user first) and rebuild.

- [ ] **Step 5: `.github/workflows/release.yml`**

```yaml
name: release
on:
  push:
    tags: ["v*"]
permissions:
  contents: write
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-unknown-linux-musl
      - uses: Swatinem/rust-cache@v2
      - name: Tag matches Cargo.toml version
        run: |
          v="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
          if [ "v$v" != "$GITHUB_REF_NAME" ]; then
            echo "::error::tag $GITHUB_REF_NAME does not match Cargo.toml version $v"; exit 1
          fi
      - name: Tests
        run: cargo test
      - name: Build static binary
        run: |
          sudo apt-get update -q && sudo apt-get install -y -q musl-tools
          cargo build --release --target x86_64-unknown-linux-musl
      - name: Package
        run: |
          mkdir -p dist/cst-x86_64-linux
          cp target/x86_64-unknown-linux-musl/release/cst README.md LICENSE dist/cst-x86_64-linux/
          tar -czf dist/cst-x86_64-linux.tar.gz -C dist cst-x86_64-linux
          (cd dist && sha256sum cst-x86_64-linux.tar.gz > cst-x86_64-linux.tar.gz.sha256)
      - name: Create release
        env:
          GH_TOKEN: ${{ github.token }}
        run: gh release create "$GITHUB_REF_NAME" --title "cst $GITHUB_REF_NAME" --generate-notes dist/cst-x86_64-linux.tar.gz dist/cst-x86_64-linux.tar.gz.sha256

  pacman:
    needs: build
    runs-on: ubuntu-latest
    container: archlinux:base-devel
    steps:
      - uses: actions/checkout@v4
      - name: Build cst-bin package
        run: |
          pacman -Syu --noconfirm --needed github-cli
          v="${GITHUB_REF_NAME#v}"
          mkdir -p /tmp/pkg && cd /tmp/pkg
          curl -fsSL -o "cst-x86_64-linux-$v.tar.gz" "https://github.com/$GITHUB_REPOSITORY/releases/download/$GITHUB_REF_NAME/cst-x86_64-linux.tar.gz"
          "$GITHUB_WORKSPACE/packaging/aur/render.sh" "$v" "$(sha256sum "cst-x86_64-linux-$v.tar.gz" | cut -d' ' -f1)" > PKGBUILD
          useradd -m builder
          chown -R builder /tmp/pkg
          su builder -c 'cd /tmp/pkg && makepkg -f --noconfirm && makepkg --printsrcinfo > .SRCINFO'
      - name: Upload package
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          cd /tmp/pkg
          gh release upload "$GITHUB_REF_NAME" ./*.pkg.tar.zst PKGBUILD .SRCINFO --repo "$GITHUB_REPOSITORY"

  aur:
    needs: pacman
    runs-on: ubuntu-latest
    steps:
      - name: Push cst-bin to the AUR
        env:
          GH_TOKEN: ${{ github.token }}
          AUR_SSH_KEY: ${{ secrets.AUR_SSH_KEY }}
        run: |
          if [ -z "$AUR_SSH_KEY" ]; then
            echo "::warning::AUR_SSH_KEY is not set, skipping the AUR push (see docs/RELEASING.md)"; exit 0
          fi
          mkdir -p ~/.ssh
          printf '%s\n' "$AUR_SSH_KEY" > ~/.ssh/aur && chmod 600 ~/.ssh/aur
          ssh-keyscan -t ed25519,rsa aur.archlinux.org >> ~/.ssh/known_hosts 2>/dev/null
          export GIT_SSH_COMMAND="ssh -i ~/.ssh/aur -o IdentitiesOnly=yes"
          git clone -q ssh://aur@aur.archlinux.org/cst-bin.git aur
          gh release download "$GITHUB_REF_NAME" --repo "$GITHUB_REPOSITORY" -p PKGBUILD -p .SRCINFO -D aur --clobber
          cd aur
          git config user.name "cst release bot"
          git config user.email "13@users.noreply.github.com"
          git add PKGBUILD .SRCINFO
          if git diff --cached --quiet; then echo "AUR already up to date"; exit 0; fi
          git commit -qm "cst-bin ${GITHUB_REF_NAME#v}"
          git push -q origin HEAD:master
```

- [ ] **Step 6: Validate the workflows** with actionlint's static binary (PyYAML is not installed):

```bash
curl -fsSL https://raw.githubusercontent.com/rhysd/actionlint/main/scripts/download-actionlint.bash | bash -s -- latest ~/.local/bin >/dev/null
actionlint .github/workflows/*.yml && echo "workflows OK"
```

Expected: `workflows OK` (actionlint also shellchecks the `run:` blocks). Fix anything it reports.

- [ ] **Step 7: `docs/RELEASING.md`**

```markdown
# Releasing cst

## One-time setup (AUR)

1. Create an account on https://aur.archlinux.org and add an SSH public key
   to it (My Account → SSH Public Key). Use a dedicated key:
   `ssh-keygen -t ed25519 -f ~/.ssh/aur -C cst-aur -N ''`.
2. Register the package name once: `git -c core.sshCommand='ssh -i ~/.ssh/aur'
   clone ssh://aur@aur.archlinux.org/cst-bin.git` (an empty repo is fine).
3. Store the private key as a repository secret:
   `gh secret set AUR_SSH_KEY < ~/.ssh/aur`.

Without the secret, releases still work; the `aur` job only warns.

## Every release

    scripts/release.sh X.Y.Z          # --dry-run to preview

The script refuses unless you are on a clean `main` with passing tests and
an unused tag. It bumps `Cargo.toml`, commits `release vX.Y.Z`, tags
`vX.Y.Z` and pushes. The tag starts `.github/workflows/release.yml`:

1. **build** — checks tag = Cargo version, tests, builds the static musl
   binary, creates the GitHub release with `cst-x86_64-linux.tar.gz` and its
   `.sha256`.
2. **pacman** — builds `cst-bin-X.Y.Z-1-x86_64.pkg.tar.zst` from
   `packaging/aur/PKGBUILD.in` in an Arch container and attaches it, the
   PKGBUILD and `.SRCINFO` to the release.
3. **aur** — pushes PKGBUILD and `.SRCINFO` to `cst-bin` on the AUR.

A failed job can be re-run from the Actions tab (`gh run rerun <id>
--failed`); jobs are safe to re-run except `build`'s `gh release create`
(delete the release first: `gh release delete vX.Y.Z --yes`).
```

- [ ] **Step 8: Commit** — `git add -A && git commit -m "feat: release workflow, cst-bin PKGBUILD, releasing guide"`

---
### Task 5: Screenshot — `ansi2svg.py` and `screenshot.sh`

**Files:**
- Create: `scripts/ansi2svg.py`, `scripts/screenshot.sh`, `docs/screenshot.svg` (generated)
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- `scripts/ansi2svg.py [--bg '#rrggbb'] < capture > out.svg`; `scripts/ansi2svg.py --selftest`
- `scripts/screenshot.sh [cst args…]`; env `CST_SHOT_KEYS` (tmux key names sent after start), `CST_SHOT_COLS`/`CST_SHOT_ROWS` (default 140×40)

- [ ] **Step 1: Write the self-test first** — create `scripts/ansi2svg.py` containing only:

```python
#!/usr/bin/env python3
import sys


def selftest():
    svg = convert("\x1b[1;38;2;255;0;0mHi\x1b[0m x\n\x1b[48;5;196m 日 \x1b[0m", bg0="#000000")
    assert 'fill="#ff0000" font-weight="bold"' in svg, svg   # truecolour fg + bold
    assert ">Hi<" in svg and "> x<" in svg, svg
    assert 'fill="#ff0000"/>' in svg, svg                     # 256-colour 196 background rect
    assert 'width="33.6"' in svg, svg                          # " 日 " is 4 cells of 8.4px
    assert svg.startswith("<svg") and svg.rstrip().endswith("</svg>"), svg
    print("ansi2svg selftest OK")


if __name__ == "__main__":
    selftest()
```

- [ ] **Step 2: Run it** — `python3 scripts/ansi2svg.py --selftest` — Expected: `NameError: name 'convert' is not defined`.

- [ ] **Step 3: Implement** — replace the file with:

```python
#!/usr/bin/env python3
"""Turn a terminal capture with ANSI colours (tmux capture-pane -e -p) into
an SVG on a monospace grid. Standard library only.

    tmux capture-pane -e -p | scripts/ansi2svg.py --bg '#313244' > out.svg
    scripts/ansi2svg.py --selftest
"""
import re
import sys
import unicodedata
from html import escape

CW, LH, FS = 8.4, 18, 14  # cell width, line height, font size (px)
SGR = re.compile(r"\x1b\[([0-9;]*)m")
OTHER = re.compile(r"\x1b\[[0-9;?]*[A-Za-ln-z]|\x1b\][^\x07]*\x07|\x1b[()][A-Za-z0-9]")
BASE16 = ["000000", "800000", "008000", "808000", "000080", "800080", "008080", "c0c0c0",
          "808080", "ff0000", "00ff00", "ffff00", "0000ff", "ff00ff", "00ffff", "ffffff"]


def xterm256(n):
    if n < 16:
        return "#" + BASE16[n]
    if n < 232:
        n -= 16
        steps = [0, 95, 135, 175, 215, 255]
        return "#%02x%02x%02x" % (steps[n // 36], steps[n // 6 % 6], steps[n % 6])
    v = 8 + 10 * (n - 232)
    return "#%02x%02x%02x" % (v, v, v)


def cells(ch):
    if unicodedata.category(ch) in ("Mn", "Me", "Cf"):
        return 0
    return 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1


def apply_sgr(params, st, fg0):
    p = [int(x) if x else 0 for x in params.split(";")] if params else [0]
    i = 0
    while i < len(p):
        c = p[i]
        if c == 0:
            st.update(fg=fg0, bg=None, bold=False)
        elif c == 1:
            st["bold"] = True
        elif c == 22:
            st["bold"] = False
        elif c in (38, 48) and i + 1 < len(p):
            key = "fg" if c == 38 else "bg"
            if p[i + 1] == 2 and i + 4 < len(p):
                st[key] = "#%02x%02x%02x" % tuple(p[i + 2:i + 5])
                i += 4
            elif p[i + 1] == 5 and i + 2 < len(p):
                st[key] = xterm256(p[i + 2])
                i += 2
        elif c == 39:
            st["fg"] = fg0
        elif c == 49:
            st["bg"] = None
        elif 30 <= c <= 37:
            st["fg"] = xterm256(c - 30)
        elif 90 <= c <= 97:
            st["fg"] = xterm256(c - 90 + 8)
        elif 40 <= c <= 47:
            st["bg"] = xterm256(c - 40)
        i += 1


def convert(text, bg0="#1e1e2e", fg0="#cdd6f4"):
    lines = text.rstrip("\n").split("\n")
    st = {"fg": fg0, "bg": None, "bold": False}
    body, width = [], 0
    for y, line in enumerate(lines):
        col = 0
        for k, part in enumerate(SGR.split(OTHER.sub("", line))):
            if k % 2 == 1:
                apply_sgr(part, st, fg0)
                continue
            if not part:
                continue
            n = sum(cells(c) for c in part)
            x, top = col * CW, y * LH
            if st["bg"]:
                body.append(f'<rect x="{x:.1f}" y="{top}" width="{n * CW:.1f}" height="{LH}" fill="{st["bg"]}"/>')
            if part.strip():
                bold = ' font-weight="bold"' if st["bold"] else ""
                body.append(f'<text x="{x:.1f}" y="{top + LH - 5}" fill="{st["fg"]}"{bold} '
                            f'textLength="{n * CW:.1f}" lengthAdjust="spacingAndGlyphs">{escape(part)}</text>')
            col += n
        width = max(width, col)
    w, h = width * CW, len(lines) * LH
    font = "ui-monospace, 'JetBrains Mono', 'DejaVu Sans Mono', Menlo, monospace"
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w:.0f}" height="{h}" viewBox="0 0 {w:.0f} {h}" '
            f'font-family="{font}" font-size="{FS}" xml:space="preserve">\n'
            f'<rect width="100%" height="100%" rx="10" fill="{bg0}"/>\n' + "\n".join(body) + "\n</svg>\n")


def selftest():
    svg = convert("\x1b[1;38;2;255;0;0mHi\x1b[0m x\n\x1b[48;5;196m 日 \x1b[0m", bg0="#000000")
    assert 'fill="#ff0000" font-weight="bold"' in svg, svg   # truecolour fg + bold
    assert ">Hi<" in svg and "> x<" in svg, svg
    assert 'fill="#ff0000"/>' in svg, svg                     # 256-colour 196 background rect
    assert 'width="33.6"' in svg, svg                          # " 日 " is 4 cells of 8.4px
    assert svg.startswith("<svg") and svg.rstrip().endswith("</svg>"), svg
    print("ansi2svg selftest OK")


def main():
    args = sys.argv[1:]
    if "--selftest" in args:
        selftest()
        return
    bg = args[args.index("--bg") + 1] if "--bg" in args else "#1e1e2e"
    sys.stdout.write(convert(sys.stdin.read(), bg0=bg))


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run** — `chmod +x scripts/ansi2svg.py && python3 scripts/ansi2svg.py --selftest` — Expected: `ansi2svg selftest OK`.

- [ ] **Step 5: `scripts/screenshot.sh`**

```bash
#!/usr/bin/env bash
# Render the real TUI into docs/screenshot.svg (needs tmux and a release build).
#   scripts/screenshot.sh [cst args...]          e.g. scripts/screenshot.sh tmux
#   CST_SHOT_KEYS="Enter" scripts/screenshot.sh  keys sent after start (tmux names)
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
bin="$root/target/release/cst"
[ -x "$bin" ] || { echo "build first: cargo build --release" >&2; exit 1; }
sock="cstshot-$$"
cleanup() { tmux -L "$sock" kill-server 2>/dev/null || true; }
trap cleanup EXIT
tmux -L "$sock" -f /dev/null new-session -d -x "${CST_SHOT_COLS:-140}" -y "${CST_SHOT_ROWS:-40}" \
  env -u TMUX COLORTERM=truecolor "$bin" "$@"
sleep 1
if [ -n "${CST_SHOT_KEYS:-}" ]; then
  # shellcheck disable=SC2086 # a space-separated list of key names
  tmux -L "$sock" send-keys ${CST_SHOT_KEYS}
  sleep 0.5
fi
tmux -L "$sock" capture-pane -e -p | python3 "$root/scripts/ansi2svg.py" --bg "#313244" > "$root/docs/screenshot.svg"
echo "wrote docs/screenshot.svg"
```

- [ ] **Step 6: Generate and look at it** — `chmod +x scripts/screenshot.sh && cargo build --release -q && scripts/screenshot.sh && grep -c "<text" docs/screenshot.svg` — Expected: `wrote docs/screenshot.svg` and a count in the hundreds. Open it (`xdg-open docs/screenshot.svg` is not available headless — instead check with `python3 -c "import xml.dom.minidom,sys; xml.dom.minidom.parse('docs/screenshot.svg'); print('valid xml')"`) and read a few `<text>` elements to confirm the header and section titles are there. The screenshot shows the user's own keybindings and goes into a public README: list any entries that look private (hostnames, personal script names) to the user in the task report.

- [ ] **Step 7: CI** — append to the `test` job:

```yaml
      - name: ansi2svg selftest
        run: python3 scripts/ansi2svg.py --selftest
```

and `shellcheck scripts/screenshot.sh` (already covered by `scripts/*.sh`).

- [ ] **Step 8: Commit** — `git add -A && git commit -m "docs: screenshot pipeline (tmux capture → SVG)"`

---

### Task 6: README

**Files:**
- Create: `README.md`

- [ ] **Step 1: Check** — `test -f README.md; echo $?` — Expected: `1`.

- [ ] **Step 2: Write `README.md`**

````markdown
<div align="center">

# cst

**Every keyboard shortcut you have, on one screen.**

A fast terminal cheatsheet that reads the *live* keybindings of your window manager,
terminals, multiplexers and tools — custom ones included — in the style of an
awesome WM popup.

[![CI](https://github.com/13/cst/actions/workflows/ci.yml/badge.svg)](https://github.com/13/cst/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/13/cst)](https://github.com/13/cst/releases/latest)
[![AUR](https://img.shields.io/aur/version/cst-bin)](https://aur.archlinux.org/packages/cst-bin)
[![License: MIT](https://img.shields.io/github/license/13/cst)](LICENSE)

<img src="docs/screenshot.svg" alt="cst showing awesome, kitty, tmux and zellij shortcuts in columns" width="100%">

</div>

## Features

- **Your real bindings** — reads each app's config or asks the running app, then layers
  it over sensible defaults. Rebind something and `cst` shows the new key.
- **Type to filter** — rows that don't match dim instead of disappearing, so nothing jumps.
- **One app or all** — `cst tmux`, or Tab through apps inside the sheet.
- **Looks like your desktop** — colours come from your active awesome theme
  (falls back to Catppuccin); truecolor, 256 colours or `NO_COLOR`.
- **Fast and small** — one static binary, no runtime dependencies, starts in well under 100 ms.
- **Never breaks on a bad config** — a broken file shows a ⚠ note and falls back to defaults.

## Supported apps

| App | Where the keys come from |
|---|---|
| awesome | the running WM (`awesome-client`), else `~/.config/awesome/keys.lua` |
| kitty | `kitty.conf` (+ includes, `kitty_mod`) over kitty's defaults |
| wezterm | `wezterm show-keys` (your effective key tables) |
| alacritty | `alacritty.toml` (+ imports) over alacritty's defaults |
| tmux | the running server (`list-keys`, `-N` notes), else `~/.tmux.conf` over tmux's defaults |
| zellij | the `keybinds` block of `config.kdl` (honours `clear-defaults`) |
| nvim | mappings with a description (`nvim --headless`) over core motions |
| yazi | `keymap.toml` over yazi's defaults |
| lazygit | `keybinding:` in `config.yml` over lazygit's defaults |
| fzf | `--bind` in `FZF_DEFAULT_OPTS` / `FZF_DEFAULT_OPTS_FILE` |

Apps that aren't installed are skipped. `cst --list` shows what was found and where each
app's keys came from (`live`, `defaults` or `mixed`).

## Install

**Arch Linux (AUR)**

```sh
yay -S cst-bin        # or paru -S cst-bin
```

**Arch Linux (package from the release page)**

```sh
curl -LO https://github.com/13/cst/releases/latest/download/cst-bin-<version>-1-x86_64.pkg.tar.zst
sudo pacman -U cst-bin-*-x86_64.pkg.tar.zst
```

**Any x86_64 Linux (one line)**

```sh
curl -fsSL https://raw.githubusercontent.com/13/cst/main/install.sh | bash
```

Installs the static binary into `~/.local/bin` (`--system` for `/usr/local/bin`,
`--version v0.1.0` for a specific release). Uninstall with the same script:

```sh
curl -fsSL https://raw.githubusercontent.com/13/cst/main/install.sh | bash -s -- --uninstall
```

**From source**

```sh
cargo install --git https://github.com/13/cst
```

## Usage

```sh
cst            # all installed apps
cst tmux       # just one app
cst --list     # detected apps and where their keys come from
```

| Key | Action |
|---|---|
| type | filter (non-matching rows dim) |
| `Backspace` / `Ctrl+U` | delete a character / clear the filter |
| `Tab` / `Shift+Tab` | focus the next / previous app |
| `↑` `↓` `PgUp` `PgDn` `Home` `End` | scroll |
| `Esc` | clear the filter, or quit when it's empty |
| `Ctrl+C` | quit |

## Theming

`cst` reads the active theme of [awesome](https://awesomewm.org)
(`~/.cache/awesome/theme`, else `config.default_theme` in `config.lua`) and uses its
infocard colours, so the sheet matches your desktop's cheatsheet popup. Without awesome
it uses Catppuccin Mocha. Colours are truecolor when `COLORTERM=truecolor`, 256 colours
otherwise, and off when `NO_COLOR` is set.

## How it works

Each app is a *source*: it reads the live config (files, or commands like
`wezterm show-keys` with a 1 s timeout), turns every notation (`ctrl+shift+t`, `C-a`,
`<C-w>`, `Ctrl g`) into the same key names, and layers the result over bundled defaults —
a live binding replaces the default on the same key, an unbind removes it. All sources
load in parallel; one failing or slow source never blocks the others.

## Contributing

```sh
cargo test                       # unit tests, fixtures, layout snapshots
cargo clippy --all-targets -- -D warnings
scripts/test-install.sh          # installer (after cargo build --release)
```

Adding an app is one file in `src/sources/`, a defaults list in `src/defaults/` and a
fixture in `tests/fixtures/`; see `src/sources/kitty.rs` for a small example.
Releases: see [docs/RELEASING.md](docs/RELEASING.md).

## License

[MIT](LICENSE)
````

- [ ] **Step 3: Check links and paths** — `grep -o '](\(docs\|LICENSE\)[^)]*)' README.md | sort -u` — Expected: `](LICENSE)` and `](docs/RELEASING.md)`; both files exist (`ls LICENSE docs/RELEASING.md docs/screenshot.svg`).

- [ ] **Step 4: Commit** — `git add -A && git commit -m "docs: README"`

---

### Task 7: Create the GitHub repo, first CI run, release v0.1.0

**Files:** none (remote side effects the user asked for: create `13/cst`, push, tag `v0.1.0`).

- [ ] **Step 1: Create and push** —

```bash
gh repo create cst --public --source . --remote origin --push \
  --description "Keyboard shortcut cheatsheet for your terminal apps, in the style of an awesome WM popup"
gh repo edit 13/cst --add-topic cheatsheet,keybindings,tui,rust,awesomewm,tmux,zellij,kitty,wezterm
gh repo view 13/cst --json visibility,url -q '.visibility + " " + .url'
```

Expected: `PUBLIC https://github.com/13/cst`.

- [ ] **Step 2: First CI run green** — `sleep 5; gh run watch "$(gh run list -R 13/cst -w ci -L 1 --json databaseId -q '.[0].databaseId')" -R 13/cst --exit-status` — Expected: all steps ✓. On failure: `gh run view --log-failed`, fix (TDD where code is involved), push, re-watch.

- [ ] **Step 3: Release** — `scripts/release.sh 0.1.0` (Cargo version is already 0.1.0: tags without a commit) then watch: `sleep 5; gh run watch "$(gh run list -R 13/cst -w release -L 1 --json databaseId -q '.[0].databaseId')" -R 13/cst --exit-status`. Expected: build ✓, pacman ✓, aur ✓ with the warning "AUR_SSH_KEY is not set".

- [ ] **Step 4: Verify the assets** —

```bash
gh release view v0.1.0 -R 13/cst --json assets -q '.assets[].name' | sort
d=$(mktemp -d); cd "$d"
gh release download v0.1.0 -R 13/cst
sha256sum -c cst-x86_64-linux.tar.gz.sha256
tar -xzf cst-x86_64-linux.tar.gz && ./cst-x86_64-linux/cst --version && file cst-x86_64-linux/cst | grep -o "statically linked"
tar -tf cst-bin-0.1.0-1-x86_64.pkg.tar.zst | grep usr/bin/cst
cd - >/dev/null; rm -rf "$d"
```

Expected assets: `.SRCINFO`, `PKGBUILD`, `cst-bin-0.1.0-1-x86_64.pkg.tar.zst`, `cst-x86_64-linux.tar.gz`, `cst-x86_64-linux.tar.gz.sha256`; checksum OK; `cst 0.1.0`; statically linked; `usr/bin/cst`.

- [ ] **Step 5: Real one-line install** — `curl -fsSL https://raw.githubusercontent.com/13/cst/main/install.sh | bash && cst --version && cst --list | head -3` — Expected: `installed cst 0.1.0 to ~/.local/bin/cst` (replacing the locally built copy with the release build), then the list.

- [ ] **Step 6: Report** what the user still has to do once: the AUR setup in `docs/RELEASING.md` (account, key, `gh secret set AUR_SSH_KEY`), after which `gh run rerun <release run id> --job aur` publishes 0.1.0 to the AUR.
