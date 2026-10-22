import { useEffect, useState } from "react";
import { Database, Play, RefreshCw, ShieldCheck } from "lucide-react";
import { api } from "../api";
import type { DataJob, QualityReport, SourceStatus } from "../types";
import { ErrorState, Loading } from "../components/States";
import { dateLabel } from "../format";

const labels: Record<string, string> = {
  instruments: "证券主数据",
  quote_daily: "日线行情",
  daily_basic: "每日基础指标",
  financial_report: "财务报表",
  index_quote: "指数行情",
  company_profile: "公司资料",
  sector_catalog: "板块成员",
};

const statusLabels: Record<string, string> = {
  succeeded: "已完成",
  failed: "失败",
  running: "进行中",
  queued: "排队中",
};

export function DataAdmin() {
  const [quality, setQuality] = useState<QualityReport[]>([]);
  const [jobs, setJobs] = useState<DataJob[]>([]);
  const [sources, setSources] = useState<SourceStatus | null>(null);
  const [sourceBusy, setSourceBusy] = useState("");
  const [sourceNotice, setSourceNotice] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState("");
  const [notice, setNotice] = useState("");

  const load = () => {
    setError("");
    Promise.all([api.quality(), api.jobs(), api.sourceStatus()])
      .then(([nextQuality, nextJobs, nextSources]) => {
        setQuality(nextQuality);
        setJobs(nextJobs);
        setSources(nextSources);
      })
      .catch((err: Error) => setError(err.message));
  };

  useEffect(load, []);

  useEffect(() => {
    let active = true;
    const timer = window.setInterval(() => {
      Promise.all([api.quality(), api.jobs(), api.sourceStatus()]).then(([nextQuality, nextJobs, nextSources]) => {
        if (active) { setQuality(nextQuality); setJobs(nextJobs); setSources(nextSources); }
      }).catch(() => undefined);
    }, 10000);
    return () => { active = false; window.clearInterval(timer); };
  }, []);

  const runSource = async (dataset: string) => {
    setSourceBusy(dataset);
    setSourceNotice("");
    const end = new Date().toISOString().slice(0, 10);
    const start = new Date(Date.now() - 7 * 86400000).toISOString().slice(0, 10);
    const symbols = dataset === "index_quote"
      ? ["000001.SH", "000300.SH", "000905.SH", "399001.SZ", "399006.SZ"]
      : [
          "000333.SZ", "000651.SZ", "000858.SZ", "002594.SZ", "300059.SZ",
          "300750.SZ", "600030.SH", "600036.SH", "600519.SH", "600900.SH",
          "601166.SH", "601318.SH", "601899.SH",
        ];
    try {
      const result = await api.sourceSync(dataset, symbols, start, end);
      setSourceNotice(`${labels[dataset] ?? dataset} 公开源同步完成：${JSON.stringify(result)}`);
      load();
    } catch (err) {
      setSourceNotice(err instanceof Error ? err.message : "公开源同步失败");
    } finally {
      setSourceBusy("");
    }
  };

  const run = async (dataset: string) => {
    setBusy(dataset);
    setNotice("");
    try {
      setJobs(await api.runJob(dataset));
      setNotice(`已触发 ${labels[dataset] ?? dataset} 同步（演示任务立即完成）`);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "触发失败");
    } finally {
      setBusy("");
    }
  };

  if (error) return <ErrorState message={error} onRetry={load} />;
  if (!quality.length) return <Loading label="正在读取数据体检" />;

  return (
    <div className="page data-page">
      <header className="page-head compact-head">
        <div>
          <h1 className="display">数据管理</h1>
        </div>
        <div className="head-aside">
          <span className="stamp">来源与同步状态</span>
          <div className="head-aside-row"><span className="mono muted">数据集</span><span className="mono">{quality.length}</span></div>
          <div className="head-aside-row"><span className="mono muted">最近任务</span><span className="mono">{jobs[0] ? statusLabels[jobs[0].status] ?? jobs[0].status : "—"}</span></div>
        </div>
      </header>

      {sources ? (
        <section className="panel source-panel">
          <div className="panel-head">
            <div><span className="eyebrow mono">SOURCES</span><h2>行情来源</h2></div>
            <span className={`stamp ${sources.freshness.some((item) => item.status !== "fresh") ? "stamp-warn" : ""}`}>
              {sources.freshness.length === 0 ? "未同步" : sources.freshness.some((item) => item.status !== "fresh") ? "部分过期" : "数据新鲜"}
            </span>
          </div>
          <div className="source-grid">
            {sources.health.map((source) => (
              <div className={`source-card ${source.available ? "" : "source-card-down"}`} key={source.source}>
                <div className="source-card-head">
                  <span>{source.source === "tencent" ? "腾讯" : source.source === "sina" ? "新浪财经" : "东方财富"}</span>
                  <span className={`status-pill ${source.available ? "status-succeeded" : "status-failed"}`}>
                    {source.available ? "可用" : `冷却 ${source.cooldown_seconds}s`}
                  </span>
                </div>
                <p className="mono muted">失败 {source.failures} 次{source.last_error ? ` · ${source.last_error.slice(0, 80)}` : ""}</p>
              </div>
            ))}
          </div>
          <div className="freshness-list">
            {sources.freshness.map((item) => (
              <div className="freshness-row" key={item.dataset}>
                <span className="mono">{labels[item.dataset] ?? item.dataset}</span>
                <span className={`status-pill status-${item.status === "fresh" ? "succeeded" : item.status === "partial" ? "running" : "failed"}`}>
                  {item.status === "fresh" ? "新鲜" : item.status === "partial" ? "部分成功" : "已过期"}
                </span>
                <span className="mono muted">{item.last_success_at ? dateLabel(item.last_success_at) : "无成功记录"}</span>
                {item.last_source ? <span className="mono muted">来源 {item.last_source}</span> : null}
              </div>
            ))}
          </div>
          <p className="panel-footnote">行情快照每60秒更新，历史日线每15分钟补齐；腾讯、东方财富、新浪依次切换，每源最多2次。失败范围每分钟重试，最多5次，达到上限后停止并保留上次数据。待处理 {sources.retry_queue.length} 个范围，其中 {sources.retry_queue.filter((task) => task.attempts >= 5).length} 个已达上限，可手动重新同步。分钟线、估值和资料在个股页查看状态及重新补取。</p>
        </section>
      ) : null}

      <section className="panel run-panel">
        <div className="panel-head">
          <div><span className="eyebrow mono">PUBLIC SOURCES</span><h2>公开源同步</h2></div>
          <Database size={17} />
        </div>
        <p className="panel-footnote">按数据集主备；第一轮为小样本。失败时保留最后成功数据，不生成 mock。</p>
        {sourceNotice ? <p className="inline-notice" role="status">{sourceNotice}</p> : null}
        <div className="run-grid">
          {[
            ["quote_daily", "个股日线"],
            ["snapshot", "行情快照"],
            ["index_quote", "指数行情"],
          ].map(([dataset, label]) => (
            <button key={dataset} className="run-card" onClick={() => runSource(dataset)} disabled={sourceBusy === dataset}>
              <span>{label}</span>
              <span className="mono">{sourceBusy === dataset ? "同步中…" : "腾讯 / 东财"}</span>
              <Play size={15} aria-hidden="true" />
            </button>
          ))}
        </div>
      </section>

      <div className="data-grid">
        <section className="panel">
          <div className="panel-head">
            <div><span className="eyebrow mono">COVERAGE</span><h2>数据体检</h2></div>
            <ShieldCheck size={17} />
          </div>
          <div className="quality-list">
            {quality.map((item) => (
              <div className="quality-row" key={item.dataset}>
                <div className="quality-head">
                  <span>{labels[item.dataset] ?? item.dataset}</span>
                  <span className="mono">{(item.coverage * 100).toFixed(1)}%</span>
                </div>
                <div className="quality-track"><span style={{ width: `${Math.min(item.coverage * 100, 100)}%` }} /></div>
                <div className="quality-meta mono">
                  <span>缺失 {item.missing}</span>
                  <span>重复 {item.duplicates}</span>
                  <span>异常 {item.anomalies}</span>
                  <span>{dateLabel(item.updated_at)}</span>
                </div>
              </div>
            ))}
          </div>
        </section>

        <section className="panel">
          <div className="panel-head">
            <div><span className="eyebrow mono">JOBS</span><h2>同步任务</h2></div>
            <button className="btn btn-ghost" onClick={load}><RefreshCw size={14} /> 刷新</button>
          </div>
          {notice ? <p className="inline-notice" role="status">{notice}</p> : null}
          <div className="table-wrap">
            <table className="data-table">
              <thead><tr><th>数据集</th><th>状态</th><th className="num">进度</th><th>说明</th><th>开始</th></tr></thead>
              <tbody>
                {jobs.map((job) => (
                  <tr key={job.id}>
                    <td><span className="cell-title">{labels[job.dataset] ?? job.dataset}</span><span className="cell-sub mono">{job.dataset}</span></td>
                    <td><span className={`status-pill status-${job.status}`}>{statusLabels[job.status] ?? job.status}</span></td>
                    <td className="num mono">{job.progress}%</td>
                    <td className="muted">{job.message}</td>
                    <td className="mono muted">{dateLabel(job.started_at)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      </div>

      <section className="panel run-panel">
        <div className="panel-head">
          <div><span className="eyebrow mono">MANUAL RUN</span><h2>手动触发</h2></div>
          <Database size={17} />
        </div>
        <p className="panel-footnote">MVP 使用演示种子；真实接入时，启动阶段只加载最近数据，历史由后台回填。</p>
        <div className="run-grid">
          {Object.entries(labels).map(([key, label]) => (
            <button key={key} className="run-card" onClick={() => run(key)} disabled={busy === key}>
              <span>{label}</span>
              <span className="mono">{busy === key ? "触发中…" : "运行同步"}</span>
              <Play size={15} aria-hidden="true" />
            </button>
          ))}
        </div>
      </section>
    </div>
  );
}
