import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ArrowLeft, ArrowUpRight } from "lucide-react";
import { api } from "../api";
import type { IndexDetail, MemberRow, SectorDetail } from "../types";
import { Change } from "../components/Change";
import { Empty, ErrorState, Loading } from "../components/States";
import { money, num } from "../format";

type Kind = "index" | "sector";

export function MarketDetail({ kind }: { kind: Kind }) {
  const params = useParams();
  const navigate = useNavigate();
  const [data, setData] = useState<IndexDetail | SectorDetail | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    setData(null);
    setError("");
    let active = true;
    const load = () => {
      const request = kind === "index" ? api.index(params.code ?? "") : api.sector(Number(params.id ?? 0));
      request.then((next) => { if (active) { setData(next); setError(""); } }).catch((err: Error) => { if (active) setError(err.message); });
    };
    load();
    const timer = window.setInterval(load, 10000);
    return () => { active = false; window.clearInterval(timer); };
  }, [kind, params.code, params.id]);

  if (error && !data) return <ErrorState message={error} onRetry={() => navigate(0)} />;
  if (!data) return <Loading label="正在读取详情" />;

  const isIndex = "code" in data;
  const title = data.name;
  const members: MemberRow[] = data.members;

  return (
    <div className="page detail-page">
      <button className="back-link" onClick={() => navigate(-1)}><ArrowLeft size={14} /> 返回</button>
      <header className="detail-head">
        <div>
          <span className="eyebrow mono">{isIndex ? `INDEX / ${data.code}` : `SECTOR / ${data.sector_type}`}</span>
          <h1 className="display">{title}</h1>
        </div>
        <div className="detail-quote">
          <strong className="mono">{isIndex ? num(data.close) : "—"}</strong>
          <Change value={data.change_pct} />
          {isIndex ? <Change value={data.change_points} points /> : null}
          <span className="mono muted">{isIndex ? `成交额 ${data.amount_known ? money(data.amount) : "—"} · ${data.as_of}` : `成员 ${members.length} 只`}</span>
        </div>
      </header>
      {error ? <p className="inline-notice" role="status">更新失败，保留上次数据：{error}</p> : null}

      <section className="panel">
        <div className="panel-head">
          <div><span className="eyebrow mono">CONSTITUENTS</span><h2>成分 / 成员</h2></div>
          <span className="panel-note">{members.length} 只 · 演示数据</span>
        </div>
        {members.length ? (
          <div className="table-wrap">
            <table className="data-table">
              <thead><tr><th>证券</th><th className="num">权重</th><th className="num">最新价</th><th className="num">涨跌幅</th><th className="num">操作</th></tr></thead>
              <tbody>
                {members.map((member) => (
                  <tr key={member.symbol}>
                    <td><span className="cell-title">{member.name}</span><span className="cell-sub mono">{member.symbol}</span></td>
                    <td className="num mono">{(member.weight * 100).toFixed(2)}%</td>
                    <td className="num mono">{num(member.close)}</td>
                    <td className="num"><Change value={member.change_pct} /></td>
                    <td className="num"><button className="icon-btn" aria-label={`打开 ${member.name}`} onClick={() => navigate(`/stock/${member.symbol}`)}><ArrowUpRight size={14} /></button></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : <Empty title="暂无成分数据" note="数据源未提供成分时，不用伪造成分填充。" />}
      </section>
    </div>
  );
}
