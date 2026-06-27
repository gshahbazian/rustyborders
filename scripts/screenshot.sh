#!/usr/bin/env bash
#
# Manual visual-verification harness for rustyborders.
#
# Launches two deterministic subject windows (scripts/subject_window.swift) side
# by side, makes the right-hand one frontmost, starts rustyborders, captures the
# WHOLE display, and leaves a PNG on disk for you (and Claude) to eyeball.
#
# Capturing the full display lets us confirm the border is drawn ONLY around the
# frontmost (active) window and that the inactive window is left untouched.
#
# This is NOT a pass/fail test. It needs a logged-in GUI session and Screen
# Recording permission for your terminal (macOS will prompt on first run).
#
# Usage:
#   scripts/screenshot.sh [border-args...]
#
# Examples:
#   scripts/screenshot.sh
#   scripts/screenshot.sh width=10 'active_color=oklch(84% 0.32 150 / 1)'
#
# Any extra args are passed straight through to rustyborders, overriding the
# defaults below (active_color/width). Note: inactive_color is intentionally
# left at its transparent default so inactive windows stay borderless.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

OUT="$REPO_ROOT/border-screenshot.png"
SUBJECT_OUT="$(mktemp -t rustyborders-subject)"

# --- Default border settings (override by passing args to this script) -------
DEFAULT_ARGS=(
  "width=8"
  "active_color=0xff00ff00"
)
BORDER_ARGS=("${DEFAULT_ARGS[@]}")
if [ "$#" -gt 0 ]; then
  BORDER_ARGS=("$@")
fi

# --- Cleanup on exit ---------------------------------------------------------
cleanup() {
  [ -n "${BORDERS_PID:-}" ] && kill "$BORDERS_PID" 2>/dev/null || true
  [ -n "${SUBJECT_PID:-}" ] && kill "$SUBJECT_PID" 2>/dev/null || true
  pkill -x rustyborders 2>/dev/null || true
  rm -f "$SUBJECT_OUT" 2>/dev/null || true
}
trap cleanup EXIT

echo "==> Building rustyborders (release)…"
cargo build --release --quiet
BIN="$REPO_ROOT/target/release/rustyborders"

echo "==> Stopping any existing rustyborders instance…"
pkill -x rustyborders 2>/dev/null || true
sleep 0.5

echo "==> Launching two subject windows (active = right)…"
swift "$REPO_ROOT/scripts/subject_window.swift" >"$SUBJECT_OUT" 2>/dev/null &
SUBJECT_PID=$!

# Wait for the subject windows to be on screen.
READY=""
for _ in $(seq 1 50); do
  READY="$(grep '^READY' "$SUBJECT_OUT" 2>/dev/null || true)"
  [ -n "$READY" ] && break
  sleep 0.2
done
if [ -z "$READY" ]; then
  echo "error: subject windows did not come up" >&2
  exit 1
fi
sleep 0.5

echo "==> Starting rustyborders: ${BORDER_ARGS[*]}"
"$BIN" "${BORDER_ARGS[@]}" &
BORDERS_PID=$!
sleep 1.5

echo "==> Capturing full display → $OUT"
screencapture -x "$OUT"

echo ""
echo "Done. Screenshot written to:"
echo "  $OUT"
echo ""
echo "Open it with:  open \"$OUT\""
