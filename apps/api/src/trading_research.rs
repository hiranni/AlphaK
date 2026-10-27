//! Manually scoped research workspace. No Agent, account ledger or order writes.
use crate::{
    db::AppState,
    error::{envelope, ApiResponse, AppError},
    persistence,
};
use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{Datelike, FixedOffset, NaiveDate, Timelike, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
const ENGINE: &str = "research-analysis-1";
const IDS: [&str; 8] = [
    "ak-state-v1",
    "ak-regime-v1",
    "ak-continuation-v1",
    "ak-repair-v1",
    "ak-cooling-v1",
    "ak-event-v1",
    "ak-quality-v1",
    "ak-ensemble-v1",
];
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Allocation {
    pub model_id: String,
    pub weight: f64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub name: String,
    pub symbols: Vec<String>,
    pub allocations: Vec<Allocation>,
    pub sentiment_scope: String,
}
pub(crate) fn validate(x: &Config) -> Result<(), AppError> {
    if x.name.trim().is_empty() || x.name.chars().count() > 60 {
        return Err(AppError::bad_request("名称需为1–60个字符"));
    }
    let unique: BTreeSet<_> = x.symbols.iter().collect();
    if x.symbols.is_empty() || x.symbols.len() > 8 || unique.len() != x.symbols.len() {
        return Err(AppError::bad_request("必须选择1–8只不重复股票"));
    }
    if !["pool", "market"].contains(&x.sentiment_scope.as_str()) {
        return Err(AppError::bad_request("情绪范围必须为pool或market"));
    }
    let models: BTreeSet<_> = x.allocations.iter().map(|a| &a.model_id).collect();
    if x.allocations.is_empty()
        || x.allocations.len() > 8
        || models.len() != x.allocations.len()
        || x.allocations.iter().any(|a| {
            !IDS.contains(&a.model_id.as_str())
                || !a.weight.is_finite()
                || a.weight <= 0.
                || a.weight > 1.
        })
        || x.allocations.iter().map(|a| a.weight).sum::<f64>() > 1. + 1e-9
    {
        return Err(AppError::bad_request(
            "策略必须唯一，权重大于0且合计不超过100%",
        ));
    }
    Ok(())
}
pub fn init(c: &Connection) -> rusqlite::Result<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS research_workspace(id TEXT PRIMARY KEY,config TEXT NOT NULL,created_at TEXT NOT NULL); CREATE TABLE IF NOT EXISTS research_analysis(id TEXT PRIMARY KEY,workspace_id TEXT NOT NULL REFERENCES research_workspace(id),input_hash TEXT NOT NULL,input TEXT NOT NULL,report TEXT NOT NULL,created_at TEXT NOT NULL,UNIQUE(workspace_id,input_hash)); CREATE INDEX IF NOT EXISTS research_analysis_recent ON research_analysis(workspace_id,created_at DESC);")
}
pub async fn list(State(s): State<AppState>) -> Result<Json<ApiResponse<Value>>, AppError> {
    let out=persistence::run_db(s,|c|{
 let mut stmt=c.prepare("SELECT id,config,created_at FROM research_workspace ORDER BY created_at DESC,id LIMIT 100")?;
 let rows=stmt.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?)))?.collect::<Result<Vec<_>,_>>()?;
 let mut result=vec![];
 for(id,raw,created) in rows{
 let latest:Option<String>=c.query_row("SELECT report FROM research_analysis WHERE workspace_id=?1 ORDER BY created_at DESC,id DESC LIMIT 1",[&id],|r|r.get(0)).optional()?;
 result.push(json!({"id":id,"config":serde_json::from_str::<Value>(&raw).unwrap_or(Value::Null),"created_at":created,"latest":latest.and_then(|x|serde_json::from_str::<Value>(&x).ok())}));
 }Ok(json!(result))}).await?;
    Ok(envelope(out))
}
pub async fn create(
    State(s): State<AppState>,
    Json(x): Json<Config>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    validate(&x)?;
    let out=persistence::run_db(s,move|c|{
 for symbol in &x.symbols{
 let valid:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM instruments WHERE symbol=?1 AND exchange IN ('SSE','SZSE','SH','SZ') AND status NOT IN ('delisted','inactive'))",[symbol],|r|r.get(0))?;
 if !valid{return Err(AppError::bad_request(format!("股票不在有效沪深样本中：{symbol}")))}
 }
 let id=uuid::Uuid::new_v4().to_string();
 c.execute("INSERT INTO research_workspace VALUES(?1,?2,?3)",params![id,serde_json::to_string(&x).unwrap(),Utc::now().to_rfc3339()])?;
 Ok(json!({"id":id}))}).await?;
    Ok(envelope(out))
}
#[derive(Clone, Serialize, Deserialize, Debug)]
struct Bar {
    date: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    source: String,
    fetched_at: String,
}
#[derive(Clone, Serialize, Deserialize)]
struct Stock {
    symbol: String,
    name: String,
    bars: Vec<Bar>,
}
fn avg(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len().max(1) as f64
}
fn sd(x: &[f64]) -> f64 {
    let m = avg(x);
    avg(&x.iter().map(|v| (v - m).powi(2)).collect::<Vec<_>>()).sqrt()
}
fn state(b: &[Bar], i: usize, breadth: f64) -> [f64; 7] {
    let returns: Vec<_> = b[i - 20..=i]
        .windows(2)
        .map(|w| w[1].close / w[0].close - 1.)
        .collect();
    let vol = sd(&returns).max(0.003);
    let ma = avg(&b[i - 19..=i].iter().map(|x| x.close).collect::<Vec<_>>());
    let vm = avg(&b[i - 20..i].iter().map(|x| x.volume).collect::<Vec<_>>());
    [
        b[i].close / b[i - 5].close - 1.,
        b[i].close / b[i - 20].close - 1.,
        b[i].close / ma - 1.,
        vol,
        if vm > 0. { b[i].volume / vm } else { 0. },
        b[i].close / b[i - 59..=i].iter().map(|x| x.close).fold(0., f64::max) - 1.,
        breadth,
    ]
}
fn prediction(
    b: &[Bar],
    i: usize,
    breadth: &std::collections::BTreeMap<String, f64>,
    window: usize,
    k: usize,
    caution: f64,
) -> Option<Value> {
    let current = state(b, i, *breadth.get(&b[i].date)?);
    // Five trading sessions open-to-open; label endpoint strictly before decision close.
    let rows: Vec<_> = (60..i)
        .filter(|j| j + 6 < i && j + window >= i)
        .filter_map(|j| {
            Some((
                j,
                state(b, j, *breadth.get(&b[j].date)?),
                b[j + 6].open / b[j + 1].open - 1.,
            ))
        })
        .collect();
    if rows.len() < 40 || rows.len() < k {
        return None;
    }
    let means: [f64; 7] =
        std::array::from_fn(|n| avg(&rows.iter().map(|r| r.1[n]).collect::<Vec<_>>()));
    let scales: [f64; 7] =
        std::array::from_fn(|n| sd(&rows.iter().map(|r| r.1[n]).collect::<Vec<_>>()).max(1e-5));
    let mut near: Vec<_> = rows
        .iter()
        .map(|r| {
            (
                (0..7)
                    .map(|n| ((r.1[n] - current[n]) / scales[n]).clamp(-5., 5.).powi(2))
                    .sum::<f64>(),
                r,
            )
        })
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1 .0.cmp(&b.1 .0)));
    near.truncate(k);
    let ys: Vec<_> = near.iter().map(|(_, r)| r.2).collect();
    let mean = avg(&ys);
    let uncertainty = sd(&ys) / (k as f64 / 5.).max(1.).sqrt();
    let mut sorted = ys.clone();
    sorted.sort_by(f64::total_cmp);
    let downside = (-sorted[((k - 1) as f64 * 0.1).floor() as usize]).max(0.02);
    Some(
        json!({"expected_return":mean,"net_edge":mean-0.002-caution*uncertainty,"uncertainty":uncertainty,"downside":downside,"samples":rows.len(),"params":{"window":window,"neighbors":k,"caution":caution},"analogues":near.iter().take(5).map(|(_,r)|json!({"date":b[r.0].date,"label_available":b[r.0+6].date,"return":r.2})).collect::<Vec<_>>(),"features":current,"training_means":means}),
    )
}
fn forecast(b: &[Bar], breadth: &std::collections::BTreeMap<String, f64>) -> Option<Value> {
    let i = b.len() - 1;
    let mut best: Option<(f64, usize, usize, f64)> = None;
    let mut trials = vec![];
    for window in [80, 120] {
        for k in [8, 16, 24] {
            for caution in [0., 0.5, 1.] {
                let mut rewards = vec![];
                for v in ((i.saturating_sub(40)).max(110)..i.saturating_sub(6)).step_by(5) {
                    if let Some(p) = prediction(b, v, breadth, window, k, caution) {
                        let invested = p["net_edge"].as_f64().unwrap() > 0.;
                        rewards.push(if invested {
                            b[v + 6].open / b[v + 1].open - 1. - 0.002
                        } else {
                            0.
                        });
                    }
                }
                if rewards.len() < 4 {
                    continue;
                }
                let score = avg(&rewards) - 0.5 * sd(&rewards);
                trials.push(json!({"window":window,"neighbors":k,"caution":caution,"score":score,"validation_events":rewards.len()}));
                if best.as_ref().is_none_or(|x| score > x.0) {
                    best = Some((score, window, k, caution));
                }
            }
        }
    }
    let (score, w, k, c) = best?;
    let mut p = prediction(b, i, breadth, w, k, c)?;
    p["validation_score"] = json!(score);
    p["trials"] = json!(trials);
    Some(p)
}
fn analyze(config: &Config, stocks: &[Stock], cutoff: &str) -> Value {
    let dates: BTreeSet<_> = stocks
        .iter()
        .flat_map(|s| s.bars.iter().map(|b| b.date.clone()))
        .collect();
    let mut breadth = std::collections::BTreeMap::new();
    for date in dates {
        let mut up = 0;
        let mut count = 0;
        for s in stocks {
            if let Some(i) = s.bars.iter().position(|b| b.date == date) {
                if i > 0 {
                    count += 1;
                    up += usize::from(s.bars[i].close > s.bars[i - 1].close)
                }
            }
        }
        if count == stocks.len() {
            breadth.insert(date, up as f64 / count as f64);
        }
    }
    let forecasts: Vec<_> = stocks
        .iter()
        .map(|s| {
            if s.bars.len() >= 180 {
                forecast(&s.bars, &breadth)
            } else {
                None
            }
        })
        .collect();
    let latest_dates: BTreeSet<_> = stocks
        .iter()
        .filter_map(|s| s.bars.last().map(|b| b.date.as_str()))
        .collect();
    let common = if latest_dates.len() == 1 {
        latest_dates.first().copied()
    } else {
        None
    };
    let today = NaiveDate::parse_from_str(cutoff, "%Y-%m-%d").unwrap();
    let fresh = common
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .is_some_and(|d| (today - d).num_days() <= 5);
    let sentiment = common.and_then(|d| breadth.get(d)).copied();
    let mut models = vec![];
    for alloc in &config.allocations {
        let dependency = match alloc.model_id.as_str() {
            "ak-event-v1" => Some("事件适配器未实现；需要PIT公告、去重文本与历史预期基线"),
            "ak-quality-v1" => Some("基本面适配器未实现；需要PIT财报、行业适配与历史估值"),
            "ak-ensemble-v1" => Some("组合适配器未实现；需要子模型历史样本外预测和实例资金记录"),
            _ => None,
        };
        let global = dependency.or(if config.sentiment_scope == "market" {
            Some("全市场历史情绪适配器尚未实现；不会改用样本情绪")
        } else if !fresh || sentiment.is_none() {
            Some("池内最新日期未对齐、情绪缺失或行情过期")
        } else {
            None
        });
        let mut rows = vec![];
        for (s, p) in stocks.iter().zip(&forecasts) {
            if let Some(reason) = global {
                rows.push(json!({"symbol":s.symbol,"name":s.name,"status":"blocked","reason":reason,"bars":s.bars.len(),"target_ceiling":0.}));
                continue;
            }
            let Some(p) = p else {
                rows.push(json!({"symbol":s.symbol,"name":s.name,"status":"blocked","reason":"至少180根有效日线，且历史同日池内情绪与成熟标签需足够","bars":s.bars.len(),"target_ceiling":0.}));
                continue;
            };
            let f = p["features"].as_array().unwrap();
            let trend = f[2].as_f64().unwrap();
            let vr = f[4].as_f64().unwrap();
            let dd = f[5].as_f64().unwrap();
            let heat = sentiment.unwrap();
            let previous = s
                .bars
                .get(s.bars.len() - 2)
                .and_then(|b| breadth.get(&b.date))
                .copied()
                .unwrap_or(heat);
            let improving = heat > previous;
            let priorhigh = s.bars[s.bars.len() - 21..s.bars.len() - 1]
                .iter()
                .map(|b| b.high)
                .fold(0., f64::max);
            let gate = match alloc.model_id.as_str() {
                "ak-state-v1" => heat >= 0.25,
                "ak-regime-v1" => heat >= 0.5 && trend > 0.,
                "ak-continuation-v1" => {
                    s.bars.last().unwrap().close > priorhigh && vr >= 1.2 && heat >= 0.5
                }
                "ak-repair-v1" => {
                    dd < -0.08
                        && improving
                        && s.bars.last().unwrap().close > s.bars[s.bars.len() - 2].close
                }
                "ak-cooling-v1" => heat < previous && trend > 0.05 && vr > 1.5,
                _ => false,
            };
            let edge = p["net_edge"].as_f64().unwrap();
            let risk = p["downside"].as_f64().unwrap();
            let validated = p["validation_score"].as_f64().unwrap() > 0.;
            let defensive = alloc.model_id == "ak-cooling-v1";
            let action = if defensive && gate {
                "reduce_review"
            } else if !defensive && gate && edge > 0. && validated {
                "buy_candidate"
            } else {
                "wait"
            };
            let ceiling = if action == "buy_candidate" {
                (0.2f64).min(0.8 / stocks.len() as f64).min(0.005 / risk)
                    * (edge / risk).clamp(0., 1.)
            } else {
                0.
            };
            rows.push(json!({"symbol":s.symbol,"name":s.name,"status":"analyzed","bars":s.bars.len(),"as_of":s.bars.last().unwrap().date,"action":action,"target_ceiling":ceiling,"account_ceiling":ceiling*alloc.weight,"reason":if defensive{"只提示已有持仓减仓复核；本研究配置无持仓，不创建卖单"}else if action=="buy_candidate"{"历史近邻净优势、价格/情绪研究门槛通过；待实例风控复核"}else{"模型门槛或历史验证未通过，保持等待"},"forecast":p}));
        }
        let blocked = rows.iter().filter(|r| r["status"] == "blocked").count();
        models.push(json!({"model_id":alloc.model_id,"weight":alloc.weight,"status":if blocked==rows.len(){"blocked"}else if blocked>0{"partial"}else{"exploratory"},"stocks":rows,"rules_version":"pilot-gates-1","exit_policy":"5个交易日持有期为预测标签；8%收盘风险复核、门槛失效或到期需重新分析。没有撮合/持仓，不自动卖出。"}));
    }
    json!({"engine_version":ENGINE,"status":"research_only","as_of":common,"cutoff":cutoff,"sentiment":{"scope":config.sentiment_scope,"pool_advance_ratio":if config.sentiment_scope=="pool"{sentiment}else{None},"symbols":config.symbols,"note":"仅所选股票池上涨占比，非大盘或文本情绪"},"models":models,"cash_weight":(1.-config.allocations.iter().map(|a|a.weight).sum::<f64>()).max(0.),"limitations":["当前修订历史与未复权日线，非严格PIT；日历不完整，5日按有效日线观测步计算，缺失交易日可能拉长实际期限；不提供回测收益或账户目标仓位","五个试验适配器共享近邻收益估计，各自研究门槛为固定工程假设，未证明策略有效","18组参数的历史滚动验证用于选择近邻估计器；不是策略完整样本外回测，未使用最终留出集","每边10bps为预测净优势假设，不含真实订单费用和冲击；标签与费用不等于可成交收益","目标为研究仓位上限，未经过整手/现金/T+1/持仓校验，不能直接下单","所有分析只读行情；无Agent、无真实订单、无模拟订单；分析需手动触发"]})
}
pub async fn run(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    // One CPU job at a time, no waiting queue and no DB mutex during model computation.
    static LIMIT: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
    let permit = LIMIT
        .try_acquire()
        .map_err(|_| AppError::bad_request("已有分析运行，请稍后重试"))?;
    let wid = id.clone();
    let (config,stocks,cutoff)=persistence::run_db(s.clone(),move|c|{
 let raw:Option<String>=c.query_row("SELECT config FROM research_workspace WHERE id=?1",[&wid],|r|r.get(0)).optional()?;
 let config:Config=serde_json::from_str(&raw.ok_or_else(||AppError::not_found("研究配置不存在"))?).map_err(|_|AppError::invalid_data("配置损坏"))?;validate(&config)?;
 let now=Utc::now().with_timezone(&FixedOffset::east_opt(28800).unwrap());let today=now.date_naive();let cutoff=if now.hour()*60+now.minute()>=910{today}else{today.pred_opt().unwrap()}.to_string();
 let mut stocks=vec![];
 for symbol in &config.symbols{
 let name:String=c.query_row("SELECT name FROM instruments WHERE symbol=?1",[symbol],|r|r.get(0))?;
 let mut q=c.prepare("SELECT trade_date,open,high,low,close,volume,source,fetched_at FROM quote_daily WHERE symbol=?1 AND trade_date<=?2 AND lower(source) NOT IN ('demo','seed','synthetic') AND open>0 AND close>0 AND high>=low AND high>=open AND high>=close AND low<=open AND low<=close AND low>0 AND volume>=0 AND NOT EXISTS(SELECT 1 FROM trading_calendar t WHERE t.trade_date=quote_daily.trade_date AND t.is_open=0) ORDER BY trade_date DESC LIMIT 400")?;
 let mut bars=q.query_map(params![symbol,cutoff],|r|Ok(Bar{date:r.get(0)?,open:r.get(1)?,high:r.get(2)?,low:r.get(3)?,close:r.get(4)?,volume:r.get(5)?,source:r.get(6)?,fetched_at:r.get(7)?}))?.collect::<Result<Vec<_>,_>>()?;
 bars.retain(|b|NaiveDate::parse_from_str(&b.date,"%Y-%m-%d").is_ok_and(|d|d.weekday().num_days_from_monday()<5)&&[b.open,b.close,b.high,b.low,b.volume].iter().all(|x|x.is_finite()));bars.reverse();stocks.push(Stock{symbol:symbol.clone(),name,bars});
 }Ok((config,stocks,cutoff))}).await?;
    let input = json!({"config":config,"stocks":stocks,"cutoff":cutoff,"engine_version":ENGINE,"definition":include_str!("../../web/src/data/quantDefinitions.ts"),"implementation_hash":persistence::hash(include_bytes!("trading_research.rs"))});
    let mut identity = input.clone();
    for stock in identity["stocks"].as_array_mut().unwrap() {
        for bar in stock["bars"].as_array_mut().unwrap() {
            bar.as_object_mut().unwrap().remove("fetched_at");
        }
    }
    let hash = persistence::hash(identity.to_string().as_bytes());
    let cached_id = id.clone();
    let cached_hash = hash.clone();
    let cached = persistence::run_db(s.clone(), move |c| {
        Ok(c.query_row(
            "SELECT report FROM research_analysis WHERE workspace_id=?1 AND input_hash=?2",
            params![cached_id, cached_hash],
            |r| r.get::<_, String>(0),
        )
        .optional()?)
    })
    .await?;
    if let Some(raw) = cached {
        drop(permit);
        return Ok(envelope(
            serde_json::from_str::<Value>(&raw).map_err(|_| AppError::internal("报告损坏"))?,
        ));
    }
    let mut report = tokio::task::spawn_blocking(move || analyze(&config, &stocks, &cutoff))
        .await
        .map_err(|e| AppError::internal(e.to_string()))?;
    let report_id = uuid::Uuid::new_v4().to_string();
    report["id"] = json!(report_id);
    report["created_at"] = json!(Utc::now().to_rfc3339());
    report["input_hash"] = json!(hash);
    let out = report.clone();
    persistence::run_db(s, move |c| {
        c.execute(
            "INSERT OR IGNORE INTO research_analysis VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                report_id,
                id,
                hash,
                input.to_string(),
                report.to_string(),
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    })
    .await?;
    drop(permit);
    Ok(envelope(out))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> Config {
        Config {
            name: "test".into(),
            symbols: vec!["002463.SZ".into()],
            allocations: vec![Allocation {
                model_id: IDS[0].into(),
                weight: 0.5,
            }],
            sentiment_scope: "pool".into(),
        }
    }
    #[test]
    fn bounds() {
        let mut c = config();
        assert!(validate(&c).is_ok());
        c.symbols = vec!["x".into(); 9];
        assert!(validate(&c).is_err());
        c = config();
        c.allocations[0].weight = 1.01;
        assert!(validate(&c).is_err());
        c = config();
        c.allocations.push(c.allocations[0].clone());
        assert!(validate(&c).is_err());
        c = config();
        c.allocations[0].weight = f64::NAN;
        assert!(validate(&c).is_err());
    }
    #[test]
    fn prerequisites_block() {
        let c = config();
        let r = analyze(
            &c,
            &[Stock {
                symbol: "002463.SZ".into(),
                name: "test".into(),
                bars: vec![],
            }],
            "2026-10-26",
        );
        assert_eq!(r["models"][0]["stocks"][0]["status"], "blocked");
    }
    #[test]
    fn future_does_not_change_prediction() {
        let mut bars: Vec<_> = (0..245)
            .map(|i| {
                let p = 20. + i as f64 * 0.03 + (i as f64 / 4.).sin() * 0.2;
                Bar {
                    date: format!("{i:04}"),
                    open: p,
                    close: p + 0.1,
                    high: p + 0.5,
                    low: p - 0.5,
                    volume: 1000.,
                    source: "test".into(),
                    fetched_at: "".into(),
                }
            })
            .collect();
        let breadth = bars.iter().map(|b| (b.date.clone(), 0.5)).collect();
        let p = prediction(&bars, 190, &breadth, 120, 16, 0.5).unwrap();
        for b in bars.iter_mut().skip(191) {
            b.open *= 10.;
            b.close *= 10.;
        }
        assert_eq!(p, prediction(&bars, 190, &breadth, 120, 16, 0.5).unwrap());
        assert!(p["analogues"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["label_available"].as_str().unwrap() < bars[190].date.as_str()));
    }
    #[test]
    fn persistence_is_idempotent_and_preserves_references() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        init(&c).unwrap();
        init(&c).unwrap();
        c.execute("INSERT INTO research_workspace VALUES('w','{}','now')", [])
            .unwrap();
        c.execute(
            "INSERT INTO research_analysis VALUES('a','w','hash','frozen','report','now')",
            [],
        )
        .unwrap();
        assert!(c
            .execute(
                "INSERT INTO research_analysis VALUES('b','w','hash','new','other','later')",
                []
            )
            .is_err());
        assert!(c
            .execute("DELETE FROM research_workspace WHERE id='w'", [])
            .is_err());
        assert_eq!(
            c.query_row("SELECT input FROM research_analysis", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "frozen"
        );
    }
    #[test]
    fn unknown_fields_and_future_models_are_not_silently_accepted() {
        let mut raw = serde_json::to_value(config()).unwrap();
        raw["auto_trade"] = json!(true);
        assert!(serde_json::from_value::<Config>(raw).is_err());
        let mut c = config();
        c.allocations[0].model_id = "unknown".into();
        assert!(validate(&c).is_err());
        c = config();
        c.allocations[0].model_id = "ak-event-v1".into();
        let r = analyze(
            &c,
            &[Stock {
                symbol: "002463.SZ".into(),
                name: "test".into(),
                bars: vec![],
            }],
            "2026-10-26",
        );
        assert!(r["models"][0]["stocks"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("PIT"));
    }
}
