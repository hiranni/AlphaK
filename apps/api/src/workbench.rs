//! Read-only, watchlist-scoped daily analysis and persistent observation alerts.
use crate::routes::ApiJson;
use crate::{
    db::{now_iso, AppState},
    error::{envelope, ApiResponse, AppError},
};
use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{Datelike, FixedOffset, Timelike, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone)]
pub struct Signal {
    pub kind: String,
    pub title: String,
    pub evidence: String,
    pub invalidation: String,
    pub risk: String,
}
#[derive(Debug, Serialize, Clone)]
pub struct Analysis {
    pub symbol: String,
    pub name: String,
    pub industry: String,
    pub as_of: String,
    pub source: String,
    pub status: String,
    pub notice: String,
    pub close: Option<f64>,
    pub change: Option<f64>,
    pub change_pct: Option<f64>,
    pub volume: Option<f64>,
    pub amount: Option<f64>,
    pub score: Option<u32>,
    pub trend: String,
    pub volatility: Option<f64>,
    pub volume_ratio: Option<f64>,
    pub support: Option<f64>,
    pub resistance: Option<f64>,
    pub ma20: Option<f64>,
    pub ma60: Option<f64>,
    pub sparkline: Vec<f64>,
    pub signals: Vec<Signal>,
    pub bars_count: usize,
    pub technical_bars_count: usize,
    pub history_status: String,
    pub history_start: String,
}
#[derive(Serialize)]
pub struct Workbench {
    pub generated_at: String,
    pub expected_session: String,
    pub analyses: Vec<Analysis>,
}
#[derive(Clone)]
struct Row {
    date: String,
    close: f64,
    high: f64,
    low: f64,
    volume: f64,
    amount: Option<f64>,
    factor: f64,
    adjusted: bool,
    source: String,
}

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS watch_alert (
      id TEXT PRIMARY KEY, symbol TEXT NOT NULL, kind TEXT NOT NULL, threshold REAL NOT NULL,
      cooldown_minutes INTEGER NOT NULL, enabled INTEGER NOT NULL, last_triggered_at TEXT,
      last_observation TEXT, created_at TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS watch_alert_event (
      id INTEGER PRIMARY KEY AUTOINCREMENT, rule_id TEXT NOT NULL, symbol TEXT NOT NULL,
      message TEXT NOT NULL, as_of TEXT NOT NULL, created_at TEXT NOT NULL);
      CREATE INDEX IF NOT EXISTS idx_watch_alert_event ON watch_alert_event(created_at);",
    )
}
fn connection(state: &AppState) -> Result<std::sync::MutexGuard<'_, Connection>, AppError> {
    state
        .db
        .lock()
        .map_err(|_| AppError::internal("database lock poisoned"))
}
fn expected_session(conn: &Connection) -> Result<String, AppError> {
    let now = Utc::now().with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap());
    let date = if now.hour() < 15 {
        now.date_naive() - chrono::Duration::days(1)
    } else {
        now.date_naive()
    };
    let known: Option<String> = conn.query_row(
        "SELECT MAX(trade_date) FROM trading_calendar WHERE is_open=1 AND trade_date<=?1",
        [date.to_string()],
        |r| r.get(0),
    )?;
    let coverage: Option<String> =
        conn.query_row("SELECT MAX(trade_date) FROM trading_calendar", [], |r| {
            r.get(0)
        })?;
    let known = known.filter(|_| {
        coverage
            .as_deref()
            .is_some_and(|end| end >= date.to_string().as_str())
    });
    Ok(known.unwrap_or_else(|| {
        let mut d = date;
        while d.weekday().number_from_monday() > 5 {
            d -= chrono::Duration::days(1);
        }
        d.to_string()
    }))
}
fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let v: Vec<_> = values.collect();
    v.iter().sum::<f64>() / v.len() as f64
}
fn signal(kind: &str, title: &str, evidence: String, invalidation: &str, risk: &str) -> Signal {
    Signal {
        kind: kind.into(),
        title: title.into(),
        evidence,
        invalidation: invalidation.into(),
        risk: risk.into(),
    }
}
fn analyze(symbol: &str, name: &str, industry: &str, raw: &[Row], expected: &str) -> Analysis {
    let mut a = Analysis {
        symbol: symbol.into(),
        name: name.into(),
        industry: industry.into(),
        as_of: String::new(),
        source: "pending".into(),
        status: "missing".into(),
        notice: "尚无真实日线，请先更新行情".into(),
        close: None,
        change: None,
        change_pct: None,
        volume: None,
        amount: None,
        score: None,
        trend: "数据不足".into(),
        volatility: None,
        volume_ratio: None,
        support: None,
        resistance: None,
        ma20: None,
        ma60: None,
        sparkline: vec![],
        signals: vec![],
        bars_count: 0,
        technical_bars_count: 0,
        history_status: String::new(),
        history_start: String::new(),
    };
    let Some(last) = raw.last() else {
        return a;
    };
    a.as_of = last.date.clone();
    a.source = last.source.clone();
    if last.source == "demo" || last.source.starts_with("mock") || last.source.is_empty() {
        a.status = "demo".into();
        a.notice = "演示数据不参与分析、评分和提醒".into();
        return a;
    }
    if !last.close.is_finite() || last.close <= 0.0 {
        a.notice = "日线价格无效，暂停分析与提醒".into();
        return a;
    }
    a.close = Some(last.close);
    a.volume = Some(last.volume);
    a.amount = last.amount;
    a.status = if last.date.as_str() < expected {
        "stale"
    } else {
        "ready"
    }
    .into();
    a.notice = if a.status == "stale" {
        format!("日线未更新至 {expected}，暂停新提醒")
    } else {
        "已完成交易日的日线分析，非实时行情".into()
    };
    // Do not bridge seeded data or compare different volume units across providers.
    let real: Vec<_> = raw
        .iter()
        .rev()
        .take_while(|r| {
            r.source != "demo"
                && !r.source.starts_with("mock")
                && !r.source.is_empty()
                && r.close.is_finite()
                && r.close > 0.0
                && r.high.is_finite()
                && r.low.is_finite()
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    a.bars_count = real.len();
    a.history_start = real.first().map(|r| r.date.clone()).unwrap_or_default();
    if real.len() >= 2 {
        let prev = real[real.len() - 2];
        // Require known adjustment factors across the boundary to avoid ex-rights price jumps.
        if prev.adjusted
            && last.adjusted
            && prev.factor > 0.0
            && last.factor > 0.0
            && prev.factor.is_finite()
            && last.factor.is_finite()
        {
            let previous = prev.close * prev.factor / last.factor;
            a.change = Some(last.close - previous);
            a.change_pct = Some((last.close / previous - 1.0) * 100.0);
        }
    }
    if real.len() >= 21
        && real[real.len() - 21..]
            .iter()
            .all(|r| r.source == last.source)
    {
        let avg = mean(
            real[real.len() - 21..real.len() - 1]
                .iter()
                .map(|r| r.volume),
        );
        if avg > 0.0 {
            a.volume_ratio = Some(last.volume / avg);
        }
    }
    let adjusted: Vec<_> = real
        .iter()
        .rev()
        .take_while(|r| r.adjusted && r.factor.is_finite() && r.factor > 0.0)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let prices: Vec<_> = adjusted
        .iter()
        .map(|r| r.close * r.factor / last.factor)
        .collect();
    a.sparkline = prices
        .iter()
        .skip(prices.len().saturating_sub(30))
        .copied()
        .collect();
    a.technical_bars_count = prices.len();
    if prices.len() < 21 {
        a.notice
            .push_str("；有效连续复权日线不足 21 条，技术信号与评分暂不可用");
        return a;
    }
    let n = prices.len();
    let prev = &adjusted[n - 21..n - 1];
    let ma20 = mean(prices[n - 20..].iter().copied());
    a.ma20 = Some(ma20);
    let resistance = prev
        .iter()
        .map(|r| r.high * r.factor / last.factor)
        .fold(f64::NEG_INFINITY, f64::max);
    let support = prev
        .iter()
        .map(|r| r.low * r.factor / last.factor)
        .fold(f64::INFINITY, f64::min);
    a.resistance = Some(resistance);
    a.support = Some(support);
    let returns: Vec<_> = prices[n - 21..]
        .windows(2)
        .map(|p| (p[1] / p[0]).ln())
        .collect();
    let avg = mean(returns.iter().copied());
    let volatility = (returns.iter().map(|r| (r - avg).powi(2)).sum::<f64>() / 19.0).sqrt()
        * 252_f64.sqrt()
        * 100.0;
    a.volatility = Some(volatility);
    if n >= 60 {
        let ma60 = mean(prices[n - 60..].iter().copied());
        a.ma60 = Some(ma60);
        a.trend = if last.close > ma20 && ma20 > ma60 {
            "上行"
        } else if last.close < ma20 && ma20 < ma60 {
            "下行"
        } else {
            "震荡"
        }
        .into();
        // Rule conformance score v1: no probability, no backtest claim.
        a.score = a.volume_ratio.map(|ratio| {
            25 * u32::from(last.close > ma20)
                + 25 * u32::from(ma20 > ma60)
                + 25 * u32::from(prices[n - 1] > prices[n - 21])
                + 25 * u32::from(ratio >= 1.5)
        });
    } else {
        a.notice
            .push_str("；完整趋势和评分需 60 条有效复权日线，正在自动补齐");
        a.trend = if last.close > ma20 {
            "位于 MA20 上方"
        } else {
            "位于 MA20 下方"
        }
        .into();
    }
    if last.close > resistance {
        a.signals.push(signal(
            "breakout",
            "突破前 20 日高点",
            format!("收盘 {:.2} > 前 20 日最高价 {:.2}", last.close, resistance),
            "收盘重新跌回触发日的前高以下",
            "可能假突破；未计交易成本，未做历史绩效验证",
        ));
    }
    if last.close < support {
        a.signals.push(signal(
            "support_break",
            "跌破前 20 日低点",
            format!("收盘 {:.2} < 前 20 日最低价 {:.2}", last.close, support),
            "收盘重新站回触发日的前低上方",
            "下行风险观察，不构成卖出指令",
        ));
    }
    let previous_ma = mean(prices[n - 21..n - 1].iter().copied());
    if last.close > ma20 && prices[n - 2] <= previous_ma {
        a.signals.push(signal(
            "reversal",
            "上穿 MA20",
            format!(
                "前收 {:.2} ≤ 前 MA20 {:.2}，本收 {:.2} > MA20 {:.2}",
                prices[n - 2],
                previous_ma,
                last.close,
                ma20
            ),
            "收盘重新跌回 MA20 下方",
            "震荡区间容易反复触发",
        ));
    }
    let momentum = (prices[n - 1] / prices[n - 6] - 1.0) * 100.0;
    if momentum >= 5.0 {
        a.signals.push(signal(
            "momentum",
            "5 日动量增强",
            format!("5 个交易日复权收益 {momentum:.2}% ≥ 5%"),
            "5 日收益低于 5%",
            "短期追涨与回撤风险",
        ));
    }
    if let Some(v) = a.volume_ratio.filter(|v| *v >= 2.0) {
        a.signals.push(signal(
            "volume",
            "成交量放大",
            format!("本日成交量 / 前 20 日均量 = {v:.2} 倍"),
            "成交量比低于 2 倍",
            "放量不代表上涨；仅比较同源成交量",
        ));
    }
    if volatility >= 40.0 {
        a.signals.push(signal(
            "volatility",
            "高波动观察",
            format!("20 日对数收益样本标准差 × √252 = {volatility:.2}%"),
            "20 日年化波动率低于 40%",
            "年化历史波动率不是未来最大损失",
        ));
    }
    a
}
fn analyses(conn: &Connection, expected: &str) -> Result<Vec<Analysis>, AppError> {
    let mut stmt=conn.prepare("SELECT DISTINCT w.symbol,i.name,i.industry FROM watchlist_item w JOIN instruments i ON i.symbol=w.symbol ORDER BY w.symbol")?;
    let symbols = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut out = vec![];
    for (symbol, name, industry) in symbols {
        let mut stmt=conn.prepare("SELECT trade_date,close,high,low,volume,CASE WHEN amount_known=1 THEN amount ELSE NULL END,adj_factor,adj_source,source FROM quote_daily WHERE symbol=?1 AND trade_date<=?2 AND (trade_date<?3 OR julianday(fetched_at)>=julianday(trade_date || 'T15:00:00+08:00')) ORDER BY trade_date DESC LIMIT 160")?;
        let mut rows = stmt
            .query_map(
                params![
                    symbol,
                    expected,
                    Utc::now()
                        .with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap())
                        .date_naive()
                        .to_string()
                ],
                |r| {
                    Ok(Row {
                        date: r.get(0)?,
                        close: r.get(1)?,
                        high: r.get(2)?,
                        low: r.get(3)?,
                        volume: r.get(4)?,
                        amount: r.get(5)?,
                        factor: r.get(6)?,
                        adjusted: {
                            let source = r.get::<_, String>(7)?;
                            !source.is_empty() && source != "demo" && !source.starts_with("mock")
                        },
                        source: r.get(8)?,
                    })
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.reverse();
        out.push(analyze(&symbol, &name, &industry, &rows, expected));
    }
    Ok(out)
}
/// Fill only user watchlist history, once per symbol per five minutes; keep reads nonblocking.
#[cfg(not(test))]
fn refill_history(state: AppState, symbols: Vec<String>) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static RUNNING: AtomicBool = AtomicBool::new(false);
    if symbols.is_empty()
        || RUNNING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
    {
        return;
    }
    tokio::spawn(async move {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                RUNNING.store(false, Ordering::Release);
            }
        }
        let _reset = Reset;
        let end = Utc::now()
            .with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap())
            .date_naive();
        let start = end - chrono::Duration::days(365);
        for symbol in symbols {
            let now = Utc::now().timestamp();
            let eligible = {
                let Ok(conn) = state.db.lock() else { continue };
                let attempted: i64 = conn.query_row("SELECT COALESCE(MAX(attempted_at),0) FROM live_cache WHERE symbol=?1 AND dataset='watch_history'", [&symbol], |r| r.get(0)).unwrap_or(now);
                if now - attempted < 300 {
                    false
                } else {
                    conn.execute("INSERT INTO live_cache(symbol,dataset,attempted_at,error) VALUES(?1,'watch_history',?2,'running') ON CONFLICT(symbol,dataset) DO UPDATE SET attempted_at=excluded.attempted_at,error='running'",params![symbol,now]).is_ok()
                }
            };
            if !eligible {
                continue;
            }
            let result = crate::routes::sync_daily_symbol(
                &state,
                "quote_daily",
                &symbol,
                &start.to_string(),
                &end.to_string(),
                &format!("watch-history-{}", uuid::Uuid::new_v4()),
                true,
            )
            .await;
            let error = match result {
                Ok(_) => String::new(),
                Err(err) => {
                    tracing::warn!(%symbol,error=%err,"watch history refill failed");
                    err
                }
            };
            if let Ok(conn) = state.db.lock() {
                let _ = conn.execute("UPDATE live_cache SET error=?2,fetched_at=CASE WHEN ?2='' THEN ?3 ELSE fetched_at END,failures=CASE WHEN ?2='' THEN 0 ELSE failures+1 END WHERE symbol=?1 AND dataset='watch_history'",params![symbol,error,Utc::now().timestamp()]);
            }
        }
    });
}
pub async fn dashboard(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Workbench>>, AppError> {
    let (expected, mut items) = {
        let conn = connection(&state)?;
        let expected = expected_session(&conn)?;
        let mut items = analyses(&conn, &expected)?;
        for a in &mut items {
            if a.technical_bars_count < 60 || a.volume_ratio.is_none() {
                let (error, fetched): (String, i64) = conn
                    .query_row(
                        "SELECT error,fetched_at FROM live_cache WHERE symbol=?1 AND dataset='watch_history'",
                        [&a.symbol],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .unwrap_or_default();
                a.history_status = if error == "running" {
                    "历史自动补齐中"
                } else if !error.is_empty() {
                    "历史补齐失败，稍后重试"
                } else if fetched > 0 {
                    "上游有效历史仍不足，稍后重试"
                } else {
                    "等待自动补齐历史"
                }
                .into();
            }
        }
        (expected, items)
    };
    #[cfg(not(test))]
    refill_history(
        state,
        items
            .iter()
            .filter(|a| a.technical_bars_count < 60 || a.volume_ratio.is_none())
            .map(|a| a.symbol.clone())
            .collect(),
    );
    Ok(envelope(Workbench {
        generated_at: now_iso(),
        analyses: std::mem::take(&mut items),
        expected_session: expected,
    }))
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AlertInput {
    pub symbol: String,
    pub kind: String,
    pub threshold: f64,
    pub cooldown_minutes: i64,
    pub enabled: bool,
}
#[derive(Debug, Serialize)]
pub struct AlertRule {
    pub id: String,
    #[serde(flatten)]
    pub config: AlertInput,
    pub last_triggered_at: Option<String>,
}
#[derive(Serialize)]
pub struct AlertEvent {
    pub id: i64,
    pub rule_id: String,
    pub symbol: String,
    pub message: String,
    pub as_of: String,
    pub created_at: String,
}
#[derive(Serialize)]
pub struct Alerts {
    pub rules: Vec<AlertRule>,
    pub events: Vec<AlertEvent>,
}
fn rules(conn: &Connection) -> Result<Vec<AlertRule>, AppError> {
    Ok(conn.prepare("SELECT id,symbol,kind,threshold,cooldown_minutes,enabled,last_triggered_at FROM watch_alert ORDER BY created_at DESC,id")?.query_map([],|r|Ok(AlertRule{id:r.get(0)?,config:AlertInput{symbol:r.get(1)?,kind:r.get(2)?,threshold:r.get(3)?,cooldown_minutes:r.get(4)?,enabled:r.get(5)?},last_triggered_at:r.get(6)?}))?.collect::<rusqlite::Result<Vec<_>>>()?)
}
fn alert_state(conn: &Connection) -> Result<Alerts, AppError> {
    let events=conn.prepare("SELECT id,rule_id,symbol,message,as_of,created_at FROM watch_alert_event ORDER BY id DESC LIMIT 100")?.query_map([],|r|Ok(AlertEvent{id:r.get(0)?,rule_id:r.get(1)?,symbol:r.get(2)?,message:r.get(3)?,as_of:r.get(4)?,created_at:r.get(5)?}))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Alerts {
        rules: rules(conn)?,
        events,
    })
}
pub async fn alerts(State(state): State<AppState>) -> Result<Json<ApiResponse<Alerts>>, AppError> {
    let conn = connection(&state)?;
    Ok(envelope(alert_state(&conn)?))
}
fn validate(conn: &Connection, b: &AlertInput) -> Result<(), AppError> {
    let member: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM watchlist_item WHERE symbol=?1)",
        [&b.symbol],
        |r| r.get(0),
    )?;
    if !member {
        return Err(AppError::bad_request("提醒标的必须在自选列表中"));
    }
    if !matches!(
        b.kind.as_str(),
        "price_above"
            | "price_below"
            | "change_above"
            | "change_below"
            | "volume_ratio"
            | "breakout"
            | "support_break"
            | "volatility"
    ) {
        return Err(AppError::bad_request("不支持的提醒条件"));
    }
    if !b.threshold.is_finite()
        || b.threshold.abs() > 1e9
        || (matches!(
            b.kind.as_str(),
            "price_above" | "price_below" | "volume_ratio" | "volatility"
        ) && b.threshold <= 0.0)
        || !(1..=10080).contains(&b.cooldown_minutes)
    {
        return Err(AppError::bad_request("阈值无效，冷却时间须为 1–10080 分钟"));
    }
    Ok(())
}
pub async fn create_alert(
    State(state): State<AppState>,
    ApiJson(b): ApiJson<AlertInput>,
) -> Result<Json<ApiResponse<Alerts>>, AppError> {
    let conn = connection(&state)?;
    validate(&conn, &b)?;
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM watch_alert", [], |r| r.get(0))?;
    if count >= 200 {
        return Err(AppError::bad_request("最多保存 200 条提醒规则"));
    }
    conn.execute("INSERT INTO watch_alert(id,symbol,kind,threshold,cooldown_minutes,enabled,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![uuid::Uuid::new_v4().to_string(),b.symbol,b.kind,b.threshold,b.cooldown_minutes,b.enabled,now_iso()])?;
    Ok(envelope(alert_state(&conn)?))
}
pub async fn update_alert(
    State(state): State<AppState>,
    Path(id): Path<String>,
    ApiJson(b): ApiJson<AlertInput>,
) -> Result<Json<ApiResponse<Alerts>>, AppError> {
    let conn = connection(&state)?;
    validate(&conn, &b)?;
    // Preserve the observation watermark on edits and pause/resume to prevent duplicate notifications.
    let changed=conn.execute("UPDATE watch_alert SET symbol=?1,kind=?2,threshold=?3,cooldown_minutes=?4,enabled=?5 WHERE id=?6",params![b.symbol,b.kind,b.threshold,b.cooldown_minutes,b.enabled,id])?;
    if changed == 0 {
        return Err(AppError::not_found("提醒不存在"));
    }
    Ok(envelope(alert_state(&conn)?))
}
pub async fn delete_alert(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Alerts>>, AppError> {
    let conn = connection(&state)?;
    if conn.execute("DELETE FROM watch_alert WHERE id=?1", [id])? == 0 {
        return Err(AppError::not_found("提醒不存在"));
    }
    Ok(envelope(alert_state(&conn)?))
}
fn matches_rule(b: &AlertInput, a: &Analysis) -> bool {
    if a.status != "ready" {
        return false;
    }
    match b.kind.as_str() {
        "price_above" => a.close.is_some_and(|v| v >= b.threshold),
        "price_below" => a.close.is_some_and(|v| v <= b.threshold),
        "change_above" => a.change_pct.is_some_and(|v| v >= b.threshold),
        "change_below" => a.change_pct.is_some_and(|v| v <= b.threshold),
        "volume_ratio" => a.volume_ratio.is_some_and(|v| v >= b.threshold),
        "volatility" => a.volatility.is_some_and(|v| v >= b.threshold),
        "breakout" | "support_break" => a.signals.iter().any(|s| s.kind == b.kind),
        _ => false,
    }
}
pub fn evaluate_alerts(conn: &Connection, now: chrono::DateTime<Utc>) -> Result<usize, AppError> {
    let enabled = rules(conn)?
        .into_iter()
        .filter(|r| r.config.enabled)
        .collect::<Vec<_>>();
    if enabled.is_empty() {
        return Ok(0);
    }
    let expected = expected_session(conn)?;
    let data = analyses(conn, &expected)?;
    let mut count = 0;
    for r in enabled {
        let Some(a) = data.iter().find(|a| a.symbol == r.config.symbol) else {
            continue;
        };
        if !matches_rule(&r.config, a) {
            continue;
        }
        let observed: Option<String> = conn.query_row(
            "SELECT last_observation FROM watch_alert WHERE id=?1",
            [&r.id],
            |r| r.get(0),
        )?;
        if observed.as_deref() == Some(&a.as_of) {
            continue;
        }
        if r.last_triggered_at
            .as_ref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .is_some_and(|last| {
                (now - last.with_timezone(&Utc)).num_minutes() < r.config.cooldown_minutes
            })
        {
            continue;
        }
        let message = format!(
            "{} {} 条件满足（阈值 {}），日线截至 {}；非实时提醒",
            a.name, r.config.kind, r.config.threshold, a.as_of
        );
        let tx = conn.unchecked_transaction()?;
        tx.execute("INSERT INTO watch_alert_event(rule_id,symbol,message,as_of,created_at) VALUES(?1,?2,?3,?4,?5)",params![r.id,a.symbol,message,a.as_of,now.to_rfc3339()])?;
        tx.execute(
            "UPDATE watch_alert SET last_triggered_at=?1,last_observation=?2 WHERE id=?3",
            params![now.to_rfc3339(), a.as_of, r.id],
        )?;
        tx.commit()?;
        count += 1;
    }
    Ok(count)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn sample(n: usize) -> Vec<Row> {
        (0..n)
            .map(|i| Row {
                date: format!("2026-01-{:02}", i + 1),
                close: 100.0 + i as f64,
                high: 101.0 + i as f64,
                low: 99.0 + i as f64,
                volume: 100.0,
                amount: Some(10000.0),
                factor: 1.0,
                adjusted: true,
                source: "tencent".into(),
            })
            .collect()
    }
    #[test]
    fn signal_excludes_current_high_and_score_is_rule_conformance() {
        let mut rows = sample(65);
        rows[64].close = 180.0;
        rows[64].high = 200.0;
        rows[64].volume = 300.0;
        let a = analyze("x", "test", "test", &rows, "2026-01-65");
        assert_eq!(a.score, Some(100));
        assert_eq!(a.resistance, Some(164.0));
        assert!(a.signals.iter().any(|s| s.kind == "breakout"));
        assert_eq!(a.volume_ratio, Some(3.0));
    }
    #[test]
    fn seeded_missing_and_short_data_cannot_produce_scores() {
        let mut rows = sample(65);
        for row in &mut rows {
            row.source = "demo".into();
        }
        let a = analyze("x", "test", "test", &rows, "2026-01-65");
        assert_eq!(a.status, "demo");
        assert!(a.close.is_none());
        assert!(a.signals.is_empty());
        rows[64].source = "tencent".into();
        let a = analyze("x", "test", "test", &rows, "2026-01-65");
        assert!(a.score.is_none());
        assert_eq!(a.bars_count, 1);
        assert!(analyze("x", "test", "test", &[], "2026-01-65")
            .close
            .is_none());
    }
    #[test]
    fn unknown_adjustment_and_mixed_volume_sources_are_not_compared() {
        let mut rows = sample(65);
        rows[64].adjusted = false;
        let a = analyze("x", "test", "test", &rows, "2026-01-65");
        assert!(a.score.is_none());
        assert!(a.change_pct.is_none());
        assert!(a.sparkline.is_empty());
        rows[64].adjusted = true;
        rows[63].source = "sina".into();
        let a = analyze("x", "test", "test", &rows, "2026-01-65");
        assert!(a.volume_ratio.is_none());
        assert!(a.score.is_none());
    }
    #[test]
    fn reminders_validate_scope_cooldown_and_deduplicate_sessions() {
        let state = crate::db::test_state();
        let conn = state.db.lock().unwrap();
        let symbol: String = conn
            .query_row("SELECT symbol FROM watchlist_item LIMIT 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        let expected = expected_session(&conn).unwrap();
        conn.execute(
            "UPDATE quote_daily SET source='tencent',adj_source='tencent' WHERE symbol=?1",
            [&symbol],
        )
        .unwrap();
        conn.execute("INSERT OR REPLACE INTO quote_daily(symbol,trade_date,open,high,low,close,volume,amount,adj_factor,source,adj_source,fetched_at) VALUES(?1,?2,100,100,100,100,100,100,1,'tencent','tencent',?2 || 'T15:30:00+08:00')",params![symbol,expected]).unwrap();
        let b = AlertInput {
            symbol: symbol.clone(),
            kind: "price_above".into(),
            threshold: 1.0,
            cooldown_minutes: 60,
            enabled: true,
        };
        assert!(validate(&conn, &b).is_ok());
        let mut invalid = b.clone();
        invalid.symbol = "unknown".into();
        assert!(validate(&conn, &invalid).is_err());
        invalid = b.clone();
        invalid.cooldown_minutes = 0;
        assert!(validate(&conn, &invalid).is_err());
        conn.execute(
            "INSERT INTO watch_alert VALUES('rule',?1,'price_above',1,60,1,NULL,NULL,?2)",
            params![symbol, now_iso()],
        )
        .unwrap();
        let now = Utc::now();
        assert_eq!(evaluate_alerts(&conn, now).unwrap(), 1);
        assert_eq!(
            evaluate_alerts(&conn, now + chrono::Duration::hours(2)).unwrap(),
            0
        );
        conn.execute("UPDATE watch_alert SET last_observation=NULL", [])
            .unwrap();
        assert_eq!(
            evaluate_alerts(&conn, now + chrono::Duration::minutes(30)).unwrap(),
            0
        );
        conn.execute("UPDATE watch_alert SET enabled=0", [])
            .unwrap();
        assert_eq!(
            evaluate_alerts(&conn, now + chrono::Duration::hours(2)).unwrap(),
            0
        );
    }
}
