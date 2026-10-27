//! Deterministic research monitors. No orders, positions or return claims.
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
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn init(c: &Connection) -> rusqlite::Result<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS quant_monitor(id TEXT PRIMARY KEY,name TEXT NOT NULL,family TEXT NOT NULL,definition TEXT NOT NULL,enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN(0,1)),created_at TEXT NOT NULL); CREATE TABLE IF NOT EXISTS quant_observation(id TEXT PRIMARY KEY,monitor_id TEXT NOT NULL REFERENCES quant_monitor(id),symbol TEXT NOT NULL,as_of TEXT NOT NULL,input_hash TEXT NOT NULL,payload TEXT NOT NULL,created_at TEXT NOT NULL,UNIQUE(monitor_id,symbol,as_of,input_hash)); CREATE INDEX IF NOT EXISTS quant_observation_recent ON quant_observation(created_at DESC);")
}
fn now() -> chrono::DateTime<FixedOffset> {
    Utc::now().with_timezone(&FixedOffset::east_opt(28800).unwrap())
}
fn catalog() -> Vec<Value> {
    let tech = vec![
        "002463.SZ",
        "600584.SH",
        "002156.SZ",
        "603228.SH",
        "002600.SZ",
        "601208.SH",
    ];
    let auto = vec![
        "603119.SH",
        "002920.SZ",
        "000400.SZ",
        "000528.SZ",
        "000559.SZ",
        "601689.SH",
        "002048.SZ",
        "002553.SZ",
    ];
    let resource = vec![
        "601991.SH",
        "600744.SH",
        "002490.SZ",
        "002167.SZ",
        "600176.SH",
        "000657.SZ",
        "600362.SH",
        "601069.SH",
    ];
    let consumer = vec![
        "603259.SH",
        "600479.SH",
        "600721.SH",
        "600693.SH",
        "002555.SZ",
        "601888.SH",
        "601949.SH",
        "600737.SH",
    ];
    let defensive = vec![
        "601288.SH",
        "600012.SH",
        "603565.SH",
        "601083.SH",
        "601991.SH",
        "600737.SH",
        "600176.SH",
    ];
    vec![
 ("trend20","双均线趋势 20/60","trend",20,60,tech.clone(),"MA20 高于 MA60 且收盘高于 MA20；失效：收盘跌破 MA20"),
 ("trend10","敏捷趋势 10/30","trend",10,30,auto.clone(),"MA10 高于 MA30 且收盘高于 MA10；失效：收盘跌破 MA10"),
 ("break20","20 日放量突破","breakout",20,60,tech.clone(),"收盘突破此前 20 日最高价且成交量超过此前 20 日均量 1.5 倍"),
 ("break60","60 日平台突破","breakout",60,90,resource.clone(),"收盘突破此前 60 日最高价且成交量超过此前 20 日均量 1.5 倍"),
 ("pullback","上升趋势回踩","pullback",20,60,auto.clone(),"MA20 高于 MA60、收盘位于 MA20 上方 0–2%、当日反弹"),
 ("momentum","20 日动量增强","momentum",20,60,tech.clone(),"20 日收益超过 8%、收盘高于 MA20、当日上涨"),
 ("rsi_rebound","RSI 超卖修复","rebound",14,60,consumer.clone(),"RSI14 从不高于 30 回升至高于 30；仅观察，不抄底下单"),
 ("boll_break","布林上轨突破","boll_break",20,60,auto.clone(),"收盘突破此前 20 日均值加 2 倍标准差，量比大于 1.2"),
 ("boll_revert","布林下轨修复","boll_revert",20,60,consumer.clone(),"前日跌至下轨外，今日回到此前 20 日下轨上方且上涨"),
 ("low_vol","低波动趋势观察","low_vol",20,60,defensive.clone(),"20 日收益标准差低于 2%、MA20 高于 MA60、收盘高于 MA20"),
 ("volume","异动放量观察","volume",20,60,resource.clone(),"成交量超过此前 20 日均量 2 倍且当日上涨；不等于资金净流入"),
 ("risk","破位风险预警","risk",20,60,defensive.clone(),"收盘低于 MA20 且单日跌幅超过 3%；风险提示，不等于持仓卖单"),
 ("sentiment","样本情绪共振","sentiment",20,60,tech,"策略池有效标的上涨占比不低于 60%，个股高于 MA20 且上涨；这是池内情绪代理，不代表全市场舆情"),
 ].into_iter().map(|(id,name,family,short,long,symbols,rule)|json!({"id":id,"name":name,"family":family,"version":1,"schema_version":"research-monitor-1","short":short,"long":long,"symbols":symbols,"rule":rule,"risk":"研究候选未经回测验证；不得据此自动下单。历史数据为当前版本重建，非严格 PIT。","mode":"signal_observation"})).collect()
}
pub async fn seed(State(s): State<AppState>) -> Result<Json<ApiResponse<Value>>, AppError> {
    persistence::run_db(s, |c| {
        let mut n = 0;
        for d in catalog() {
            n += c.execute(
                "INSERT OR IGNORE INTO quant_monitor VALUES(?1,?2,?3,?4,1,?5)",
                params![
                    d["id"].as_str(),
                    d["name"].as_str(),
                    d["family"].as_str(),
                    d.to_string(),
                    now().to_rfc3339()
                ],
            )?;
        }
        Ok(envelope(json!({"created":n})))
    })
    .await
}
pub async fn list(State(s): State<AppState>) -> Result<Json<ApiResponse<Value>>, AppError> {
    persistence::run_db(s, |c| {
 let mut q=c.prepare("SELECT definition,enabled FROM quant_monitor ORDER BY created_at,id")?;
 let rows=q.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,bool>(1)?)))?.collect::<Result<Vec<_>,_>>()?;
 let mut data=vec![];
 for (raw,enabled) in rows {let mut v:Value=serde_json::from_str(&raw).map_err(|e|AppError::internal(e.to_string()))?;
 let mut q=c.prepare("SELECT payload FROM quant_observation o WHERE monitor_id=?1 AND rowid=(SELECT rowid FROM quant_observation n WHERE n.monitor_id=o.monitor_id AND n.symbol=o.symbol ORDER BY created_at DESC,rowid DESC LIMIT 1)")?;
 let results=q.query_map([v["id"].as_str().unwrap()],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
 let mut counts=json!({"triggered":0,"condition_met":0,"watching":0,"blocked":0});let mut latest=Value::Null;
 for raw in results{let mut result:Value=serde_json::from_str(&raw).map_err(|e|AppError::internal(e.to_string()))?;let status=result["status"].as_str().unwrap_or("blocked");counts[status]=json!(counts[status].as_u64().unwrap_or(0)+1);result.as_object_mut().unwrap().remove("input");if latest.is_null()||result["as_of"].as_str()>latest["as_of"].as_str(){latest=result;}}
 v["enabled"]=json!(enabled);v["latest"]=latest;v["counts"]=counts;data.push(v);}
 Ok(envelope(json!({"strategies":data,"mode":"signal_observation","schedule":"Asia/Shanghai 工作日 09:30–11:30、13:00–15:15，每分钟检查已完成日线；相同输入去重","calendar":"缺失交易日历时仅按工作日研究观察，不能确认法定交易日；无撮合、无订单","as_of":now().to_rfc3339()})))
 }).await
}
#[derive(Deserialize)]
pub struct Toggle {
    enabled: bool,
}
pub async fn toggle(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<Toggle>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    persistence::run_db(s, move |c| {
        if c.execute(
            "UPDATE quant_monitor SET enabled=?1 WHERE id=?2",
            params![b.enabled, id],
        )? == 0
        {
            return Err(AppError::not_found("策略不存在"));
        }
        Ok(envelope(json!({"enabled":b.enabled})))
    })
    .await
}
#[derive(Clone, serde::Serialize)]
struct Bar {
    date: String,
    close: f64,
    high: f64,
    volume: f64,
    source: String,
    fetched_at: String,
}
fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}
fn std(v: &[f64]) -> f64 {
    let m = mean(v);
    mean(&v.iter().map(|x| (x - m).powi(2)).collect::<Vec<_>>()).sqrt()
}
fn rsi(b: &[Bar]) -> f64 {
    let changes: Vec<f64> = b
        .windows(2)
        .rev()
        .take(14)
        .map(|p| p[1].close - p[0].close)
        .collect();
    let up = changes.iter().map(|x| x.max(0.)).sum::<f64>();
    let down = changes.iter().map(|x| (-x).max(0.)).sum::<f64>();
    if up + down == 0. {
        50.
    } else {
        100. * up / (up + down)
    }
}
fn condition(d: &Value, b: &[Bar], breadth: Option<f64>) -> (bool, Value) {
    let n = b.len();
    let short = d["short"].as_u64().unwrap() as usize;
    let long = d["long"].as_u64().unwrap() as usize;
    let close = b[n - 1].close;
    let prev = b[n - 2].close;
    let prices: Vec<f64> = b.iter().map(|b| b.close).collect();
    let ma = mean(&prices[n - short..]);
    let slow = mean(&prices[n - long..]);
    let history = &b[n - 1 - short..n - 1];
    let high = history.iter().map(|b| b.high).fold(0., f64::max);
    let volume_mean = mean(
        &b[n - 21..n - 1]
            .iter()
            .map(|b| b.volume)
            .collect::<Vec<_>>(),
    );
    let vr = if volume_mean > 0. {
        b[n - 1].volume / volume_mean
    } else {
        0.
    };
    let previous = &prices[n - 21..n - 1];
    let upper = mean(previous) + 2. * std(previous);
    let lower = mean(previous) - 2. * std(previous);
    let ret = close / prev - 1.;
    let momentum = close / b[n - 21].close - 1.;
    let vol = std(&b[n - 21..]
        .windows(2)
        .map(|p| p[1].close / p[0].close - 1.)
        .collect::<Vec<_>>());
    let matched = match d["family"].as_str().unwrap() {
        "trend" => ma > slow && close > ma,
        "breakout" => close > high && vr >= 1.5,
        "pullback" => ma > slow && close >= ma && close <= ma * 1.02 && ret > 0.,
        "momentum" => momentum > 0.08 && close > ma && ret > 0.,
        "rebound" => rsi(b) > 30. && rsi(&b[..n - 1]) <= 30.,
        "boll_break" => close > upper && prev <= upper && vr > 1.2,
        "boll_revert" => prev < lower && close > lower && ret > 0.,
        "low_vol" => vol < 0.02 && ma > slow && close > ma,
        "volume" => vr > 2. && ret > 0.,
        "risk" => close < ma && ret < -0.03,
        "sentiment" => breadth.is_some_and(|x| x >= 0.6) && close > ma && ret > 0.,
        _ => false,
    };
    (
        matched,
        json!({"close":close,"previous_close":prev,"fast_ma":ma,"slow_ma":slow,"prior_high":high,"relative_volume":vr,"return_20d":momentum,"return_1d":ret,"volatility_20d":vol,"rsi14":rsi(b),"pool_advance_ratio":breadth}),
    )
}
fn evaluate(c: &Connection) -> Result<Value, AppError> {
    evaluate_at(c, now())
}
fn evaluate_at(c: &Connection, time: chrono::DateTime<FixedOffset>) -> Result<Value, AppError> {
    let today = time.date_naive();
    let cutoff = if time.hour() * 60 + time.minute() >= 910 {
        today
    } else {
        today.pred_opt().unwrap()
    };
    let mut q = c.prepare("SELECT definition FROM quant_monitor WHERE enabled=1")?;
    let defs = q
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(q);
    let mut inserted = 0;
    let mut blocked = 0;
    for raw in defs {
        let d: Value = serde_json::from_str(&raw).map_err(|e| AppError::internal(e.to_string()))?;
        let symbols = d["symbols"]
            .as_array()
            .ok_or_else(|| AppError::invalid_data("股票池缺失"))?;
        if symbols.is_empty() || symbols.len() > 8 {
            return Err(AppError::invalid_data("策略股票池必须为 1–8 只"));
        }
        let mut pool = vec![];
        for symbol in symbols {
            let symbol = symbol.as_str().unwrap();
            let mut stmt=c.prepare("SELECT trade_date,close,high,volume,source,fetched_at FROM quote_daily WHERE symbol=?1 AND trade_date<=?2 AND source NOT IN ('demo','seed','synthetic') AND NOT EXISTS(SELECT 1 FROM trading_calendar t WHERE t.trade_date=quote_daily.trade_date AND t.is_open=0) ORDER BY trade_date DESC LIMIT 180")?;
            let mut bars = stmt
                .query_map(params![symbol, cutoff.to_string()], |r| {
                    Ok(Bar {
                        date: r.get(0)?,
                        close: r.get(1)?,
                        high: r.get(2)?,
                        volume: r.get(3)?,
                        source: r.get(4)?,
                        fetched_at: r.get(5)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            bars.retain(|b| {
                NaiveDate::parse_from_str(&b.date, "%Y-%m-%d")
                    .is_ok_and(|date| date.weekday().number_from_monday() <= 5)
                    && b.close.is_finite()
                    && b.close > 0.
                    && b.high.is_finite()
                    && b.high >= b.close
                    && b.volume.is_finite()
                    && b.volume >= 0.
            });
            bars.reverse();
            pool.push((symbol.to_string(), bars));
        }
        let expected = pool
            .iter()
            .filter_map(|(_, b)| b.last().map(|b| b.date.clone()))
            .max();
        let valid: Vec<_> = pool
            .iter()
            .filter(|(_, b)| {
                b.len() >= d["long"].as_u64().unwrap() as usize + 2
                    && b.last().map(|b| &b.date) == expected.as_ref()
            })
            .collect();
        let ratio = if valid.len() == pool.len() {
            Some(
                valid
                    .iter()
                    .filter(|(_, b)| b[b.len() - 1].close > b[b.len() - 2].close)
                    .count() as f64
                    / valid.len() as f64,
            )
        } else {
            None
        };
        for (symbol, bars) in &pool {
            let last = bars.last();
            let stale = last
                .and_then(|b| NaiveDate::parse_from_str(&b.date, "%Y-%m-%d").ok())
                .is_none_or(|d| (cutoff - d).num_days() > 5);
            let enough = bars.len() >= d["long"].as_u64().unwrap() as usize + 2;
            let ready = enough
                && !stale
                && last.map(|b| &b.date) == expected.as_ref()
                && (d["family"] != "sentiment" || ratio.is_some());
            let (matched, metrics) = if ready {
                condition(&d, bars, ratio)
            } else {
                blocked += 1;
                (false, Value::Null)
            };
            let prior = if ready {
                Some(condition(&d, &bars[..bars.len() - 1], None).0)
            } else {
                None
            };
            let status = if !ready {
                "blocked"
            } else if matched && prior == Some(false) && d["family"] != "sentiment" {
                "triggered"
            } else if matched {
                "condition_met"
            } else {
                "watching"
            };
            let input = json!({"definition":d,"symbol":symbol,"bars":bars,"pool":if d["family"]=="sentiment" {json!(pool)}else{Value::Null},"cutoff":cutoff.to_string(),"engine_version":"monitor-1"});
            let hash = persistence::hash(input.to_string().as_bytes());
            let as_of = last.map(|b| b.date.as_str()).unwrap_or("unknown");
            let payload = json!({"strategy_id":d["id"],"name":d["name"],"symbol":symbol,"as_of":as_of,"status":status,"metrics":metrics,"bars":bars.len(),"reason":if ready{d["rule"].as_str().unwrap()}else{"历史不足、过期、池内日期不齐或情绪覆盖不足"},"risk":d["risk"],"input":input,"input_hash":hash,"observed_at":time.to_rfc3339(),"calendar":"工作日过滤 + 已知休市日；未知日历不作交易确认"});
            // Re-fetch timestamps are provenance, not a new market observation.
            let mut identity = input.clone();
            identity.as_object_mut().unwrap().remove("cutoff");
            fn strip_fetch(v: &mut Value) {
                match v {
                    Value::Object(o) => {
                        o.remove("fetched_at");
                        for value in o.values_mut() {
                            strip_fetch(value)
                        }
                    }
                    Value::Array(a) => {
                        for value in a {
                            strip_fetch(value)
                        }
                    }
                    _ => {}
                }
            }
            strip_fetch(&mut identity);
            let id = persistence::hash(
                json!({"input":identity,"status":status})
                    .to_string()
                    .as_bytes(),
            );
            inserted += c.execute(
                "INSERT OR IGNORE INTO quant_observation VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    id,
                    d["id"].as_str(),
                    symbol,
                    as_of,
                    hash,
                    payload.to_string(),
                    time.to_rfc3339()
                ],
            )?;
        }
    }
    Ok(
        json!({"inserted":inserted,"blocked":blocked,"observed_at":time.to_rfc3339(),"cutoff":cutoff.to_string()}),
    )
}
pub async fn run(State(s): State<AppState>) -> Result<Json<ApiResponse<Value>>, AppError> {
    persistence::run_db(s, |c| Ok(envelope(evaluate(c)?))).await
}
pub async fn observations(State(s): State<AppState>) -> Result<Json<ApiResponse<Value>>, AppError> {
    persistence::run_db(s, |c| {
        let mut q = c.prepare(
            "SELECT payload FROM quant_observation ORDER BY created_at DESC,id LIMIT 200",
        )?;
        let rows = q
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(envelope(json!(rows
            .into_iter()
            .filter_map(|x| serde_json::from_str::<Value>(&x).ok())
            .map(|mut v| {
                v.as_object_mut().unwrap().remove("input");
                v
            })
            .collect::<Vec<_>>())))
    })
    .await
}
pub fn spawn(s: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            let t = now();
            let m = t.hour() * 60 + t.minute();
            if t.weekday().number_from_monday() <= 5
                && ((570..=690).contains(&m) || (780..=915).contains(&m))
            {
                if let Err(e) = persistence::run_db(s.clone(), |c| {
                    let closed:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM trading_calendar WHERE trade_date=?1 AND is_open=0)",[now().date_naive().to_string()],|r|r.get(0))?;
                    if closed {Ok(json!({"skipped":"calendar_closed"}))} else {evaluate(c)}
                }).await {
                    tracing::warn!("quant monitor: {e:?}");
                }
            }
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_scoped_unique() {
        let d = catalog();
        assert_eq!(d.len(), 13);
        let mut ids = std::collections::HashSet::new();
        for v in d {
            assert!(ids.insert(v["id"].to_string()));
            let s = v["symbols"].as_array().unwrap();
            assert!(!s.is_empty() && s.len() <= 8);
            assert_eq!(
                s.iter().collect::<std::collections::HashSet<_>>().len(),
                s.len()
            );
        }
    }
    #[test]
    fn trend_and_risk() {
        let b: Vec<Bar> = (0..100)
            .map(|i| Bar {
                date: "2026-10-23".into(),
                close: 100. + i as f64,
                high: 101. + i as f64,
                volume: 100.,
                source: "test".into(),
                fetched_at: "2026-10-25".into(),
            })
            .collect();
        let d = catalog();
        assert!(condition(&d[0], &b, None).0);
        assert!(!condition(&d[11], &b, None).0);
        assert!(!condition(&d[12], &b, None).0);
    }
    #[test]
    fn observation_replay_pause_and_provenance_dedup() {
        let c = Connection::open_in_memory().unwrap();
        init(&c).unwrap();
        c.execute_batch("CREATE TABLE trading_calendar(trade_date TEXT,is_open INTEGER); CREATE TABLE quote_daily(symbol TEXT,trade_date TEXT,close REAL,high REAL,volume REAL,source TEXT,fetched_at TEXT);").unwrap();
        let mut d = catalog()[0].clone();
        d["symbols"] = json!(["002463.SZ"]);
        c.execute(
            "INSERT INTO quant_monitor VALUES('trend20','test','trend',?1,1,'now')",
            [d.to_string()],
        )
        .unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 10, 26).unwrap();
        for i in 0..160 {
            let day = date - chrono::Duration::days(i);
            c.execute(
                "INSERT INTO quote_daily VALUES('002463.SZ',?1,?2,?3,100,'test','first')",
                params![day.to_string(), 200. - i as f64, 201. - i as f64],
            )
            .unwrap();
        }
        let morning = chrono::DateTime::parse_from_rfc3339("2026-10-26T10:00:00+08:00").unwrap();
        let result = evaluate_at(&c, morning).unwrap();
        assert_eq!(result["inserted"], 1);
        assert_eq!(result["blocked"], 0);
        let raw: String = c
            .query_row("SELECT payload FROM quant_observation", [], |r| r.get(0))
            .unwrap();
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["as_of"], "2026-10-23");
        assert!(v["input"]["bars"]
            .as_array()
            .unwrap()
            .iter()
            .all(|b| b["date"] != "2026-10-25" && b["date"] != "2026-10-26"));
        assert_eq!(evaluate_at(&c, morning).unwrap()["inserted"], 0);
        c.execute("UPDATE quote_daily SET fetched_at='second'", [])
            .unwrap();
        assert_eq!(evaluate_at(&c, morning).unwrap()["inserted"], 0);
        c.execute("UPDATE quant_monitor SET enabled=0", []).unwrap();
        assert_eq!(evaluate_at(&c, morning).unwrap()["inserted"], 0);
        c.execute("UPDATE quant_monitor SET enabled=1", []).unwrap();
        c.execute("UPDATE quote_daily SET source='demo'", [])
            .unwrap();
        let blocked = evaluate_at(&c, morning).unwrap();
        assert_eq!(blocked["blocked"], 1);
    }
}
