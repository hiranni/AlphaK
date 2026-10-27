#!/usr/bin/env python3
"""Exercise backup/restore on an online copy; never modify the source DB."""
import argparse
import hashlib
import importlib.util
import json
import os
import shutil
from pathlib import Path
import sqlite3
import tempfile

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('storage_backup', ROOT/'scripts/storage-backup.py')
backup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(backup)


def fingerprints(path):
    with backup.connect_ro(path) as c:
        result = {}
        for (name,) in c.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"):
            # Restore deliberately changes the maintenance/autocleanup preferences.
            query = f'SELECT * FROM "{name}"'
            if name == 'app_setting':
                query += " WHERE key NOT IN ('retention.maintenance','retention.auto_cleanup')"
            rows = sorted(c.execute(query).fetchall(), key=repr)
            result[name] = {'rows':len(rows), 'sha256':hashlib.sha256(repr(rows).encode()).hexdigest()}
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('database', type=Path)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='alphak-restore-') as directory:
        directory = Path(directory)
        db = directory/'copy.db'
        with backup.connect_ro(args.database) as source, sqlite3.connect(db) as target:
            source.backup(target)
        source.close()
        target.close()
        # Copy immutable inputs through the same verified package used by operators.
        recovery = directory/'source.db'
        backup.make_backup(args.database, recovery)
        os.environ["ALPHAK_INPUT_DIR"] = str(directory/"isolated.inputs")
        shutil.copytree(Path(str(recovery)+".inputs"), backup.input_root(db))
        backup.restore(db, recovery)
        before = fingerprints(db)
        recovery = directory/'drill.db'
        backup.make_backup(db, recovery)
        with sqlite3.connect(db) as c:
            c.execute("UPDATE app_setting SET value='drill' WHERE key NOT LIKE 'retention.%'")
            c.commit()
        c.close()
        result = backup.restore(db, recovery)
        assert fingerprints(db) == before, 'restored table hashes differ'
        assert result['pre_restore'] and backup.verify_backup(Path(result['pre_restore']))
        with backup.connect_ro(db) as c:
            assert c.execute('PRAGMA integrity_check').fetchall() == [('ok',)]
            assert not c.execute('PRAGMA foreign_key_check').fetchall()
            assert c.execute("SELECT value FROM app_setting WHERE key='retention.auto_cleanup'").fetchone() == ('false',)
            assert c.execute("SELECT value FROM app_setting WHERE key='retention.maintenance'").fetchone() == ('true',)
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps({'source':'read-only online copy','data_preserved':True,'integrity':'ok','foreign_keys':'ok','maintenance':True,'auto_cleanup':False,'pre_restore_backup_verified':True,'table_checks':before}, indent=2)+'\n')
    print('Restore drill passed on disposable database; original database unchanged.')


if __name__ == '__main__':
    main()
