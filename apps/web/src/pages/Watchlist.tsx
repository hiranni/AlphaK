import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { ArrowDown, ArrowUp, ArrowUpDown, ArrowUpRight, Activity, BellPlus, LayoutGrid, TrendingUp, Zap, Download, GripVertical, Palette, Pencil, Plus, RefreshCw, Star, Trash2, Upload, X } from "lucide-react";
import { api } from "../api";
import type { SearchResult, Settings, WatchAnalysis, WatchlistGroup, WatchWorkbench } from "../types";
import { Change } from "../components/Change";
import { Empty, ErrorState, Loading } from "../components/States";
import { dateLabel, num, compact, changeClass } from "../format";
import { parseCsv, toCsv } from "../csv";

import { Sparkline, WatchOverview, WatchSignals } from "../components/WatchWorkbench";
import { WatchAnalysisPanel } from "../components/WatchAnalysis";
import { WatchAlertsPanel } from "../components/WatchAlerts";

const ALL_WATCHLIST = "watchlist-all";

const SORT_FIELDS = [
  ["price", "最新价"], ["change_amount", "涨跌额"], ["change", "当日涨跌幅"],
  ["amount", "成交额"], ["volume_ratio", "量比"], ["score", "规则符合分"],
] as const;

function sortValue(a: WatchAnalysis | undefined, key: string): number | null {
  if (!a) return null;
  let value: number | null | undefined;
  value = key === "price" ? a.close : key === "change_amount" ? a.change : key === "change" ? a.change_pct : key === "amount" ? a.amount : key === "volume_ratio" ? a.volume_ratio : a.score;
  return value != null && Number.isFinite(value) ? value : null;
}


const GROUP_COLORS = [
  "#b23a2b", "#c2412d", "#d97706", "#b98a2e", "#8a8f2b", "#4d7c3a",
  "#2f6f5e", "#2f7d7a", "#2f6f9e", "#3b6ea8", "#4f5fa8", "#7a5c9e",
  "#9b4f7a", "#a84f5f", "#8a8175", "#6b7280", "#4b5563", "#1f2937",
  "#e11d48", "#ea580c", "#ca8a04", "#16a34a", "#0891b2", "#2563eb",
  "#7c3aed", "#c026d3", "#db2777", "#475569", "#000000", "#ffffff",
];

function GroupDialog({ title, onClose, children, alert = false }: { title: string; onClose: () => void; children: React.ReactNode; alert?: boolean }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    ref.current?.showModal();
    return () => { ref.current?.close(); previous?.focus(); };
  }, []);
  return <dialog ref={ref} className="watch-dialog watch-group-dialog" role={alert ? "alertdialog" : "dialog"} aria-label={title} onCancel={e => { e.preventDefault(); onClose(); }}>
    <header className="watch-dialog-head"><h2>{title}</h2><button className="icon-btn" aria-label={`关闭${title}`} onClick={onClose}><X size={16} /></button></header>
    <div className="watch-dialog-body">{children}</div>
  </dialog>;
}

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
  const [menu, setMenu] = useState<{ id: string; x: number; y: number } | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState("");
  const [selectedSymbol, setSelectedSymbol] = useState("");
  const [searchOpen, setSearchOpen] = useState(false);
  const [editingNote, setEditingNote] = useState<string | null>(null);
  const [noteDraft, setNoteDraft] = useState("");
  const [noteError, setNoteError] = useState("");
  const [groupName, setGroupName] = useState("");
  const [dragId, setDragId] = useState<string | null>(null);
  const [importErrors, setImportErrors] = useState<string[]>([]);
  const [scrollTop, setScrollTop] = useState(0);
  const [workbench, setWorkbench] = useState<WatchWorkbench | null>(null);
  const [analysisError, setAnalysisError] = useState("");
  const [analysisReload, setAnalysisReload] = useState(0);
  const [filter, setFilter] = useState("all");
  const [listQuery, setListQuery] = useState("");
  const [orderBy, setOrderBy] = useState("manual");
  const [sortDirection, setSortDirection] = useState<"asc" | "desc">("desc");
  const sortBy = (key: string) => {
    setSortDirection(orderBy === key && sortDirection === "desc" ? "asc" : "desc");
    setOrderBy(key);
  };
  const sortState = (...keys: string[]): "ascending" | "descending" | "none" => keys.includes(orderBy) ? sortDirection === "asc" ? "ascending" : "descending" : "none";
  const sortButton = (key: string, label: string, hint?: string) => {
    const active = orderBy === key;
    const Icon = active ? sortDirection === "asc" ? ArrowUp : ArrowDown : ArrowUpDown;
    return <button type="button" className={`watch-sort-button${active ? " is-active" : ""}`} onClick={() => sortBy(key)} aria-label={`按${label}排序`} title={`${hint ? `${hint}；` : ""}点击按${label}${active && sortDirection === "desc" ? "升序" : "降序"}排列`}>{label}<Icon size={12} aria-hidden="true" /></button>;
  };
  const [inspect, setInspect] = useState<string | null>(null);
  const [alertSymbol, setAlertSymbol] = useState<string | null>(null);
  const [checked, setChecked] = useState<string[]>([]);
  const [confirmBatch, setConfirmBatch] = useState(false);
  const [assignOpen, setAssignOpen] = useState(false);
  const [assignTarget, setAssignTarget] = useState("");
  const [assignError, setAssignError] = useState("");
  const [batchText, setBatchText] = useState("");
  const [batchOpen, setBatchOpen] = useState(false);
  const [detailed, setDetailed] = useState(() => localStorage.getItem("alphak.watch.view") === "detailed");
  const [focusSymbols, setFocusSymbols] = useState<string[]>(() => { try { const saved = JSON.parse(localStorage.getItem("alphak.watch.focus") || "[]"); return Array.isArray(saved) ? saved.filter(v => typeof v === "string") : []; } catch { return []; } });
  const tableRef = useRef<HTMLDivElement>(null);
  const rowHeight = detailed ? 88 : 64;
  const viewportHeight = 460;
  const overscan = 8;
  const [fields, setFields] = useState(() => {
    try {
      const saved = JSON.parse(localStorage.getItem("alphak.watch.fields") || "{}");
      return { price: true, change: true, note: true, asOf: true, spark: true, amount: true, score: true, trend: true, ...saved } as Record<string, boolean>;
    } catch {
      return { price: true, change: true, note: true, asOf: true, spark: true, amount: true, score: true, trend: true };
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
    setSearchError("");
    if (!symbol.trim() || symbol === selectedSymbol) {
      setSearching(false);
      return;
    }
    setSearching(true);
    const timer = window.setTimeout(() => {
      api.search(symbol.trim())
        .then((items) => { if (active) setResults(items); })
        .catch((err: Error) => { if (active) setSearchError(err.message); })
        .finally(() => { if (active) setSearching(false); });
    }, 220);
    return () => { active = false; window.clearTimeout(timer); };
  }, [symbol, selectedSymbol]);

  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") setMenu(null); };
    window.addEventListener("click", close);
    window.addEventListener("keydown", escape);
    return () => { window.removeEventListener("click", close); window.removeEventListener("keydown", escape); };
  }, [menu]);

  useEffect(() => {
    const interval = settings?.poll_interval ?? 0;
    if (interval <= 0) return;
    const timer = window.setInterval(() => {
      api.watchlists().then(setGroups).catch(() => setNotice("行情更新失败，保留上次数据，稍后自动重试"));
    }, interval * 1000);
    return () => window.clearInterval(timer);
  }, [settings?.poll_interval]);

  const current = useMemo(() => groups.find((group) => group.id === selected) ?? groups[0], [groups, selected]);
  const isAll = current?.id === ALL_WATCHLIST;
  const customGroups = groups.filter(g => g.id !== ALL_WATCHLIST);
  const symbolKey = [...new Set(groups.flatMap(g => g.items.map(i => i.symbol)))].sort().join(",");
  const trackedSymbols = useMemo(() => [...new Map(groups.flatMap(g => g.items).map(i => [i.symbol, { symbol: i.symbol, name: i.name }])).values()], [groups]);
  const analysisMap = useMemo(() => new Map(workbench?.analyses.map(a => [a.symbol, a]) ?? []), [workbench]);
  useEffect(() => {
    let active = true;
    const fetch = () => api.watchWorkbench().then(next => { if (active) { setWorkbench(next); setAnalysisError(""); } }).catch((err: Error) => { if (active) setAnalysisError(err.message); });
    void fetch();
    const timer = window.setInterval(() => { if (!document.hidden) void fetch(); }, 30000);
    return () => { active = false; clearInterval(timer); };
  }, [symbolKey, analysisReload]);
  useEffect(() => { localStorage.setItem("alphak.watch.focus", JSON.stringify(focusSymbols)); }, [focusSymbols]);
  useEffect(() => { localStorage.setItem("alphak.watch.view", detailed ? "detailed" : "compact"); }, [detailed]);
  const filteredItems = useMemo(() => {
    const items = (current?.items ?? []).filter(item => {
      const a = analysisMap.get(item.symbol);
      if (listQuery && !`${item.name} ${item.symbol} ${item.note}`.toLowerCase().includes(listQuery.trim().toLowerCase())) return false;
      if (filter === "focus") return focusSymbols.includes(item.symbol);
      if (filter === "breakout") return a?.status === "ready" && a.signals.some(s => s.kind === "breakout");
      if (filter === "volatility") return a?.status === "ready" && a.volatility != null && a.volatility >= 40;
      if (filter === "signals") return a?.status === "ready" && a.signals.length > 0;
      return true;
    });
    if (orderBy !== "manual") items.sort((x, y) => {
      const a = analysisMap.get(x.symbol), b = analysisMap.get(y.symbol);
      const av = sortValue(a, orderBy);
      const bv = sortValue(b, orderBy);
      return av == null ? bv == null ? 0 : 1 : bv == null ? -1 : (sortDirection === "asc" ? av - bv : bv - av);
    });
    return items;
  }, [current, listQuery, filter, focusSymbols, orderBy, sortDirection, analysisMap]);
  const totalItems = filteredItems.length;
  const virtualize = totalItems > 100;
  const startIndex = virtualize ? Math.max(0, Math.floor(scrollTop / rowHeight) - overscan) : 0;
  const endIndex = virtualize ? Math.min(totalItems, Math.ceil((scrollTop + viewportHeight) / rowHeight) + overscan) : totalItems;
  const visibleItems = filteredItems.slice(startIndex, endIndex);
  const columnCount = 5 + Object.values(fields).filter(Boolean).length;
  useEffect(() => {
    setScrollTop(0);
    if (tableRef.current) tableRef.current.scrollTop = 0;
    setChecked([]); setConfirmBatch(false);
  }, [current?.id, filter, listQuery, orderBy, sortDirection, detailed]);
  const toggleFocus = (symbol: string) => setFocusSymbols(v => v.includes(symbol) ? v.filter(s => s !== symbol) : [...v, symbol]);
  const batchRemove = async () => {
    setBusy(true); let failed = 0; const remaining: string[] = [];
    for (const id of checked) {
      try { setGroups(await api.deleteItem(id)); } catch { failed++; remaining.push(id); }
    }
    setChecked(remaining); setConfirmBatch(false); setBusy(false); setNotice(`批量移除完成，失败 ${failed} 条`);
  };
  const assignToGroup = async () => {
    if (!current || !assignTarget || busy) return;
    setBusy(true); setAssignError("");
    const failed: string[] = [];
    for (const item of current.items.filter(i => checked.includes(i.id))) {
      try { setGroups(await api.addItem(assignTarget, item.symbol, item.note)); }
      catch { failed.push(item.name); }
    }
    setBusy(false);
    if (failed.length) setAssignError(`以下股票未加入，可重试：${failed.join("、")}`);
    else { setAssignOpen(false); setChecked([]); setNotice("已加入分组，全部自选保持不变"); }
  };
  const batchAdd = async () => {
    if (!current) return;
    const queries = [...new Set(batchText.split(/[\s,，;；]+/).map(s => s.trim()).filter(Boolean))];
    if (queries.length > 50) { setNotice("每次最多批量加入 50 只证券"); return; }
    setBusy(true); let added = 0; const failed: string[] = [];
    for (const q of queries) {
      try {
        const matches = await api.search(q);
        const exact = matches.find(i => i.symbol.toLowerCase() === q.toLowerCase() || i.symbol.split(".")[0] === q || i.name === q);
        const chosen = exact || (matches.length === 1 ? matches[0] : undefined);
        if (!chosen) { failed.push(q); continue; }
        setGroups(await api.addItem(current.id, chosen.symbol)); added++;
      } catch { failed.push(q); }
    }
    setBusy(false); setBatchText(failed.join("\n")); if (!failed.length) setBatchOpen(false);
    setNotice(`批量处理完成：成功 ${added}，失败或匹配不唯一 ${failed.length}${failed.length ? `（${failed.join("、")}）` : ""}`);
  };

  useEffect(() => {
    setScrollTop(0);
    setConfirmDelete(false);
    setConfirmClear(false);
    setEditingGroup(false);
    setMenu(null);
    setEditingNote(null);
  }, [current?.id]);

  const add = async (chosen?: string) => {
    if (!current || !symbol.trim()) return;
    setBusy(true);
    setNotice("");
    try {
      const matches = chosen || selectedSymbol ? [] : await api.search(symbol.trim());
      const exact = matches.find((item) => item.symbol === symbol.trim().toUpperCase() || item.symbol.split(".")[0] === symbol.trim() || item.name === symbol.trim());
      const resolved = chosen || selectedSymbol || exact?.symbol || (matches.length === 1 ? matches[0].symbol : undefined);
      if (!resolved) { setNotice(matches.length ? "请选择下方的证券后加入" : "未找到证券，请检查名称或代码"); return; }
      const next = await api.addItem(current.id, resolved, note.trim());
      setGroups(next);
      setSymbol("");
      setSelectedSymbol("");
      setSearchOpen(false);
      setNote("");
      setNotice(`已加入「${current.name}」`);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "加入失败");
    } finally {
      setBusy(false);
    }
  };

  const chooseSuggestion = (item: SearchResult) => {
    setSymbol(item.symbol);
    setSelectedSymbol(item.symbol);
    setSearchOpen(false);
    setResults([]);
    setSearchError("");
    window.setTimeout(() => document.querySelector<HTMLInputElement>('.add-row input[aria-label="备注"]')?.focus(), 0);
  };

  const startNote = (id: string, value: string) => {
    setNoteError("");
    setEditingNote(id);
    setNoteDraft(value);
  };

  const saveNote = async (id: string) => {
    setNoteError("");
    setBusy(true);
    try {
      setGroups(await api.updateItem(id, { note: noteDraft }));
      setEditingNote(null);
      setNotice("备注已更新");
    } catch (err) {
      setNoteError(err instanceof Error ? err.message : "备注更新失败");
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
      setAnalysisReload(v => v + 1);
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
    if (!current || isAll) return;
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

  const updateGroupColor = async (color: string) => {
    if (!current) return;
    setBusy(true);
    try {
      setGroups(await api.updateGroup(current.id, { color }));
      setMenu(null);
      setNotice("分组颜色已更新");
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "颜色更新失败");
    } finally {
      setBusy(false);
    }
  };

  const clearGroup = async () => {
    if (!current) return;
    setBusy(true);
    try {
      setGroups(await api.clearGroup(current.id));
      setConfirmClear(false);
      setMenu(null);
      setNotice(`已清空「${current.name}」中的自选记录，行情数据保留。`);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "清空分组失败");
    } finally {
      setBusy(false);
    }
  };

  const openMenu = (event: { clientX: number; clientY: number; preventDefault: () => void }, id: string) => {
    event.preventDefault();
    if (id === ALL_WATCHLIST) return;
    setSelected(id);
    const width = 216;
    setMenu({ id, x: Math.min(event.clientX, window.innerWidth - width - 12), y: Math.min(event.clientY, window.innerHeight - 460) });
  };

  const reorder = async (targetId: string) => {
    if (!current || !dragId || dragId === targetId || orderBy !== "manual" || filter !== "all" || listQuery.trim()) return;
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
          <h1 className="display">自选工作台</h1>
        </div>
        <div className="head-aside">
          <span className="stamp">A 股 · 日线观察</span>
          <div className="head-aside-row"><span className="mono muted">轮询</span><span className="mono">{settings?.poll_interval ? `${settings.poll_interval}s` : "关闭"}</span></div>
          <div className="head-aside-row"><span className="mono muted">分组</span><span className="mono">{customGroups.length}</span></div>
        </div>
      </header>

      <WatchOverview groups={groups} data={workbench} error={analysisError} onRetry={() => setAnalysisReload(v => v + 1)} />
      <div className="watch-layout">
        <aside className="panel group-panel">
          <div className="panel-head">
            <div><span className="eyebrow mono">GROUPS</span><h2>自选</h2></div>
            <div className="inline-actions">
              <button className="icon-btn" aria-label="管理当前分组" title="管理当前分组" disabled={isAll} onClick={event => { event.stopPropagation(); if (current) openMenu({ clientX: event.currentTarget.getBoundingClientRect().left, clientY: event.currentTarget.getBoundingClientRect().bottom, preventDefault() {} }, current.id); }}><Palette size={15} /></button>
              <button className="icon-btn" onClick={() => setShowNewGroup(true)} aria-label="新建分组" title="新建分组"><Plus size={15} /></button>
            </div>
          </div>
          <div className="group-list" role="tablist" aria-label="自选分组">
            {groups.map((group) => (
              <button
                key={group.id}
                role="tab" aria-selected={current?.id === group.id}
                onKeyDown={event => { if (["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) { event.preventDefault(); const index = groups.findIndex(g => g.id === group.id); const next = event.key === "Home" ? 0 : event.key === "End" ? groups.length - 1 : (index + (event.key === "ArrowRight" ? 1 : -1) + groups.length) % groups.length; setSelected(groups[next].id); (event.currentTarget.parentElement?.children[next] as HTMLElement)?.focus(); } }}
                tabIndex={current?.id === group.id ? 0 : -1}
                className={`group-row ${current?.id === group.id ? "is-active" : ""}`}
                onClick={() => setSelected(group.id)}
                onContextMenu={(event) => openMenu(event, group.id)}
                title={group.id === ALL_WATCHLIST ? "全部关注的股票，跨组去重" : "右键管理分组"}
              >
                <span className="group-name"><i className="group-color" style={{ background: group.color || "#8a8175" }} />{group.name}</span>
                <span className="mono muted">{group.items.length}</span>
              </button>
            ))}
          </div>
          {editingGroup ? (
            <GroupDialog title="重命名分组" onClose={() => { if (!busy) setEditingGroup(false); }}><div className="group-create">
              <input className="input" value={groupName} onChange={(event) => setGroupName(event.target.value)} placeholder="分组名称" autoFocus />
              <div className="inline-actions">
                <button className="btn btn-solid" onClick={renameGroup} disabled={busy || !groupName.trim()}>保存</button>
                <button className="btn btn-ghost" onClick={() => setEditingGroup(false)}>取消</button>
              </div>
            </div></GroupDialog>
          ) : null}
          {showNewGroup ? (
            <GroupDialog title="新建分组" onClose={() => { if (!busy) setShowNewGroup(false); }}><div className="group-create">
              <input className="input" value={newGroup} onChange={(event) => setNewGroup(event.target.value)} placeholder="新分组名称" autoFocus />
              <div className="inline-actions">
                <button className="btn btn-solid" onClick={createGroup} disabled={busy || !newGroup.trim()}>创建</button>
                <button className="btn btn-ghost" onClick={() => setShowNewGroup(false)}>取消</button>
              </div>
            </div></GroupDialog>
          ) : null}
          {confirmDelete ? <GroupDialog title="删除分组确认" alert onClose={() => { if (!busy) setConfirmDelete(false); }}><div className="group-create"><p>删除「{current?.name}」分组？股票仍保留在全部自选中，其他分组不受影响。</p><div className="inline-actions"><button className="btn btn-solid" disabled={busy} onClick={deleteGroup}>确认删除</button><button className="btn btn-ghost" onClick={() => setConfirmDelete(false)}>取消</button></div></div></GroupDialog> : null}
          {confirmClear ? <GroupDialog title="清空分组确认" alert onClose={() => { if (!busy) setConfirmClear(false); }}><div className="group-create"><p>清空「{current?.name}」的股票归类？全部自选与其他分组会保留。</p><div className="inline-actions"><button className="btn btn-solid" disabled={busy} onClick={clearGroup}>确认清空</button><button className="btn btn-ghost" onClick={() => setConfirmClear(false)}>取消</button></div></div></GroupDialog> : null}
          {menu ? (() => {
            const group = groups.find((item) => item.id === menu.id);
            if (!group) return null;
            return <div className="watch-context" style={{ left: menu.x, top: menu.y }} onClick={(event) => event.stopPropagation()} role="menu" aria-label={`${group.name} 分组操作`}>
              <div className="watch-context-title">{group.name}</div>
              <button onClick={() => { setGroupName(group.name); setEditingGroup(true); setMenu(null); }}><span>重命名</span></button>
              <div className="watch-context-label"><Palette size={12} /> 分组颜色</div>
              <div className="watch-color-grid">{GROUP_COLORS.map((color) => <button key={color} className={`watch-color ${group.color === color ? "is-active" : ""}`} style={{ background: color }} onClick={() => void updateGroupColor(color)} aria-label={`颜色 ${color}`} title={color} />)}</div>
              <label className="watch-color-custom" onClick={(event) => event.stopPropagation()}>
                <input type="color" value={group.color || "#8a8175"} onChange={(event) => void updateGroupColor(event.target.value)} aria-label="自定义分组颜色" />
                <span>自定义颜色</span>
                <span className="mono muted">{group.color || "#8a8175"}</span>
              </label>
              <button onClick={() => { setConfirmClear(true); setMenu(null); }}><span>清空分组</span><span className="mono muted">{group.items.length}</span></button>
              <button className="is-danger" disabled={isAll} onClick={() => { setConfirmDelete(true); setMenu(null); }}><span>删除分组</span><Trash2 size={13} /></button>
            </div>;
          })() : null}
        </aside>

        <section className="panel watch-main">
          <div className="panel-head">
            <div><span className="eyebrow mono">TRACKED</span><h2>{current?.name ?? "自选"}</h2></div>
            <div className="inline-actions">
              <details className="field-menu">
                <summary className="btn btn-ghost">字段</summary>
                <div className="field-menu-pop">
                  {[["price", "最新价 / 涨跌额"], ["change", "当日涨跌幅"], ["spark", "日线迷你图"], ["amount", "成交额 / 成交量比"], ["score", "规则符合分"], ["trend", "趋势 / 信号"], ["note", "备注"], ["asOf", "数据时间"]].map(([key, label]) => (
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

          <div className="watch-toolbar">
            <input className="input" value={listQuery} onChange={e => setListQuery(e.target.value)} placeholder="筛选名称 / 代码 / 备注" aria-label="筛选自选" />
            <select className="select" value={orderBy === "manual" ? "manual" : `${orderBy}:${sortDirection}`} onChange={e => { const [key, direction] = e.target.value.split(":"); setOrderBy(key); setSortDirection(direction === "asc" ? "asc" : "desc"); }} aria-label="自选排序"><option value="manual">自定义顺序</option>{SORT_FIELDS.flatMap(([key, label]) => [<option key={`${key}:desc`} value={`${key}:desc`}>{label}从高到低</option>, <option key={`${key}:asc`} value={`${key}:asc`}>{label}从低到高</option>])}</select>
            <button className="btn btn-ghost" aria-pressed={detailed} onClick={() => setDetailed(v => !v)}>{detailed ? "切换紧凑视图" : "切换详细视图"}</button>
          </div>
          <div className="watch-filter-tabs" role="group" aria-label="自选筛选">{([{ value: "all", label: "全部", icon: LayoutGrid }, { value: "focus", label: "重点关注", icon: Star }, { value: "breakout", label: "趋势突破", icon: TrendingUp }, { value: "volatility", label: "高波动", icon: Activity }, { value: "signals", label: "有信号", icon: Zap }]).map(({ value, label, icon: Icon }) => <button key={value} className={`btn btn-ghost watch-filter-${value} ${filter === value ? "is-active" : ""}`} aria-pressed={filter === value} onClick={() => setFilter(value)}><Icon size={14} aria-hidden="true" />{label}</button>)}<span className="muted">{isAll ? "全部自选" : "当前分组"} {totalItems} / {current?.items.length ?? 0}</span></div>
          <div className="add-row">
            <div className="watch-search">
              <input className="input mono" value={symbol} onChange={(event) => { setSymbol(event.target.value); setSelectedSymbol(""); setSearchOpen(true); }} onFocus={() => setSearchOpen(true)} onBlur={() => window.setTimeout(() => setSearchOpen(false), 160)} onKeyDown={(event) => { if (event.key === "Enter" && !event.nativeEvent.isComposing) { setSearchOpen(false); void add(); } }} placeholder="搜索名称 / 代码 / 拼音" aria-label="证券代码" aria-expanded={searchOpen && Boolean(symbol.trim()) && !selectedSymbol} />
              {searchOpen && symbol.trim() && !selectedSymbol ? <div className="watch-suggestions" role="listbox" aria-label="证券搜索结果">
                {searching ? <div className="watch-suggestion-note">搜索中…</div> : null}
                {!searching && searchError ? <div className="watch-suggestion-note is-error">{searchError}</div> : null}
                {!searching && !searchError && results.length === 0 ? <div className="watch-suggestion-note">未找到匹配证券</div> : null}
                {results.slice(0, 8).map((item) => <button key={item.symbol} type="button" role="option" disabled={busy} onMouseDown={(event) => event.preventDefault()} onClick={() => chooseSuggestion(item)}><span>{item.name}</span><span className="mono muted">{item.symbol}</span></button>)}
              </div> : null}
            </div>
            <input className="input" value={note} onChange={(event) => setNote(event.target.value)} placeholder="备注（可选）" aria-label="备注" />
            <button className="btn btn-solid" onClick={() => void add()} disabled={busy || !symbol.trim()}><Plus size={14} /> 加入</button>
          </div>
          <div className="watch-batch-bar"><button className="text-btn" disabled={busy} onClick={() => setBatchOpen(v => !v)}>批量添加</button><span className="muted">勾选 {checked.length} 只</span><button className="text-btn" disabled={busy || !checked.length || !customGroups.length} onClick={() => { setAssignTarget(customGroups.find(g => g.id !== current?.id)?.id ?? customGroups[0]?.id ?? ""); setAssignError(""); setAssignOpen(true); }}>加入分组</button><button className="text-btn" disabled={busy || !checked.length} onClick={() => setConfirmBatch(true)}>批量移除</button>{confirmBatch ? <span role="alert">{isAll ? "确认取消关注所选股票？将同时从各分组移除。" : "确认从当前分组移除？全部自选仍保留。"}<button className="text-btn" disabled={busy} onClick={() => void batchRemove()}>确认批量移除</button><button className="text-btn" onClick={() => setConfirmBatch(false)}>取消</button></span> : null}</div>
          {batchOpen ? <div className="watch-batch-add"><textarea className="input" rows={3} value={batchText} onChange={e => setBatchText(e.target.value)} aria-label="批量证券" placeholder="每行一只名称或代码，也可用逗号分隔；单次最多 50 只" /><button className="btn btn-solid" disabled={busy || !batchText.trim()} onClick={() => void batchAdd()}>{busy ? "处理中…" : "批量加入当前分组"}</button></div> : null}
          {notice ? <p className="inline-notice" role="status">{notice}</p> : null}
          {importErrors.length ? <ul className="import-errors">{importErrors.map((error) => <li key={error}>{error}</li>)}</ul> : null}

          {totalItems ? (
            <div
              ref={tableRef}
              className={`table-wrap watch-table-scroll ${virtualize ? "virtual-table" : ""} ${detailed ? "watch-detailed" : "watch-compact"}`}
              onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
            >
              <table className="data-table watch-table">
                <colgroup>
                  <col style={{ width: 36 }} /><col style={{ width: 28 }} /><col style={{ width: 36 }} /><col style={{ width: 132 }} />
                  {fields.price ? <col style={{ width: 120 }} /> : null}{fields.change ? <col style={{ width: 88 }} /> : null}
                  {fields.spark ? <col style={{ width: 108 }} /> : null}{fields.amount ? <col style={{ width: 128 }} /> : null}
                  {fields.score ? <col style={{ width: 96 }} /> : null}{fields.trend ? <col style={{ width: 160 }} /> : null}
                  {fields.note ? <col style={{ width: 128 }} /> : null}{fields.asOf ? <col style={{ width: 124 }} /> : null}
                  <col style={{ width: 136 }} />
                </colgroup>
                <thead><tr><th><input type="checkbox" aria-label="全选当前筛选结果" checked={totalItems > 0 && filteredItems.every(i => checked.includes(i.id))} onChange={e => setChecked(e.target.checked ? filteredItems.map(i => i.id) : [])} /></th><th aria-label="排序" /><th aria-label="重点关注" /><th>证券</th>{fields.price ? <th className="num" aria-sort={sortState("price", "change_amount")}>{sortButton("price", "最新价")} / {sortButton("change_amount", "涨跌额")}</th> : null}{fields.change ? <th className="num" aria-sort={sortState("change")}>{sortButton("change", "当日涨跌幅", "按数据日期对应交易日的涨跌幅排序；当日数据未更新时使用最近交易日，不代表实时行情")}</th> : null}{fields.spark ? <th title="历史日线走势，仅供观察趋势；当日表现请查看当日涨跌幅及日线日期">日线走势</th> : null}{fields.amount ? <th className="num" aria-sort={sortState("amount", "volume_ratio")}>{sortButton("amount", "成交额")} / {sortButton("volume_ratio", "量比")}</th> : null}{fields.score ? <th className="num" aria-sort={sortState("score")}>{sortButton("score", "规则符合分")}</th> : null}{fields.trend ? <th>趋势 / 信号</th> : null}{fields.note ? <th>备注</th> : null}{fields.asOf ? <th>日线日期 / 来源</th> : null}<th className="num">操作</th></tr></thead>
                <tbody>
                  {startIndex > 0 ? <tr aria-hidden="true" className="virtual-spacer" style={{ height: startIndex * rowHeight }}><td colSpan={columnCount} /></tr> : null}
                  {visibleItems.map((item) => { const a = analysisMap.get(item.symbol); return (
                    <tr
                      key={item.id}
                      draggable={!busy && orderBy === "manual" && filter === "all" && !listQuery.trim()}
                      style={{ height: rowHeight }}
                      className={dragId === item.id ? "is-dragging" : ""}
                      onDragStart={() => setDragId(item.id)}
                      onDragOver={(event) => event.preventDefault()}
                      onDrop={() => reorder(item.id)}
                      onDragEnd={() => setDragId(null)}
                      data-row={item.id}
                    >
                      <td><input type="checkbox" aria-label={`选择 ${item.name}`} checked={checked.includes(item.id)} onChange={e => setChecked(v => e.target.checked ? [...v, item.id] : v.filter(id => id !== item.id))} /></td>
                      <td className="drag-handle" aria-label={`拖拽 ${item.name} 排序`} title="拖拽排序"><GripVertical size={14} /></td>
                      <td><button className={`icon-btn watch-focus ${focusSymbols.includes(item.symbol) ? "is-active" : ""}`} aria-pressed={focusSymbols.includes(item.symbol)} aria-label={`重点关注 ${item.name}`} onClick={() => toggleFocus(item.symbol)}><Star size={14} fill={focusSymbols.includes(item.symbol) ? "currentColor" : "none"} /></button></td>
                      <td className="row-link"><button className="watch-stock-button" onClick={() => setInspect(item.symbol)}><span className="cell-title">{item.name}</span><span className="cell-sub mono">{item.symbol}</span>{detailed ? <span className="cell-sub">{a?.industry || "行业未知"}</span> : null}</button></td>
                      {fields.price ? <td className="num mono">{num(a?.close)}<span className={`cell-sub ${changeClass(a?.change)}`}>{a?.change != null ? `${a.change > 0 ? "+" : ""}${num(a.change)}` : "—"}</span></td> : null}
                      {fields.change ? <td className="num"><Change value={a?.change_pct} /></td> : null}
                      {fields.spark ? <td><Sparkline values={a?.sparkline ?? []} /></td> : null}
                      {fields.amount ? <td className="num mono">{compact(a?.amount)}<span className="cell-sub">量比 {a?.volume_ratio != null ? `${num(a.volume_ratio)}×` : "—"}</span>{detailed ? <span className="cell-sub">成交量 {compact(a?.volume)}（源口径）</span> : null}</td> : null}
                      {fields.score ? <td className="num mono" title="规则符合分 v1，非预测胜率，尚未回测">{a?.score != null ? `${a.score}/100` : "—"}</td> : null}
                      {fields.trend ? <td title={a?.notice}><span>{a?.trend || "待分析"}</span><span className="cell-sub">{a?.status === "demo" ? "演示 · 不参与" : a?.status === "stale" ? "日线待更新" : a && a.technical_bars_count < 21 ? `有效日线 ${a.technical_bars_count}/21 条` : a?.status === "ready" ? a.signals[0]?.title || "暂无条件触发" : "数据待补齐"}</span>{a?.history_status ? <span className="cell-sub">{a.history_status}{a.technical_bars_count >= 21 && a.technical_bars_count < 60 ? ` · ${a.technical_bars_count}/60 条` : ""}</span> : null}</td> : null}
                      {fields.note ? <td className="muted">
                        <button className="note-link" onClick={() => startNote(item.id, item.note)} title="编辑备注">{item.note || "添加备注"}</button>
                      </td> : null}
                      {fields.asOf ? <td className="mono muted">{dateLabel(a?.as_of)}<span className="cell-sub">{a?.source || "待更新"}{a?.status === "stale" ? " · 已过期" : ""}</span></td> : null}
                      <td className="num">
                        <button className="icon-btn" onClick={() => setAlertSymbol(item.symbol)} aria-label={`设置 ${item.name} 提醒`}><BellPlus size={14} /></button>
                        <button className="icon-btn" onClick={() => navigate(`/stock/${item.symbol}`)} aria-label={`打开 ${item.name} 完整分析`}><ArrowUpRight size={14} /></button>
                        <button className="icon-btn" onClick={() => startNote(item.id, item.note)} aria-label={`编辑 ${item.name} 备注`} disabled={busy}><Pencil size={13} /></button>
                        <button className="icon-btn" onClick={() => remove(item.id)} aria-label={`${isAll ? "取消关注" : "移出分组"} ${item.name}`} title={isAll ? "取消关注，并从所有分组移除" : "仅移出本组，全部自选保留"} disabled={busy}><Trash2 size={14} /></button>
                      </td>
                    </tr>
                  ); })}
                  {endIndex < totalItems ? <tr aria-hidden="true" className="virtual-spacer" style={{ height: (totalItems - endIndex) * rowHeight }}><td colSpan={columnCount} /></tr> : null}
                </tbody>
              </table>
            </div>
          ) : <Empty title={current?.items.length ? "没有符合当前筛选条件的股票" : "这个分组还没有股票"} note={current?.items.length ? "可清除搜索或切换全部自选；技术筛选仅使用新鲜真实日线。" : "输入名称或代码加入，或批量添加、导入 CSV。"} />}
          <p className="panel-footnote">自定义顺序且未筛选时可拖拽排序。迷你图为有效复权日线；量比使用前 20 日同源均量。重点关注、列与视图偏好保存在此浏览器。刷新失败保留上次数据。</p>
        </section>
      </div>
      {assignOpen ? <GroupDialog title="加入分组" onClose={() => { if (!busy) setAssignOpen(false); }}>
        <p>将勾选的 {checked.length} 只股票加入分组，已有归类和全部自选会保留。</p>
        <label className="field">目标分组<select className="input" value={assignTarget} onChange={e => setAssignTarget(e.target.value)} disabled={busy}>{customGroups.map(g => <option key={g.id} value={g.id}>{g.name}</option>)}</select></label>
        {assignError ? <p role="alert">{assignError}</p> : null}
        <footer className="watch-dialog-foot"><button className="btn btn-ghost" disabled={busy} onClick={() => setAssignOpen(false)}>取消</button><button className="btn btn-solid" disabled={busy || !assignTarget} onClick={() => void assignToGroup()}>{busy ? "处理中…" : "确认加入"}</button></footer>
      </GroupDialog> : null}
      {editingNote ? <GroupDialog title="编辑自选备注" onClose={() => { if (!busy) setEditingNote(null); }}>
        <form onSubmit={e => { e.preventDefault(); void saveNote(editingNote); }}>
          <p>{groups.flatMap(g => g.items).find(i => i.id === editingNote)?.name} <span className="mono muted">{groups.flatMap(g => g.items).find(i => i.id === editingNote)?.symbol}</span></p>
          <label className="field">备注<textarea className="input watch-note-draft" rows={6} value={noteDraft} onChange={e => setNoteDraft(e.target.value)} autoFocus disabled={busy} placeholder="记录关注原因、观察价位或后续计划…" onKeyDown={e => { if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); if (!busy) void saveNote(editingNote); } }} /></label>
          {noteError ? <p role="alert" className="inline-notice">{noteError}</p> : null}
          <p className="muted">{isAll ? "自选备注" : "仅修改当前分组中的备注"} · ⌘ / Ctrl + Enter 保存</p>
          <footer className="watch-dialog-foot"><button type="button" className="btn btn-ghost" disabled={busy} onClick={() => setEditingNote(null)}>取消</button><button className="btn btn-solid" disabled={busy}>{busy ? "保存中…" : "保存备注"}</button></footer>
        </form>
      </GroupDialog> : null}
      <WatchSignals analyses={(workbench?.analyses ?? []).filter(a => current?.items.some(i => i.symbol === a.symbol))} onInspect={setInspect} />
      <WatchAlertsPanel symbols={trackedSymbols} alertSymbol={alertSymbol} onDialogClose={() => setAlertSymbol(null)} />
      {inspect ? <WatchAnalysisPanel key={inspect} symbol={inspect} analysis={analysisMap.get(inspect)} onClose={() => setInspect(null)} onAlert={setAlertSymbol} /> : null}
    </div>
  );
}
