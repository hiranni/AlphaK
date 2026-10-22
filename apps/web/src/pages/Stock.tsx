import { useEffect, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ArrowLeft, ArrowUpRight, BookmarkPlus, ExternalLink, Layers, RotateCw } from "lucide-react";
import { api } from "../api";
import type { BarsResponse, Fundamentals, Profile, StockDetail, WatchlistGroup } from "../types";
import { Change } from "../components/Change";
import { ErrorState, Loading } from "../components/States";
import { KLineChart } from "../components/KLineChart";
import { changeClass, dateLabel, money, num, pct } from "../format";
import { realFundamentals, realNumber, realProfile, type SourceRow } from "../stockData";

type Tab = "overview" | "financials" | "profile";

export function Stock() {
  const { symbol = "" } = useParams();
  const navigate = useNavigate();
  const [storedStock, setStock] = useState<StockDetail | null>(null);
  const [metrics, setMetrics] = useState<SourceRow | null>(null);
  const [supplementStatus, setSupplementStatus] = useState<Record<string, string>>({});
  const stock = storedStock ? { ...storedStock, ...(metrics ? { total_mv: realNumber(metrics.total_mv), float_mv: realNumber(metrics.float_mv), pe_ttm: realNumber(metrics.pe_ttm), pb: realNumber(metrics.pb), turnover: realNumber(metrics.turnover) ?? storedStock.turnover } : {}) } : null;
  const [bars, setBars] = useState<BarsResponse | null>(null);
  const [fundamentals, setFundamentals] = useState<Fundamentals | null>(null);
  const [profile, setProfile] = useState<Profile | null>(null);
  const [groups, setGroups] = useState<WatchlistGroup[]>([]);
  const [error, setError] = useState("");
  const [barError, setBarError] = useState("");
  const [tab, setTab] = useState<Tab>("overview");
  const [period, setPeriod] = useState("daily");
  const [adjust, setAdjust] = useState("qfq");
  const [indicator, setIndicator] = useState<"macd" | "kdj" | "none">("macd");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const [reportIndex, setReportIndex] = useState(0);
  const [barReload, setBarReload] = useState(0);
  const [supplementReload, setSupplementReload] = useState(0);

  useEffect(() => {
    let active = true;
    setError("");
    setStock(null);
    setBars(null);
    setFundamentals(null);
    setProfile(null);
    setNotice("");
    setReportIndex(0);
    setMetrics(null);
    setSupplementStatus({});
    Promise.all([api.stock(symbol), api.watchlists()])
      .then(([nextStock, nextGroups]) => {
        if (!active) return;
        setStock(nextStock);
        setGroups(nextGroups);
      })
      .catch((err: Error) => {
        if (active) setError(err.message);
      });
    return () => {
      active = false;
    };
  }, [symbol]);

  useEffect(() => {
    let active = true;
    let loading = false;
    const load = async () => {
      if (loading) return;
      loading = true;
      await Promise.allSettled(["metrics", "profile", "financials"].map(async dataset => {
        try {
          const result = await api.supplement<SourceRow | SourceRow[]>(symbol, dataset);
          if (!active) return;
          if (dataset === "metrics") setMetrics(result.data as SourceRow);
          if (dataset === "profile") setProfile(realProfile(symbol, result.data as SourceRow, result.source, result.updated_at));
          if (dataset === "financials") setFundamentals(realFundamentals(symbol, result.data as SourceRow[]));
          setSupplementStatus(previous => ({ ...previous, [dataset]: `${result.source} · ${new Date(result.updated_at * 1000).toLocaleTimeString("zh-CN")}${result.stale ? " · 保留上次数据" : ""}` }));
        } catch (err) {
          if (active) setSupplementStatus(previous => ({ ...previous, [dataset]: err instanceof Error ? err.message : "更新失败" }));
        }
      }));
      loading = false;
    };
    void load();
    const timer = window.setInterval(() => void load(), 60000);
    return () => { active = false; window.clearInterval(timer); };
  }, [symbol, supplementReload]);

  useEffect(() => {
    let active = true;
    let loading = false;
    const timer = window.setInterval(() => {
      if (loading) return;
      loading = true;
      Promise.all([api.stock(symbol), api.bars(symbol, period, adjust)])
        .then(([nextStock, nextBars]) => { if (active) { setStock(nextStock); setBars(nextBars); } })
        .catch(() => { if (active) setNotice("行情更新失败，保留上次数据，稍后自动重试"); }).finally(() => { loading = false; });
    }, 60000);
    return () => { active = false; window.clearInterval(timer); };
  }, [symbol, period, adjust]);

  useEffect(() => {
    if (!symbol || stock?.symbol !== symbol) return;
    let active = true;
    setBarError("");
    setBars(null);
    api.bars(symbol, period, adjust)
      .then((nextBars) => {
        if (active) setBars(nextBars);
      })
      .catch((err: Error) => {
        if (active) setBarError(err.message);
      });
    return () => {
      active = false;
    };
  }, [symbol, stock?.symbol, period, adjust, barReload]);

  const selectedReport = useMemo(() => fundamentals?.reports[reportIndex], [fundamentals, reportIndex]);
  const selectedIndicator = useMemo(() => fundamentals?.indicators.find((item) => item.report_period === selectedReport?.report_period), [fundamentals, selectedReport]);
  const firstGroup = groups[0];

  const addToWatchlist = async () => {
    if (!firstGroup || !stock) return;
    setBusy(true);
    setNotice("");
    try {
      const next = await api.addItem(firstGroup.id, stock.symbol);
      setGroups(next);
      setNotice(`已加入「${firstGroup.name}」`);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "加入失败");
    } finally {
      setBusy(false);
    }
  };

  if (error) return <ErrorState message={error} onRetry={() => navigate(0)} />;
  if (!stock) return <Loading label="正在读取个股档案" />;

  return (
    <div className="page stock-page">
      <button className="back-link" onClick={() => navigate(-1)}><ArrowLeft size={14} /> 返回</button>

      <header className="stock-head">
        <div>
          <span className="eyebrow mono">{stock.exchange} · {stock.board} · {profile?.industry || stock.industry}</span>
          <h1 className="display stock-title">{stock.name}</h1>
          <div className="stock-sub">
            <span className="mono">{stock.symbol}</span>
            {profile?.list_date || stock.list_date ? <span className="mono muted">上市 {profile?.list_date || stock.list_date}</span> : null}
            <span className="mono muted">数据截至 {stock.as_of} · {stock.source}{stock.stale ? " · 已过期" : ""}</span>
            {stock.updated_at ? <span className="mono muted">更新 {new Date(stock.updated_at).toLocaleTimeString("zh-CN", { timeZone: "Asia/Shanghai", hour12: false })}</span> : null}
          </div>
        </div>
        <div className="stock-price-block">
          <span className={`stock-price mono ${changeClass(stock.change_pct)}`}>{num(stock.close)}</span>
          <Change value={stock.change_pct} />
          <span className="mono muted">昨收 {num(stock.prev_close)}</span>
        </div>
        <div className="stock-actions">
          <button className="btn btn-solid" onClick={addToWatchlist} disabled={busy || !firstGroup}>
            <BookmarkPlus size={15} /> {busy ? "加入中…" : "加入自选"}
          </button>
          <button className="btn btn-ghost" onClick={() => navigate("/watchlist")}><ArrowUpRight size={15} /> 查看自选</button>
          <button className="btn btn-ghost" disabled={busy} onClick={async () => { setBusy(true); try { await api.retrySupplements(symbol); setSupplementReload(value => value + 1); setBarReload(value => value + 1); setNotice("已重新请求补取"); } catch (err) { setNotice(err instanceof Error ? err.message : "请求失败"); } finally { setBusy(false); } }}><RotateCw size={14} /> 重新补取</button>
          {notice ? <span className="inline-notice" role="status">{notice}</span> : null}
        </div>
      </header>

      <section className="stock-stats">
        <div><span>今开</span><strong className="mono">{num(stock.open)}</strong></div>
        <div><span>最高</span><strong className="mono">{num(stock.high)}</strong></div>
        <div><span>最低</span><strong className="mono">{num(stock.low)}</strong></div>
        <div><span>成交额</span><strong className="mono">{stock.amount_known ? money(stock.amount) : "—"}</strong></div>
        <div><span>换手率</span><strong className="mono">{num(stock.turnover)}%</strong></div>
        <div><span>总市值</span><strong className="mono">{money(stock.total_mv)}</strong></div>
        <div><span>流通市值</span><strong className="mono">{money(stock.float_mv)}</strong></div>
        <div><span>PE / PB</span><strong className="mono">{num(stock.pe_ttm)} / {num(stock.pb)}</strong></div>
        <div><span>52 周高</span><strong className="mono">{num(stock.high_52w)}</strong></div>
        <div><span>52 周低</span><strong className="mono">{num(stock.low_52w)}</strong></div>
      </section>

      {stock.source !== "demo" ? (
        <p className="inline-notice" role="status">
          行情每60秒更新 · 估值 {supplementStatus.metrics || "补取中"} · 公司资料及财报独立补取，保留各自报告期。
        </p>
      ) : null}

      <div className="tabs" role="tablist" aria-label="个股详情">
        <button className={`tab ${tab === "overview" ? "is-active" : ""}`} onClick={() => setTab("overview")} role="tab" aria-selected={tab === "overview"}>概览</button>
        <button className={`tab ${tab === "financials" ? "is-active" : ""}`} onClick={() => setTab("financials")} role="tab" aria-selected={tab === "financials"}>财务</button>
        <button className={`tab ${tab === "profile" ? "is-active" : ""}`} onClick={() => setTab("profile")} role="tab" aria-selected={tab === "profile"}>公司资料</button>
      </div>

      {tab === "overview" ? (
        <div className="stock-grid">
          <section className="panel chart-panel">
            <div className="panel-head chart-head">
              <div>
                <span className="eyebrow mono">PRICE / VOLUME</span>
                <h2>价格结构</h2>
              </div>
              <div className="chart-controls">
                <div className="segmented" role="group" aria-label="周期">
                  {["1m", "5m", "15m", "30m", "60m", "daily", "weekly", "monthly"].map((value) => (
                    <button key={value} className={period === value ? "is-active" : ""} onClick={() => setPeriod(value)}>{value.endsWith("m") ? `${value.slice(0, -1)}分` : value === "daily" ? "日" : value === "weekly" ? "周" : "月"}</button>
                  ))}
                </div>
                <div className="segmented" role="group" aria-label="复权">
                  {["qfq", "none", "hfq"].map((value) => (
                    <button key={value} disabled={period.endsWith("m")} className={adjust === value ? "is-active" : ""} onClick={() => setAdjust(value)}>{value === "qfq" ? "前复权" : value === "none" ? "不复权" : "后复权"}</button>
                  ))}
                </div>
                <div className="segmented" role="group" aria-label="指标">
                  {(["macd", "kdj", "none"] as const).map((value) => (
                    <button key={value} className={indicator === value ? "is-active" : ""} onClick={() => setIndicator(value)}>{value === "macd" ? "MACD" : value === "kdj" ? "KDJ" : "关闭"}</button>
                  ))}
                </div>
              </div>
            </div>
            {barError ? <ErrorState message={barError} onRetry={() => setBarReload((value) => value + 1)} /> : null}
            {bars?.notice ? <p className="inline-notice" role="status">{bars.notice}</p> : null}
            {bars ? <KLineChart bars={bars.bars} indicator={indicator} /> : !barError ? <Loading label="正在绘制 K 线" /> : null}
            <div className="chart-legend">
              <span className="mono"><i className="legend-dot ma5" /> MA5</span>
              <span className="mono"><i className="legend-dot ma20" /> MA20</span>
              <span className="mono"><i className="legend-dot ma60" /> MA60</span>
              <span className="mono muted">{bars ? `${bars.adjust === "qfq" ? "前复权" : bars.adjust === "hfq" ? "后复权" : "不复权"} / ${period.endsWith("m") ? `${period.slice(0, -1)}分钟` : period === "daily" ? "日线" : period === "weekly" ? "周线" : "月线"}` : "—"}</span>
            </div>
          </section>

          <aside className="stock-side">
            <section className="panel valuation-panel">
              <div className="panel-head">
                <div><span className="eyebrow mono">VALUATION</span><h2>估值位置</h2></div>
                <Layers size={17} />
              </div>
              <div className="valuation-grid">
                <div><span>PE(TTM)</span><strong className="mono">{num(stock.pe_ttm)}</strong></div>
                <div><span>PB</span><strong className="mono">{num(stock.pb)}</strong></div>
                <div><span>总股本</span><strong className="mono">{money(realNumber(metrics?.total_shares))}</strong></div>
                <div><span>流通股本</span><strong className="mono">{money(realNumber(metrics?.float_shares))}</strong></div>
              </div>
              <p className="panel-footnote">{supplementStatus.metrics || "正在补取真实估值数据"}</p>
            </section>

            <section className="panel">
              <div className="panel-head"><div><span className="eyebrow mono">FLOW</span><h2>数据边界</h2></div><CircleNote /></div>
              <ul className="fact-list">
                <li><span>行情来源</span><b className="mono">{stock.source}{stock.stale ? "（已过期）" : ""}</b></li>
                <li><span>行情日期</span><b className="mono">{stock.as_of}</b></li>
                <li><span>公司资料</span><b className="mono">{profile?.updated_at ? dateLabel(profile.updated_at) : "—"}</b></li>
                <li><span>基本面</span><b className="mono">{selectedReport?.report_period ?? "—"}</b></li>
              </ul>
              <div className="panel-note-block">只看当前标的，不做全市场筛选。股票池与策略执行属于后续版本。</div>
            </section>
          </aside>
        </div>
      ) : null}

      {tab === "financials" ? (
        <section className="panel financial-panel">
          <div className="panel-head">
            <div><span className="eyebrow mono">FINANCIALS</span><h2>财务快照</h2></div>
            <div className="chart-controls">
              <select className="select" value={reportIndex} onChange={(event) => setReportIndex(Number(event.target.value))} aria-label="选择报告期">
                {fundamentals?.reports.map((report, index) => (
                  <option key={report.report_period} value={index}>{report.report_period} · {report.statement_type === "annual" ? "年报" : "季报"}</option>
                ))}
              </select>
            </div>
          </div>
          {selectedReport ? (
            <>
              <div className="report-meta">
                <span>报告期 <b className="mono">{selectedReport.report_period}</b></span>
                <span>公告日 <b className="mono">{selectedReport.announce_date}</b></span>
                <span>口径 <b>合并报表</b></span>
              </div>
              <div className="financial-grid">
                <div className="fin-card">
                  <span className="eyebrow mono">INCOME</span>
                  <h3>利润表</h3>
                  <dl>
                    <div><dt>营业收入</dt><dd className="mono">{money(selectedReport.revenue)}</dd></div>
                    <div><dt>归母净利润</dt><dd className="mono">{money(selectedReport.net_profit)}</dd></div>
                    <div><dt>归母净利率</dt><dd className="mono">{selectedIndicator?.net_margin != null ? `${selectedIndicator.net_margin.toFixed(2)}%` : "—"}</dd></div>
                  </dl>
                </div>
                <div className="fin-card">
                  <span className="eyebrow mono">BALANCE</span>
                  <h3>资产负债表</h3>
                  <dl>
                    <div><dt>总资产</dt><dd className="mono">{money(selectedReport.total_assets)}</dd></div>
                    <div><dt>总负债</dt><dd className="mono">{money(selectedReport.total_liabilities)}</dd></div>
                    <div><dt>所有者权益</dt><dd className="mono">{money(selectedReport.equity)}</dd></div>
                  </dl>
                </div>
                <div className="fin-card">
                  <span className="eyebrow mono">CASH FLOW</span>
                  <h3>现金流量表</h3>
                  <dl>
                    <div><dt>经营现金流</dt><dd className="mono">{money(selectedReport.operating_cash_flow)}</dd></div>
                    <div><dt>ROE</dt><dd className="mono">{selectedIndicator?.roe != null ? `${selectedIndicator.roe.toFixed(2)}%` : "—"}</dd></div>
                    <div><dt>资产负债率</dt><dd className="mono">{selectedIndicator?.debt_ratio != null ? `${selectedIndicator.debt_ratio.toFixed(2)}%` : "—"}</dd></div>
                  </dl>
                </div>
              </div>
              <div className="table-wrap">
                <table className="data-table">
                  <thead><tr><th>报告期</th><th className="num">营收</th><th className="num">净利润</th><th className="num">ROE</th><th className="num">毛利率</th><th className="num">负债率</th><th>公告日</th></tr></thead>
                  <tbody>
                    {fundamentals?.reports.map((report) => {
                      const indicator = fundamentals.indicators.find((item) => item.report_period === report.report_period);
                      return (
                        <tr key={report.report_period} onClick={() => setReportIndex(fundamentals.reports.indexOf(report))} className="row-link">
                          <td className="mono">{report.report_period}</td>
                          <td className="num mono">{money(report.revenue)}</td>
                          <td className="num mono">{money(report.net_profit)}</td>
                          <td className="num mono">{indicator?.roe != null ? `${indicator.roe.toFixed(1)}%` : "—"}</td>
                          <td className="num mono">{indicator?.gross_margin != null ? `${indicator.gross_margin.toFixed(1)}%` : "—"}</td>
                          <td className="num mono">{indicator?.debt_ratio != null ? `${indicator.debt_ratio.toFixed(1)}%` : "—"}</td>
                          <td className="mono muted">{report.announce_date}</td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            </>
          ) : supplementStatus.financials ? <p className="inline-notice" role="status">{supplementStatus.financials}</p> : <Loading label="正在补取财务数据" />}
          <p className="panel-footnote">{supplementStatus.financials || "东方财富财报 · 按报告期匹配三张报表"} · 本年累计口径</p>
        </section>
      ) : null}

      {tab === "profile" ? (
        <section className="panel profile-panel">
          <div className="panel-head">
            <div><span className="eyebrow mono">COMPANY</span><h2>公司资料</h2></div>
            <span className="panel-note">来源 {profile?.source ?? "—"} · 更新 {profile ? dateLabel(profile.updated_at) : "—"}</span>
          </div>
          {profile ? (
            <div className="profile-grid">
              <div className="profile-copy">
                <h3>{profile.full_name}</h3>
                <p>{profile.intro}</p>
                <p className="muted">{profile.business}</p>
              </div>
              <dl className="profile-facts">
                <div><dt>所属行业</dt><dd>{profile.industry}</dd></div>
                <div><dt>上市日期</dt><dd className="mono">{profile.list_date}</dd></div>
                <div><dt>实际控制人</dt><dd>{profile.controller}</dd></div>
                <div><dt>地区</dt><dd>{profile.region}</dd></div>
                <div><dt>网站</dt><dd><a href={profile.website} target="_blank" rel="noreferrer">{profile.website} <ExternalLink size={12} /></a></dd></div>
              </dl>
            </div>
          ) : supplementStatus.profile ? <p className="inline-notice" role="status">{supplementStatus.profile}</p> : <Loading label="正在补取公司资料" />}
          <div className="panel-note-block">{supplementStatus.profile || "东方财富公司资料"}</div>
        </section>
      ) : null}
    </div>
  );
}

function CircleNote() {
  return <RotateCw size={16} aria-hidden="true" />;
}
