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
