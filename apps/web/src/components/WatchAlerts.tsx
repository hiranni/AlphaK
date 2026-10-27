import { useEffect, useRef, useState } from "react";
import { Bell, Pause, Pencil, Play, Plus, Trash2, X } from "lucide-react";
import type { WatchAlertInput, WatchAlertRule, WatchAlerts as AlertState } from "../types";
import { api } from "../api";

export const ALERT_KINDS: Record<string, string> = { price_above: "收盘价 ≥ 指定价格", price_below: "收盘价 ≤ 指定价格", change_above: "日涨跌幅 ≥ 指定百分比", change_below: "日涨跌幅 ≤ 指定百分比", volume_ratio: "成交量 / 前 20 日均量 ≥ 倍数", breakout: "突破前 20 日高点", support_break: "跌破前 20 日低点", volatility: "20 日年化波动率 ≥ 百分比" };
function defaults(symbol: string): WatchAlertInput { return { symbol, kind: "price_above", threshold: 100, cooldown_minutes: 1440, enabled: true }; }
const watermarkKey = "alphak.watch.alertWatermark";
export function WatchAlertsPanel({ symbols, alertSymbol, onDialogClose }: { symbols: { symbol: string; name: string }[]; alertSymbol: string | null; onDialogClose: () => void }) {
  const [data, setData] = useState<AlertState>({ rules: [], events: [] });
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState<WatchAlertRule | null>(null);
  const [show, setShow] = useState(false);
  const [form, setForm] = useState<WatchAlertInput>(defaults(""));
  const [permission, setPermission] = useState(() => typeof Notification === "undefined" ? "unsupported" : Notification.permission);
  const [browserEnabled, setBrowserEnabled] = useState(() => localStorage.getItem("alphak.watch.browserAlerts") === "1");
  const [confirmDelete, setConfirmDelete] = useState("");
  const [showAll, setShowAll] = useState(false);
  const dialog = useRef<HTMLDialogElement>(null);
  const notificationEnabled = useRef(browserEnabled);
  useEffect(() => { notificationEnabled.current = browserEnabled; localStorage.setItem("alphak.watch.browserAlerts", browserEnabled ? "1" : "0"); }, [browserEnabled]);
  const accept = (next: AlertState) => {
    setData(next);
    const max = next.events[0]?.id ?? 0;
    const stored = localStorage.getItem(watermarkKey);
    if (stored !== null && notificationEnabled.current && typeof Notification !== "undefined" && Notification.permission === "granted") {
      const fresh = next.events.filter(e => e.id > Number(stored)).slice(0, 3);
      fresh.forEach(e => new Notification("AlphaK · 日线条件提醒", { body: e.message, tag: `alphak-${e.rule_id}-${e.as_of}` }));
    }
    localStorage.setItem(watermarkKey, String(Math.max(Number(stored) || 0, max)));
  };
  const load = () => api.watchAlerts().then(next => { accept(next); setError(""); }).catch((e: Error) => setError(e.message));
  useEffect(() => {
    let active = true;
    const fetch = () => api.watchAlerts().then(next => { if (active) { accept(next); setError(""); } }).catch((e: Error) => { if (active) setError(e.message); });
    void fetch(); const timer = window.setInterval(fetch, 30000);
    return () => { active = false; clearInterval(timer); };
  }, []);
  useEffect(() => { if (alertSymbol) { setEditing(null); setForm(defaults(alertSymbol)); setShow(true); } }, [alertSymbol]);
  useEffect(() => {
    if (!show) return;
    const node = dialog.current; const previous = document.activeElement as HTMLElement | null;
    node?.showModal(); return () => { node?.close(); previous?.focus(); };
  }, [show]);
  const close = () => { if (!busy) { setShow(false); onDialogClose(); } };
  const save = async () => {
    setBusy(true); setError("");
    try { accept(editing ? await api.updateWatchAlert(editing.id, form) : await api.createWatchAlert(form)); setShow(false); onDialogClose(); }
    catch (e) { setError(e instanceof Error ? e.message : "保存失败"); }
    finally { setBusy(false); }
  };
  const toggle = async (r: WatchAlertRule) => {
    setBusy(true); setError("");
    try { accept(await api.updateWatchAlert(r.id, { ...r, enabled: !r.enabled })); } catch (e) { setError(e instanceof Error ? e.message : "更新失败"); } finally { setBusy(false); }
  };
  const remove = async (id: string) => {
    setBusy(true); setError("");
    try { accept(await api.deleteWatchAlert(id)); setConfirmDelete(""); } catch (e) { setError(e instanceof Error ? e.message : "删除失败"); } finally { setBusy(false); }
  };
  const enableBrowser = async () => {
    if (typeof Notification === "undefined") return;
    if (browserEnabled) { setBrowserEnabled(false); return; }
    try { const result = await Notification.requestPermission(); setPermission(result); setBrowserEnabled(result === "granted"); }
    catch { setError("浏览器未允许通知，请检查站点权限"); }
  };
  const name = (symbol: string) => symbols.find(s => s.symbol === symbol)?.name || symbol;
  return <section className="panel watch-alerts" aria-label="价格与技术提醒">
    <div className="panel-head"><div><span className="eyebrow mono">ALERTS / LOCAL</span><h2>条件提醒 <span className="muted">{data.rules.filter(r => r.enabled).length} 运行中</span></h2></div><div className="inline-actions"><button className="btn btn-ghost" disabled={permission === "unsupported" || permission === "denied"} onClick={() => void enableBrowser()}><Bell size={14} />{browserEnabled ? "关闭浏览器通知" : "开启浏览器通知"}</button><button className="btn btn-solid" disabled={!symbols.length} onClick={() => { setEditing(null); setForm(defaults(symbols[0].symbol)); setShow(true); }}><Plus size={14} /> 新建提醒</button></div></div>
    <p className="panel-footnote">服务运行期间每分钟检查本地已完成日线；同一规则、同一交易日最多提醒一次，并遵守冷却时间。行情未更新不会触发。浏览器通知还需页面保持打开；邮件与移动推送暂未接入。</p>
    {permission === "denied" ? <p className="muted">浏览器通知被禁用；可在站点权限中修改，站内记录仍会保存。</p> : null}
    {error && !show ? <p className="inline-notice" role="alert">{error} <button className="text-btn" onClick={() => void load()}>重试</button></p> : null}
    <div className="watch-alert-columns"><div><h3>监控规则</h3>{data.rules.length ? <ul className="watch-rule-list">{data.rules.map(r => <li key={r.id}>
      <div><strong>{name(r.symbol)}</strong><span className="watch-tag">{r.enabled ? "运行中" : "已暂停"}</span><p>{ALERT_KINDS[r.kind] || r.kind}{!["breakout", "support_break"].includes(r.kind) ? ` ${r.threshold}` : ""}</p><small className="muted">冷却 {r.cooldown_minutes} 分钟{!symbols.some(s => s.symbol === r.symbol) ? " · 已移出样本，暂停计算" : ""}</small></div>
      <div className="inline-actions"><button className="icon-btn" disabled={busy || !symbols.some(s => s.symbol === r.symbol)} aria-label={`${r.enabled ? "暂停" : "恢复"} ${name(r.symbol)} 提醒`} onClick={() => void toggle(r)}>{r.enabled ? <Pause size={14} /> : <Play size={14} />}</button><button className="icon-btn" disabled={busy || !symbols.some(s => s.symbol === r.symbol)} aria-label={`编辑 ${name(r.symbol)} 提醒`} onClick={() => { setEditing(r); setForm(r); setShow(true); }}><Pencil size={14} /></button><button className="icon-btn" disabled={busy} aria-label={`删除 ${name(r.symbol)} 提醒`} onClick={() => setConfirmDelete(r.id)}><Trash2 size={14} /></button></div>
      {confirmDelete === r.id ? <div className="watch-delete-confirm" role="alert"><span>删除这条规则？触发记录保留。</span><button className="text-btn" disabled={busy} onClick={() => void remove(r.id)}>确认删除提醒</button><button className="text-btn" onClick={() => setConfirmDelete("")}>取消</button></div> : null}
    </li>)}</ul> : <p className="muted">尚未设置规则。可从个股快捷分析中创建提醒。</p>}</div>
    <div><h3>最近触发记录</h3>{data.events.length ? <ol className="watch-event-list">{(showAll ? data.events : data.events.slice(0, 5)).map(e => <li key={e.id}><strong>{name(e.symbol)}</strong><p>{e.message.replace(/price_above|price_below|change_above|change_below|volume_ratio|support_break|breakout|volatility/g, kind => ALERT_KINDS[kind] || kind)}</p><time className="mono muted">记录于 {new Date(e.created_at).toLocaleString("zh-CN", { hour12: false })}</time></li>)}</ol> : <p className="muted">暂无触发记录。不会为演示数据或历史过期日线生成提醒。</p>}{data.events.length > 5 ? <button className="text-btn" onClick={() => setShowAll(v => !v)}>{showAll ? "收起记录" : `展开最近 ${data.events.length} 条`}</button> : null}</div></div>
    {show ? <dialog ref={dialog} className="watch-dialog watch-alert-dialog" aria-label={editing ? "编辑提醒" : "新建提醒"} onCancel={e => { e.preventDefault(); close(); }}>
      <header className="watch-dialog-head"><h2>{editing ? "编辑提醒" : "新建提醒"}</h2><button className="icon-btn" disabled={busy} onClick={close} aria-label="关闭提醒表单"><X size={18} /></button></header>
      <form className="watch-dialog-body" onSubmit={e => { e.preventDefault(); void save(); }}>
        <label className="field"><span>监控证券</span><select className="select" value={form.symbol} onChange={e => setForm({ ...form, symbol: e.target.value })}>{symbols.map(s => <option key={s.symbol} value={s.symbol}>{s.name} · {s.symbol}</option>)}</select></label>
        <label className="field"><span>触发条件</span><select className="select" value={form.kind} onChange={e => setForm({ ...form, kind: e.target.value, threshold: e.target.value.startsWith("change") ? 5 : e.target.value === "volume_ratio" ? 2 : e.target.value === "volatility" ? 40 : 100 })}>{Object.entries(ALERT_KINDS).map(([k, v]) => <option key={k} value={k}>{v}</option>)}</select></label>
        {!["breakout", "support_break"].includes(form.kind) ? <label className="field"><span>阈值{form.kind.startsWith("price") ? "（元）" : form.kind === "volume_ratio" ? "（倍）" : "（%）"}</span><input className="input" type="number" required step="any" value={Number.isFinite(form.threshold) ? form.threshold : ""} min={form.kind.startsWith("change") ? -100000 : 0.0001} max={1000000000} onChange={e => setForm({ ...form, threshold: e.target.valueAsNumber })} /></label> : null}
        <label className="field"><span>冷却时间（分钟）</span><input className="input" type="number" required min={1} max={10080} step={1} value={Number.isFinite(form.cooldown_minutes) ? form.cooldown_minutes : ""} onChange={e => setForm({ ...form, cooldown_minutes: e.target.valueAsNumber })} /></label>
        <label className="watch-check"><input type="checkbox" checked={form.enabled} onChange={e => setForm({ ...form, enabled: e.target.checked })} /> 启用提醒</label>
        <p className="panel-footnote">日频条件观察，无实时或盘中保证；技术条件需要真实复权日线。保存后由后台下一次检查生效。</p>
        {error ? <p role="alert" className="inline-notice">{error}</p> : null}
        <footer className="watch-dialog-foot"><button type="button" className="btn btn-ghost" disabled={busy} onClick={close}>取消</button><button type="submit" className="btn btn-solid" disabled={busy || !form.symbol}>{busy ? "保存中…" : "保存提醒"}</button></footer>
      </form>
    </dialog> : null}
  </section>;
}
