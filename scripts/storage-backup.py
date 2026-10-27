#!/usr/bin/env python3
# coding: utf-8
"""Consistent SQLite backup plus immutable input objects. No credentials in manifest."""
import argparse
from datetime import datetime, timezone, timedelta
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import tempfile


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda: f.read(1024*1024), b''):
            h.update(block)
    return h.hexdigest()


def input_root(db):
    return Path(os.environ.get('ALPHAK_INPUT_DIR', str(db.with_suffix('.inputs'))))


def connect_ro(path):
    return sqlite3.connect(path.resolve().as_uri()+'?mode=ro', uri=True)


def check_db(path):
    with connect_ro(path) as c:
        if c.execute('PRAGMA integrity_check').fetchall() != [('ok',)]:
            raise ValueError('SQLite integrity check failed')
        names = {r[0] for r in c.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        if not {'instruments','quote_daily','financial_report','watchlist_item','app_setting'} <= names:
            raise ValueError('incompatible AlphaK schema')
        if 'version' not in {r[1] for r in c.execute('PRAGMA table_info(financial_report)')}:
            raise ValueError('incompatible financial report version')
        objects = list(c.execute('SELECT hash,relative_path,size_bytes FROM input_manifest')) if 'input_manifest' in names else []
        return objects


def make_backup(db, out, kind='manual'):
    if not db.is_file():
        raise ValueError('database not found')
    if out.exists() or out.with_suffix(out.suffix+'.manifest.json').exists():
        raise ValueError('backup destination exists; refusing overwrite')
    out.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.backup-', dir=out.parent) as temp:
        temp = Path(temp)
        snapshot = temp/'snapshot.db'
        with connect_ro(db) as source, sqlite3.connect(snapshot) as target:
            source.backup(target)
        objects = check_db(snapshot)
        package = temp/'inputs'
        package.mkdir()
        entries = []
        root = input_root(db)
        for h, relative, size in objects:
            if len(h) != 64 or any(c not in '0123456789abcdef' for c in h) or relative != h:
                raise ValueError('invalid content-addressed input manifest')
            src = root/h
            if not src.is_file() or src.stat().st_size != size or digest(src) != h:
                raise ValueError('referenced input missing or checksum mismatch')
            shutil.copyfile(src, package/h)
            entries.append({'sha256':h,'size_bytes':size})
        checksum = digest(snapshot)
        manifest = {'format':1,'created_at':datetime.now(timezone.utc).isoformat(),'kind':kind,'db_sha256':checksum,'input_objects':entries,'verified':True}
        # Publish manifest last. Interrupted unpublished files are not valid recovery points.
        os.replace(package, Path(str(out)+'.inputs'))
        os.replace(snapshot, out)
        Path(str(out)+'.sha256').write_text(f'{checksum}  {out.name}\n')
        Path(str(out)+'.manifest.json').write_text(json.dumps(manifest, indent=2)+'\n')
    return manifest


def verify_backup(path):
    manifest_path = Path(str(path)+'.manifest.json')
    checksum_path = Path(str(path)+'.sha256')
    if not checksum_path.is_file() or digest(path) != checksum_path.read_text().split()[0]:
        raise ValueError('backup checksum mismatch')
    objects = check_db(path)
    if manifest_path.exists():
        manifest = json.loads(manifest_path.read_text())
        if not manifest.get('verified') or manifest['db_sha256'] != digest(path):
            raise ValueError('backup manifest mismatch')
        expected = {o['sha256']:o['size_bytes'] for o in manifest['input_objects']}
        for h, relative, size in objects:
            p = Path(str(path)+'.inputs')/h
            if relative != h or expected.get(h) != size or not p.is_file() or p.stat().st_size != size or digest(p) != h:
                raise ValueError('input backup missing or corrupt')
        return manifest
    if objects:
        raise ValueError('legacy backup lacks referenced input objects')
    return {'kind':'manual','verified':True}


def rotate(directory, now=None):
    now = now or datetime.now(timezone.utc)
    valid = []
    errors = []
    for sidecar in directory.glob('*.db.manifest.json'):
        path = Path(str(sidecar).removesuffix('.manifest.json'))
        try:
            m = verify_backup(path)
            when = datetime.fromisoformat(m['created_at'])
            valid.append((when,path,m))
        except (OSError,ValueError,KeyError) as e:
            errors.append({'file':str(path),'error':str(e)})
    valid.sort(reverse=True)
    keep = set()
    if valid:
        keep.add(valid[0][1])  # Last verified recovery point is unconditional.
    day_keys = set()
    week_keys = set()
    for when,path,m in valid:
        if m.get('kind') != 'auto' or Path(str(path)+'.pin').exists():
            keep.add(path)
            continue
        day = when.date()
        week = when.isocalendar()[:2]
        if now-when <= timedelta(days=7) and day not in day_keys:
            keep.add(path); day_keys.add(day)
        if week not in week_keys and len(week_keys)<4:
            keep.add(path); week_keys.add(week)
    removed=[]
    if not errors:  # Failed verification never causes loss of an older good backup.
        for _,path,m in valid:
            if path in keep:
                continue
            # Migration and pre-restore backups are manual/protected above.
            for suffix in ['.manifest.json','.sha256']:
                Path(str(path)+suffix).unlink(missing_ok=True)
            shutil.rmtree(str(path)+'.inputs',ignore_errors=True)
            path.unlink()
            removed.append(str(path))
    return {'removed':removed,'verification_errors':errors,'kept':len(valid)-len(removed)}


def assert_stopped(db):
    # Refuse restore while an API process has the target database open.
    import subprocess
    result = subprocess.run(['lsof','-t','--',str(db)],capture_output=True,text=True)
    if result.returncode==0 and result.stdout.strip():
        raise ValueError('database is open; stop AlphaK before restore')
    if result.returncode not in (0,1):
        raise ValueError('could not confirm database is stopped')


def restore(db, backup, stopped_check=True):
    if stopped_check:
        assert_stopped(db)
    verify_backup(backup)
    safe = None
    if db.exists():
        safe = db.parent/'backups'/f'pre-restore-{datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S-%f")}.db'
        make_backup(db,safe,'pre-restore')
    root = input_root(db)
    root.mkdir(parents=True,exist_ok=True)
    # Immutable objects can be copied before DB swap. Existing objects are checked, never overwritten silently.
    for h,_,_ in check_db(backup):
        target=root/h; src=Path(str(backup)+'.inputs')/h
        if target.exists() and digest(target)!=h:
            raise ValueError('existing input object corrupt; refusing overwrite')
        if not target.exists():
            shutil.copyfile(src,target)
    try:
        with connect_ro(backup) as source,sqlite3.connect(db) as target:
            source.backup(target)
            # Never resume deletion using restored old policy settings.
            target.execute("INSERT OR REPLACE INTO app_setting(key,value) VALUES('retention.auto_cleanup','false')")
            target.execute("INSERT OR REPLACE INTO app_setting(key,value) VALUES('retention.maintenance','true')")
            target.commit()
        check_db(db)
    except Exception:
        if safe:
            with connect_ro(safe) as source,sqlite3.connect(db) as target:
                source.backup(target)
        raise
    return {'restored':str(db),'pre_restore':str(safe) if safe else None,'maintenance':True}


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('action',choices=['backup','verify','rotate','restore'])
    ap.add_argument('path',type=Path)
    ap.add_argument('--db',type=Path,default=Path(os.environ.get('ALPHAK_DB','alphak.db')))
    ap.add_argument('--kind',choices=['auto','manual','pre-migrate','pre-restore'],default='manual')
    args=ap.parse_args()
    if args.action=='backup':result=make_backup(args.db,args.path,args.kind)
    elif args.action=='verify':result=verify_backup(args.path)
    elif args.action=='rotate':result=rotate(args.path)
    else:result=restore(args.db,args.path)
    print(json.dumps(result,ensure_ascii=False,indent=2))

if __name__=='__main__':
    main()
