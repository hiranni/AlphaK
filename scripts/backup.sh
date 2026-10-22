#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB="${ALPHAK_DB:-$ROOT/alphak.db}"
STAMP="$(date +%Y%m%d-%H%M%S)"
OUT="${1:-$ROOT/backups/alphak-$STAMP.db}"

if [[ ! -f "$DB" ]]; then
  echo "database not found: $DB" >&2
  exit 1
fi

mkdir -p "$(dirname "$OUT")"
sqlite3 "$DB" ".backup '$OUT'"
SUM="$(shasum -a 256 "$OUT" | awk '{print $1}')"
printf '%s  %s\n' "$SUM" "$(basename "$OUT")" > "$OUT.sha256"
echo "backup: $OUT"
echo "sha256: $SUM"
