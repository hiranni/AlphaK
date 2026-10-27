#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB="${ALPHAK_DB:-$ROOT/alphak.db}"
if [[ $# -ne 1 ]]; then echo 'usage: scripts/restore.sh <backup.db>' >&2; exit 1; fi
python3 "$ROOT/scripts/storage-backup.py" restore "$1" --db "$DB"
