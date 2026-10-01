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
