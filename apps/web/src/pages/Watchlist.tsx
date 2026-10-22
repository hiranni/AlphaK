import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Download, GripVertical, Pencil, Plus, RefreshCw, Star, Trash2, Upload } from "lucide-react";
import { api } from "../api";
import type { SearchResult, Settings, WatchlistGroup } from "../types";
import { Change } from "../components/Change";
import { Empty, ErrorState, Loading } from "../components/States";
import { dateLabel, num } from "../format";
import { parseCsv, toCsv } from "../csv";

export function Watchlist() {
  const [groups, setGroups] = useState<WatchlistGroup[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [selected, setSelected] = useState("");
  const [error, setError] = useState("");
  const [symbol, setSymbol] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const [newGroup, setNewGroup] = useState("");
  const [showNewGroup, setShowNewGroup] = useState(false);
  const [editingGroup, setEditingGroup] = useState(false);
  const [groupName, setGroupName] = useState("");
  const [dragId, setDragId] = useState<string | null>(null);
  const [importErrors, setImportErrors] = useState<string[]>([]);
  const [scrollTop, setScrollTop] = useState(0);
  const rowHeight = 53;
  const viewportHeight = 460;
  const overscan = 8;
  const [fields, setFields] = useState(() => {
    try {
      return JSON.parse(localStorage.getItem("alphak.watch.fields") || "") as Record<string, boolean>;
    } catch {
      return { price: true, change: true, note: true, asOf: true };
    }
  });

  useEffect(() => {
    localStorage.setItem("alphak.watch.fields", JSON.stringify(fields));
  }, [fields]);

  const toggleField = (key: string) => setFields((current) => ({ ...current, [key]: !current[key] }));
  const fileRef = useRef<HTMLInputElement>(null);
  const navigate = useNavigate();

  const load = () => {
    setError("");
    Promise.all([api.watchlists(), api.settings()])
      .then(([nextGroups, nextSettings]) => {
        setGroups(nextGroups);
        setSettings(nextSettings);
        setSelected((current) => current || nextGroups[0]?.id || "");
      })
      .catch((err: Error) => setError(err.message));
  };

  useEffect(load, []);

  useEffect(() => {
    let active = true;
    setResults([]);
    if (!symbol.trim()) return;
    const timer = window.setTimeout(() => {
      api.search(symbol.trim()).then((items) => { if (active) setResults(items); }).catch((err: Error) => { if (active) setNotice(err.message); });
    }, 250);
    return () => { active = false; window.clearTimeout(timer); };
  }, [symbol]);

  useEffect(() => {
    const interval = settings?.poll_interval ?? 0;
    if (interval <= 0) return;
    const timer = window.setInterval(() => {
      api.watchlists().then(setGroups).catch(() => setNotice("行情更新失败，保留上次数据，稍后自动重试"));
    }, interval * 1000);
    return () => window.clearInterval(timer);
  }, [settings?.poll_interval]);

  const current = useMemo(() => groups.find((group) => group.id === selected) ?? groups[0], [groups, selected]);
  const totalItems = current?.items.length ?? 0;
  const virtualize = totalItems > 100;
  const startIndex = virtualize ? Math.max(0, Math.floor(scrollTop / rowHeight) - overscan) : 0;
  const endIndex = virtualize ? Math.min(totalItems, Math.ceil((scrollTop + viewportHeight) / rowHeight) + overscan) : totalItems;
  const visibleItems = current ? current.items.slice(startIndex, endIndex) : [];
  const columnCount = 3 + Number(fields.price) + Number(fields.change) + Number(fields.note) + Number(fields.asOf);

  useEffect(() => {
    setScrollTop(0);
    setConfirmDelete(false);
    setEditingGroup(false);
  }, [current?.id]);

  const add = async (chosen?: string) => {
    if (!current || !symbol.trim()) return;
    setBusy(true);
    setNotice("");
    try {
      const matches = chosen ? [] : await api.search(symbol.trim());
      const exact = matches.find((item) => item.symbol === symbol.trim().toUpperCase() || item.symbol.split(".")[0] === symbol.trim() || item.name === symbol.trim());
      const resolved = chosen ?? exact?.symbol ?? (matches.length === 1 ? matches[0].symbol : undefined);
      if (!resolved) { setNotice(matches.length ? "请选择下方的证券后加入" : "未找到证券，请检查名称或代码"); return; }
      const next = await api.addItem(current.id, resolved, note.trim());
      setGroups(next);
      setSymbol("");
      setNote("");
      setNotice(`已加入「${current.name}」`);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "加入失败");
    } finally {
      setBusy(false);
    }
  };

  const remove = async (id: string) => {
    setBusy(true);
    try {
      setGroups(await api.deleteItem(id));
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "删除失败");
    } finally {
      setBusy(false);
    }
  };

  const refresh = async () => {
    setBusy(true);
    try {
      setGroups(await api.refreshWatchlists());
      setNotice("已请求更新，报价将自动刷新。");
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "刷新失败");
    } finally {
      setBusy(false);
    }
  };

  const renameGroup = async () => {
    if (!current || !groupName.trim()) return;
    setBusy(true);
    try {
      setGroups(await api.renameGroup(current.id, groupName.trim()));
      setEditingGroup(false);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "重命名失败");
    } finally {
      setBusy(false);
    }
  };

  const deleteGroup = async () => {
    if (!current || groups.length <= 1) return;
    setBusy(true);
    try {
      const next = await api.deleteGroup(current.id);
      setGroups(next);
      setSelected(next[0]?.id ?? "");
      setConfirmDelete(false);
      setNotice(`已删除「${current.name}」`);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "删除分组失败");
    } finally { setBusy(false); }
  };

  const reorder = async (targetId: string) => {
    if (!current || !dragId || dragId === targetId) return;
    const ids = current.items.map((item) => item.id);
    const from = ids.indexOf(dragId);
    const to = ids.indexOf(targetId);
    if (from < 0 || to < 0) return;
    ids.splice(from, 1);
    ids.splice(to, 0, dragId);
    setBusy(true);
    try {
      setGroups(await api.reorderItems(current.id, ids));
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "排序失败");
    } finally {
      setDragId(null);
      setBusy(false);
    }
  };

  const createGroup = async () => {
    if (!newGroup.trim()) return;
    setBusy(true);
    try {
      const next = await api.createGroup(newGroup.trim());
      setGroups(next);
      const created = next.find((group) => group.name === newGroup.trim());
      if (created) setSelected(created.id);
      setNewGroup("");
      setShowNewGroup(false);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "创建失败");
    } finally {
      setBusy(false);
    }
  };

  const exportCsv = () => {
    if (!current) return;
    const csv = toCsv([
      ["symbol", "name", "note"],
      ...current.items.map((item) => [item.symbol, item.name, item.note]),
    ]);
    const blob = new Blob(["\uFEFF" + csv], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `${current.name}-watchlist.csv`;
    anchor.click();
    URL.revokeObjectURL(url);
  };

  const importCsv = async (file: File) => {
    if (!current) return;
    setBusy(true);
    setNotice("");
    setImportErrors([]);
    const rows = parseCsv(await file.text());
    if (rows.length === 0) {
      setBusy(false);
      setNotice("CSV 为空");
      return;
    }
    const header = rows[0].map((cell) => cell.trim().toLowerCase());
    const hasHeader = header.includes("symbol");
    const symbolIndex = hasHeader ? header.indexOf("symbol") : 0;
    const noteIndex = hasHeader ? header.indexOf("note") : 2;
    const existing = new Set(current.items.map((item) => item.symbol));
    let added = 0;
    let skipped = 0;
    let failed = 0;
    const errors: string[] = [];
    for (let index = hasHeader ? 1 : 0; index < rows.length; index += 1) {
      const row = rows[index];
      const rawSymbol = (row[symbolIndex] ?? "").trim().toUpperCase();
      if (!rawSymbol) {
        failed += 1;
        errors.push(`第 ${index + 1} 行：代码为空`);
        continue;
      }
      if (existing.has(rawSymbol)) {
        skipped += 1;
        continue;
      }
      const note = noteIndex >= 0 ? (row[noteIndex] ?? "") : "";
      try {
        const next = await api.addItem(current.id, rawSymbol, note);
        setGroups(next);
        existing.add(rawSymbol);
        added += 1;
      } catch (err) {
        failed += 1;
        errors.push(`第 ${index + 1} 行（${rawSymbol}）：${err instanceof Error ? err.message : "导入失败"}`);
      }
    }
    setImportErrors(errors);
    setBusy(false);
    setNotice(`导入完成：新增 ${added}，重复跳过 ${skipped}，失败 ${failed}`);
  };

  if (error) return <ErrorState message={error} onRetry={load} />;
  if (!groups.length) return <Loading label="正在读取自选" />;

  return (
    <div className="page watch-page">
      <header className="page-head compact-head">
        <div>
          <h1 className="display">自选行情</h1>
        </div>
        <div className="head-aside">
          <span className="stamp">本地自选 · 未同步证券可能含演示数据</span>
          <div className="head-aside-row"><span className="mono muted">轮询</span><span className="mono">{settings?.poll_interval ? `${settings.poll_interval}s` : "关闭"}</span></div>
          <div className="head-aside-row"><span className="mono muted">分组</span><span className="mono">{groups.length}</span></div>
        </div>
      </header>

      <div className="watch-layout">
        <aside className="panel group-panel">
          <div className="panel-head"><div><span className="eyebrow mono">GROUPS</span><h2>分组</h2></div><Star size={16} /></div>
          <div className="group-list">
            {groups.map((group) => (
              <button key={group.id} className={`group-row ${current?.id === group.id ? "is-active" : ""}`} onClick={() => setSelected(group.id)}>
                <span>{group.name}</span><span className="mono muted">{group.items.length}</span>
              </button>
            ))}
          </div>
          {current ? (
            editingGroup ? (
              <div className="group-create">
                <input className="input" value={groupName} onChange={(event) => setGroupName(event.target.value)} placeholder="分组名称" autoFocus />
                <div className="inline-actions">
                  <button className="btn btn-solid" onClick={renameGroup} disabled={busy || !groupName.trim()}>保存</button>
                  <button className="btn btn-ghost" onClick={() => setEditingGroup(false)}>取消</button>
                </div>
              </div>
            ) : (
              <button className="btn btn-ghost group-add" onClick={() => { setGroupName(current.name); setEditingGroup(true); }}><Pencil size={13} /> 重命名当前分组</button>
            )
          ) : null}
          {showNewGroup ? (
            <div className="group-create">
              <input className="input" value={newGroup} onChange={(event) => setNewGroup(event.target.value)} placeholder="新分组名称" autoFocus />
              <div className="inline-actions">
                <button className="btn btn-solid" onClick={createGroup} disabled={busy || !newGroup.trim()}>创建</button>
                <button className="btn btn-ghost" onClick={() => setShowNewGroup(false)}>取消</button>
              </div>
            </div>
          ) : (
            <button className="btn btn-ghost group-add" onClick={() => setShowNewGroup(true)}><Plus size={14} /> 新建分组</button>
          )}
          <button className="btn btn-ghost group-add" disabled={busy || groups.length <= 1} onClick={() => setConfirmDelete(true)} title={groups.length <= 1 ? "至少保留一个分组" : "删除当前分组"}><Trash2 size={14} /> 删除当前分组</button>
          {confirmDelete ? <div className="group-create" role="alertdialog" aria-label="删除分组确认"><p>删除「{current?.name}」及其中的自选记录？行情数据会保留。</p><div className="inline-actions"><button className="btn btn-solid" disabled={busy} onClick={deleteGroup}>确认删除</button><button className="btn btn-ghost" onClick={() => setConfirmDelete(false)}>取消</button></div></div> : null}
        </aside>

        <section className="panel watch-main">
          <div className="panel-head">
            <div><span className="eyebrow mono">TRACKED</span><h2>{current?.name ?? "自选"}</h2></div>
            <div className="inline-actions">
              <details className="field-menu">
                <summary className="btn btn-ghost">字段</summary>
                <div className="field-menu-pop">
                  {[["price", "最新价"], ["change", "涨跌幅"], ["note", "备注"], ["asOf", "数据时间"]].map(([key, label]) => (
                    <label key={key}><input type="checkbox" checked={Boolean(fields[key])} onChange={() => toggleField(key)} /> {label}</label>
                  ))}
                </div>
              </details>
              <button className="btn btn-ghost" onClick={refresh} disabled={busy}><RefreshCw size={14} /> 刷新</button>
              <button className="btn btn-ghost" onClick={exportCsv}><Download size={14} /> 导出</button>
              <button className="btn btn-ghost" onClick={() => fileRef.current?.click()}><Upload size={14} /> 导入</button>
              <input ref={fileRef} type="file" accept=".csv,text/csv" hidden onChange={(event) => { const file = event.target.files?.[0]; if (file) void importCsv(file); event.target.value = ""; }} />
            </div>
          </div>

          <div className="add-row">
            <input className="input mono" value={symbol} onChange={(event) => setSymbol(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") void add(); }} placeholder="搜索名称 / 代码 / 拼音" aria-label="证券代码" />
            <input className="input" value={note} onChange={(event) => setNote(event.target.value)} placeholder="备注（可选）" aria-label="备注" />
            <button className="btn btn-solid" onClick={() => void add()} disabled={busy || !symbol.trim()}><Plus size={14} /> 加入</button>
          </div>
          {results.length ? <div className="group-create" aria-label="证券搜索结果">{results.slice(0, 8).map((item) => <button key={item.symbol} className="btn btn-ghost" disabled={busy} onClick={() => void add(item.symbol)}>{item.name} · {item.symbol} · 加入</button>)}</div> : null}
          {notice ? <p className="inline-notice" role="status">{notice}</p> : null}
          {importErrors.length ? <ul className="import-errors">{importErrors.map((error) => <li key={error}>{error}</li>)}</ul> : null}

          {current?.items.length ? (
            <div
              className={`table-wrap ${virtualize ? "virtual-table" : ""}`}
              onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
            >
              <table className="data-table watch-table">
                <thead><tr><th aria-label="排序" /><th>证券</th>{fields.price ? <th className="num">最新价</th> : null}{fields.change ? <th className="num">涨跌幅</th> : null}{fields.note ? <th>备注</th> : null}{fields.asOf ? <th>数据时间</th> : null}<th className="num">操作</th></tr></thead>
                <tbody>
                  {startIndex > 0 ? <tr aria-hidden="true" className="virtual-spacer" style={{ height: startIndex * rowHeight }}><td colSpan={columnCount} /></tr> : null}
                  {visibleItems.map((item) => (
                    <tr
                      key={item.id}
                      draggable
                      className={dragId === item.id ? "is-dragging" : ""}
                      onDragStart={() => setDragId(item.id)}
                      onDragOver={(event) => event.preventDefault()}
                      onDrop={() => reorder(item.id)}
                      onDragEnd={() => setDragId(null)}
                      data-row={item.id}
                    >
                      <td className="drag-handle" aria-label={`拖拽 ${item.name} 排序`} title="拖拽排序"><GripVertical size={14} /></td>
                      <td className="row-link" onClick={() => navigate(`/stock/${item.symbol}`)}>
                        <span className="cell-title">{item.name}</span>
                        <span className="cell-sub mono">{item.symbol}</span>
                      </td>
                      {fields.price ? <td className="num mono">{num(item.close)}</td> : null}
                      {fields.change ? <td className="num"><Change value={item.change_pct} /></td> : null}
                      {fields.note ? <td className="muted">{item.note || "—"}</td> : null}
                      {fields.asOf ? <td className="mono muted">{dateLabel(item.as_of)}</td> : null}
                      <td className="num">
                        <button className="icon-btn" onClick={() => remove(item.id)} aria-label={`移除 ${item.name}`} disabled={busy}><Trash2 size={14} /></button>
                      </td>
                    </tr>
                  ))}
                  {endIndex < totalItems ? <tr aria-hidden="true" className="virtual-spacer" style={{ height: (totalItems - endIndex) * rowHeight }}><td colSpan={columnCount} /></tr> : null}
                </tbody>
              </table>
            </div>
          ) : <Empty title="这个分组还没有股票" note="输入代码加入，或导入 CSV。" />}
          <p className="panel-footnote">拖拽行可排序；轮询间隔在设置页调整。刷新失败时会保留上一次报价，不静默清空。</p>
        </section>
      </div>
    </div>
  );
}
