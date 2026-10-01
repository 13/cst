#!/usr/bin/env bash
# Render the cst-bin PKGBUILD: packaging/aur/render.sh VERSION SHA256 > PKGBUILD
set -euo pipefail
[ $# -eq 2 ] || { echo "usage: render.sh VERSION SHA256" >&2; exit 2; }
[[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "render.sh: bad version '$1'" >&2; exit 2; }
[[ "$2" =~ ^[0-9a-f]{64}$ ]] || { echo "render.sh: bad sha256 '$2'" >&2; exit 2; }
sed -e "s/@VERSION@/$1/" -e "s/@SHA256@/$2/" "$(dirname "$0")/PKGBUILD.in"
