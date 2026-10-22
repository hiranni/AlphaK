#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB="${ALPHAK_DB:-$ROOT/alphak.db}"
BACKUP="${1:-}"

if [[ -z "$BACKUP" || ! -f "$BACKUP" ]]; then
  echo "usage: scripts/restore.sh <backup.db>" >&2
  exit 1
fi
if [[ ! -f "$BACKUP.sha256" ]]; then
  echo "missing checksum sidecar: $BACKUP.sha256; refusing to restore" >&2
  exit 2
fi
EXPECTED="$(awk '{print $1}' "$BACKUP.sha256")"
ACTUAL="$(shasum -a 256 "$BACKUP" | awk '{print $1}')"
if [[ -z "$EXPECTED" || "$EXPECTED" != "$ACTUAL" ]]; then
  echo "checksum mismatch; refusing to restore" >&2
  exit 2
fi

schema_ok() {
  local file="$1"
  [[ -f "$file" ]] || return 1
  local tables
  tables="$(sqlite3 "$file" "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('instruments','quote_daily','financial_report','watchlist_item','app_setting');" 2>/dev/null || echo 0)"
  [[ "$tables" -eq 5 ]] || return 1
  local version_column
  version_column="$(sqlite3 "$file" "SELECT COUNT(*) FROM pragma_table_info('financial_report') WHERE name='version';" 2>/dev/null || echo 0)"
  [[ "$version_column" -eq 1 ]] || return 1
}

if ! sqlite3 "$BACKUP" "PRAGMA integrity_check;" | grep -q '^ok$'; then
  echo "sqlite integrity check failed; refusing to restore" >&2
  exit 2
fi
if ! schema_ok "$BACKUP"; then
  echo "backup is not a compatible AlphaK database; refusing to restore" >&2
  exit 2
fi

SAFE=""
if [[ -f "$DB" ]]; then
  SAFE="$ROOT/backups/pre-restore-$(date +%Y%m%d-%H%M%S).db"
  mkdir -p "$ROOT/backups"
  sqlite3 "$DB" ".backup '$SAFE'"
  echo "current database backed up: $SAFE"
fi

sqlite3 "$BACKUP" ".backup '$DB'"
if ! sqlite3 "$DB" "PRAGMA integrity_check;" | grep -q '^ok$' || ! schema_ok "$DB"; then
  echo "restored database failed application schema check" >&2
  if [[ -n "$SAFE" && -f "$SAFE" ]]; then
    sqlite3 "$SAFE" ".backup '$DB'"
    echo "rolled back to pre-restore backup" >&2
  fi
  exit 3
fi
echo "restored: $DB"
