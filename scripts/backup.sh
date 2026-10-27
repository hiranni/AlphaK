#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB="${ALPHAK_DB:-$ROOT/alphak.db}"
STAMP="$(date +%Y%m%d-%H%M%S)-$$"
OUT="${1:-$ROOT/backups/alphak-$STAMP.db}"
KIND="${ALPHAK_BACKUP_KIND:-manual}"
if [[ "$(basename "$OUT")" == pre-migrate-* ]]; then KIND=pre-migrate; fi
python3 "$ROOT/scripts/storage-backup.py" backup "$OUT" --db "$DB" --kind "$KIND"
