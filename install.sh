#!/usr/bin/env bash
# cst installer — https://github.com/13/cst
#   curl -fsSL https://raw.githubusercontent.com/13/cst/main/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/13/cst/main/install.sh | bash -s -- --uninstall
set -euo pipefail

REPO="13/cst"
ASSET="cst-x86_64-linux.tar.gz"
BASE_URL="${CST_INSTALL_BASE_URL:-https://github.com/$REPO/releases}"

usage() {
  cat <<'USAGE'
Install or remove cst, the keyboard shortcut cheatsheet.

  install.sh [--version vX.Y.Z] [--system]   install (default: latest release into ~/.local/bin)
  install.sh --uninstall                     remove ~/.local/bin/cst and /usr/local/bin/cst
  install.sh --help

  --system   install into /usr/local/bin (uses sudo)
USAGE
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
  local url dir
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
