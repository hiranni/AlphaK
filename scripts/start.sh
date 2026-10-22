#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# The same scripts work from the checkout and from a copied release directory.
if [[ -x "$ROOT/bin/alphak-api" ]]; then
  APP="$ROOT"
else
  APP="$ROOT/dist/alphak"
fi
BIN="$APP/bin/alphak-api"
if [[ ! -x "$BIN" || ! -f "$APP/web/index.html" ]]; then
  echo "Application package missing; run make build first." >&2
  exit 1
fi
mkdir -p "$APP/run" "$APP/logs"
LOCK="$APP/run/lifecycle.lock"
if ! mkdir "$LOCK" 2>/dev/null; then
  echo "Another start/stop operation is running: $LOCK" >&2
  exit 1
fi
trap 'rmdir "$LOCK"' EXIT
PIDFILE="$APP/run/alphak.pid"
if [[ -f "$PIDFILE" ]]; then
  PID="$(cat "$PIDFILE")"
  if [[ "$PID" =~ ^[0-9]+$ ]] && kill -0 "$PID" 2>/dev/null; then
    if [[ "$(ps -p "$PID" -o command=)" == *"$BIN"* ]]; then
      echo "AlphaK already running (PID $PID)."
      exit 0
    fi
    echo "PID file belongs to another process; refusing to overwrite it." >&2
    exit 1
  fi
  rm -f "$PIDFILE"
fi
# Keep local data/config at the checkout root when launched from the checkout.
cd "$ROOT"
LOG="$APP/logs/alphak.log"
nohup env ALPHAK_WEB_DIR="$APP/web" ALPHAK_BACKUP_BEFORE_START=1 "$BIN" > "$LOG" 2>&1 < /dev/null &
PID=$!
printf '%s\n' "$PID" > "$PIDFILE"
for ((attempt=0; attempt<100; attempt++)); do
  if ! kill -0 "$PID" 2>/dev/null; then
    rm -f "$PIDFILE"
    echo "AlphaK failed to start. See $LOG" >&2
    cat "$LOG" >&2
    exit 1
  fi
  if grep -q 'AlphaK API listening on' "$LOG"; then
    echo "AlphaK started (PID $PID)."
    grep 'AlphaK API listening on' "$LOG"
    echo "Log: $LOG"
    exit 0
  fi
  sleep 0.1
done
kill "$PID" 2>/dev/null || true
rm -f "$PIDFILE"
echo "AlphaK startup timed out. See $LOG" >&2
exit 1
