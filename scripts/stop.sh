#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -x "$ROOT/bin/alphak-api" ]]; then
  APP="$ROOT"
else
  APP="$ROOT/dist/alphak"
fi
PIDFILE="$APP/run/alphak.pid"
mkdir -p "$APP/run"
LOCK="$APP/run/lifecycle.lock"
if ! mkdir "$LOCK" 2>/dev/null; then
  echo "Another start/stop operation is running: $LOCK" >&2
  exit 1
fi
trap 'rmdir "$LOCK"' EXIT
if [[ ! -f "$PIDFILE" ]]; then
  echo "AlphaK is not running (no PID file)."
  exit 0
fi
PID="$(cat "$PIDFILE")"
if [[ ! "$PID" =~ ^[0-9]+$ ]]; then
  echo "Invalid PID file: $PIDFILE" >&2
  exit 1
fi
if ! kill -0 "$PID" 2>/dev/null; then
  rm -f "$PIDFILE"
  echo "AlphaK already stopped; removed stale PID file."
  exit 0
fi
if [[ "$(ps -p "$PID" -o command=)" != *"$APP/bin/alphak-api"* ]]; then
  echo "PID $PID belongs to another process; refusing to stop it." >&2
  exit 1
fi
kill -TERM "$PID"
for ((attempt=0; attempt<100; attempt++)); do
  if ! kill -0 "$PID" 2>/dev/null || [[ "$(ps -p "$PID" -o stat=)" == *Z* ]]; then
    rm -f "$PIDFILE"
    echo "AlphaK stopped."
    exit 0
  fi
  sleep 0.1
done
echo "AlphaK has not stopped yet; PID file retained. See $APP/logs/alphak.log" >&2
exit 1
