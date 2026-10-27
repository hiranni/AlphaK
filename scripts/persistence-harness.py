#!/usr/bin/env python3
"""Validate live migrated SQLite schema and retention policies against design.md.

--update-schema updates only the structural table; lifecycle prose must be reviewed too.
Never connects to the user's database. The Rust probe uses an in-memory database.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
DESIGN = ROOT / 'openspec/changes/data-retention-and-sqlite-performance/design.md'
POLICIES = ROOT / 'apps/api/src/persistence-policies.json'
BEGIN = '<!-- persistence-schema:start -->'
END = '<!-- persistence-schema:end -->'


def render(schema, policy_bytes):
    policies = json.loads(policy_bytes)
    names = set(schema)
    registered = set(policies['tables'])
    if names != registered:
        raise ValueError(f'policy registry mismatch: missing={sorted(names-registered)}, extra={sorted(registered-names)}')
    rows = [BEGIN, f'政策文件 SHA256：`{hashlib.sha256(policy_bytes).hexdigest()}`', '',
            '| 实际表名 | 实际字段（迁移完成后） | schema SHA256 |', '|---|---|---|']
    for name, item in sorted(schema.items()):
        rows.append(f"| {name} | {', '.join(item['columns'])} | `{item['sha256']}` |")
    rows.append(END)
    return '\n'.join(rows)


def verify(document, expected):
    if BEGIN not in document or END not in document:
        raise ValueError('design.md 缺少持久化结构表')
    actual = document.split(BEGIN, 1)[1].split(END, 1)[0]
    wanted = expected.split(BEGIN, 1)[1].split(END, 1)[0]
    if actual != wanted:
        raise ValueError('持久化结构或政策发生变化，必须更新 design.md 对应生命周期说明及结构表；然后运行 make persistence-doc-update 并 review 文档差异')
    # Each real table must also have human-readable lifecycle documentation.
    prefix = document.split(BEGIN, 1)[0]
    for line in expected.splitlines():
        if not line.startswith('| ') or ' | `' not in line:
            continue
        name = line.split('|')[1].strip()
        if f'| {name} |' not in prefix and f'：{name}' not in prefix and f'/ {name}' not in prefix:
            raise ValueError(f'{name} 缺少人工编写的生命周期说明')


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--update-schema', action='store_true')
    ap.add_argument('--self-test', action='store_true')
    args = ap.parse_args()
    probe = subprocess.run(['cargo', 'run', '--quiet', '--locked', '--manifest-path', str(ROOT/'apps/api/Cargo.toml'), '--', '--schema-report'], cwd=ROOT, check=True, capture_output=True, text=True)
    schema = json.loads(probe.stdout)
    expected = render(schema, POLICIES.read_bytes())
    doc = DESIGN.read_text()
    if args.update_schema:
        if BEGIN in doc:
            doc = doc.split(BEGIN)[0] + expected + doc.split(END, 1)[1]
        else:
            doc += '\n## 持久化结构契约与 harness\n\n新增、删除或修改表、字段、索引、约束及保留政策时，必须同步更新本设计的生命周期表和下面的实际结构表。`make harness` 与 CI 校验真实迁移结果及政策指纹；不允许仅改代码。结构表由 `make persistence-doc-update` 更新，人工仍须核对用途、保存期限与删除保护。探测使用内存库，不访问用户数据库。\n\n' + expected + '\n'
        DESIGN.write_text(doc)
    verify(doc, expected)
    if args.self_test:
        for mutated in [expected.replace('TEXT', 'BLOB', 1), expected.replace('SHA256：`', 'SHA256：`bad', 1), expected.replace('| instruments |', '| undocumented |', 1)]:
            try:
                verify(doc, mutated)
            except ValueError:
                continue
            raise AssertionError('harness accepted schema/policy mutation')
    print(f'Persistence harness passed: {len(schema)} tables, schema fields/indexes/constraints + policy + design verified.')


if __name__ == '__main__':
    try:
        main()
    except (ValueError, subprocess.CalledProcessError) as exc:
        print(str(exc), file=sys.stderr)
        if isinstance(exc, subprocess.CalledProcessError):
            print(exc.stderr, file=sys.stderr)
        sys.exit(1)
