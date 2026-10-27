#!/usr/bin/env python3
# coding: utf-8
"""Migrate a disposable online copy twice; never mutate the source database."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile

ROOT=Path(__file__).resolve().parents[1]

def snapshot(c):
    result={}
    tables=[r[0] for r in c.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")]
    for table in tables:
        if table in ['app_setting','data_source_config','live_cache','market_snapshot','quote_staging','data_job','source_retry_task','quality_report','data_freshness','sync_checkpoint']:
            continue
        cols=[r[1] for r in c.execute(f'PRAGMA table_info("{table}")')]
        query=f'SELECT {",".join(cols)} FROM "{table}"'
        # Legacy startup removes only demo holiday rows; they are not research evidence.
        if table in ['quote_daily','index_quote'] and 'source' in cols:query+=" WHERE source <> 'demo'"
        rows=sorted(c.execute(query).fetchall(),key=repr)
        result[table]={'columns':cols,'rows':len(rows),'sha256':hashlib.sha256(repr(rows).encode()).hexdigest()}
    return result

def compare(c,baseline):
    for table,expected in baseline.items():
        rows=sorted(c.execute(f'SELECT {",".join(expected["columns"])} FROM "{table}"'+(" WHERE source <> 'demo'" if table in ['quote_daily','index_quote'] and 'source' in expected['columns'] else '')).fetchall(),key=repr)
        actual=hashlib.sha256(repr(rows).encode()).hexdigest()
        if actual!=expected['sha256']:raise ValueError(f'{table}: existing data changed during migration')

ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('database',type=Path);ap.add_argument('--report',type=Path,required=True);args=ap.parse_args()
with tempfile.TemporaryDirectory(prefix='alphak-migration-') as temp:
    target=Path(temp)/'copy.db'
    with sqlite3.connect(args.database.resolve().as_uri()+'?mode=ro',uri=True) as source,sqlite3.connect(target) as dest:source.backup(dest)
    with sqlite3.connect(target) as c:baseline=snapshot(c)
    reports=[]
    for n in range(2):
        process=subprocess.run(['cargo','run','--quiet','--locked','--manifest-path',str(ROOT/'apps/api/Cargo.toml'),'--','--migrate-only'],cwd=ROOT,env={**os.environ,'ALPHAK_DB':str(target)},check=True,capture_output=True,text=True)
        with sqlite3.connect(target) as c:
            compare(c,baseline)
            assert c.execute('PRAGMA integrity_check').fetchall()==[('ok',)]
            assert not c.execute('PRAGMA foreign_key_check').fetchall()
        reports.append(json.loads(process.stdout))
    args.report.parent.mkdir(parents=True,exist_ok=True)
    args.report.write_text(json.dumps({'source':'online read-only copy','runs':2,'existing_data_checks':baseline,'data_preserved':True,'integrity':'ok','foreign_keys':'ok','diagnostics':reports[-1]},indent=2)+'\n')
print('Two migration passes preserved existing rows and hashes; integrity and FK checks passed.')
