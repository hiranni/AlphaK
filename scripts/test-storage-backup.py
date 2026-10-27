#!/usr/bin/env python3
import importlib.util
from pathlib import Path
import sqlite3
import tempfile
import unittest
from datetime import datetime, timezone, timedelta

spec=importlib.util.spec_from_file_location('storage',Path(__file__).with_name('storage-backup.py'))
s=importlib.util.module_from_spec(spec);spec.loader.exec_module(s)

class BackupTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.root=Path(self.temp.name);self.db=self.root/'alphak.db'
        with sqlite3.connect(self.db) as c:
            for table in ['instruments','quote_daily','watchlist_item']:
                c.execute(f'CREATE TABLE {table}(id TEXT)')
                c.execute(f"INSERT INTO {table} VALUES('preserved')")
            c.execute('CREATE TABLE financial_report(version INTEGER)')
            c.execute('CREATE TABLE app_setting(key TEXT PRIMARY KEY,value TEXT)')
            c.execute('CREATE TABLE input_manifest(hash TEXT PRIMARY KEY,relative_path TEXT,size_bytes INTEGER)')
        self.inputs=self.db.with_suffix('.inputs');self.inputs.mkdir()
        self.h=s.hashlib.sha256(b'frozen input').hexdigest()
        (self.inputs/self.h).write_bytes(b'frozen input')
        with sqlite3.connect(self.db) as c:c.execute('INSERT INTO input_manifest VALUES(?,?,?)',(self.h,self.h,12))
    def tearDown(self):self.temp.cleanup()
    def test_backup_restore_preserves_inputs_and_enters_maintenance(self):
        backup=self.root/'manual.db';s.make_backup(self.db,backup)
        with sqlite3.connect(self.db) as c:c.execute('DELETE FROM watchlist_item')
        s.restore(self.db,backup,stopped_check=False)
        with sqlite3.connect(self.db) as c:
            self.assertEqual(c.execute('SELECT COUNT(*) FROM watchlist_item').fetchone()[0],1)
            self.assertEqual(c.execute("SELECT value FROM app_setting WHERE key='retention.maintenance'").fetchone()[0],'true')
        self.assertEqual((self.inputs/self.h).read_bytes(),b'frozen input')
    def test_corruption_missing_inputs_and_destination_are_rejected(self):
        backup=self.root/'manual.db';s.make_backup(self.db,backup)
        with self.assertRaises(ValueError):s.make_backup(self.db,backup)
        Path(str(backup)+'.inputs',self.h).unlink()
        with self.assertRaises(ValueError):s.verify_backup(backup)
        self.assertTrue(self.db.exists())
    def test_failed_new_backup_never_rotates_older_valid_backups(self):
        a=self.root/'a.db';b=self.root/'b.db';s.make_backup(self.db,a,'auto');s.make_backup(self.db,b,'auto')
        b.write_bytes(b'corrupt')
        result=s.rotate(self.root,datetime.now(timezone.utc)+timedelta(days=100))
        self.assertTrue(result['verification_errors']);self.assertTrue(a.exists())
    def test_rotation_preserves_manual_pin_and_last_verified(self):
        import json
        now=datetime.now(timezone.utc)
        for i in range(40):
            path=self.root/f'{i}.db';s.make_backup(self.db,path,'manual' if i==0 else 'auto')
            side=Path(str(path)+'.manifest.json');m=json.loads(side.read_text());m['created_at']=(now-timedelta(days=i)).isoformat();side.write_text(json.dumps(m))
        Path(str(self.root/'39.db')+'.pin').touch()
        result=s.rotate(self.root,now)
        self.assertTrue(result['removed']);self.assertTrue((self.root/'0.db').exists());self.assertTrue((self.root/'39.db').exists());self.assertTrue((self.root/'1.db').exists())
    def test_missing_input_prevents_publishing_backup(self):
        (self.inputs/self.h).unlink();out=self.root/'broken.db'
        with self.assertRaises(ValueError):s.make_backup(self.db,out)
        self.assertFalse(out.exists())

if __name__=='__main__':unittest.main()
