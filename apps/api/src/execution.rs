//! Account-scoped execution ledger. Only this service mutates execution tables.
//! Research output is NOT an Agent decision; no public raw-order submission API.
use crate::{
    db::AppState,
    error::{envelope, ApiResponse, AppError},
    persistence,
};
use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, FixedOffset, Timelike, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};

const RULES: &str = "a-share-execution-1";
pub fn init(c: &Connection) -> rusqlite::Result<()> {
    c.execute_batch(r#"
CREATE TABLE IF NOT EXISTS execution_run(id TEXT PRIMARY KEY,account_id TEXT NOT NULL REFERENCES paper_strategy(id),request_key TEXT NOT NULL,workspace_id TEXT NOT NULL REFERENCES research_workspace(id),analysis_id TEXT REFERENCES research_analysis(id),rules_version TEXT NOT NULL,context TEXT NOT NULL,created_at TEXT NOT NULL,UNIQUE(account_id,request_key));
CREATE INDEX IF NOT EXISTS execution_run_recent ON execution_run(account_id,created_at DESC);
CREATE TABLE IF NOT EXISTS execution_order(id TEXT PRIMARY KEY,run_id TEXT NOT NULL REFERENCES execution_run(id),account_id TEXT NOT NULL REFERENCES paper_strategy(id),symbol TEXT NOT NULL,side TEXT NOT NULL CHECK(side IN ('buy','sell','hold')),quantity INTEGER NOT NULL CHECK(quantity>=0),price_fen INTEGER,status TEXT NOT NULL CHECK(status IN ('filled','rejected','hold')),reason TEXT NOT NULL,created_at TEXT NOT NULL,UNIQUE(run_id,symbol));
CREATE INDEX IF NOT EXISTS execution_order_recent ON execution_order(account_id,created_at DESC);
CREATE TABLE IF NOT EXISTS execution_fill(id TEXT PRIMARY KEY,order_id TEXT NOT NULL UNIQUE REFERENCES execution_order(id),account_id TEXT NOT NULL REFERENCES paper_strategy(id),symbol TEXT NOT NULL,side TEXT NOT NULL CHECK(side IN ('buy','sell')),quantity INTEGER NOT NULL CHECK(quantity>0),price_fen INTEGER NOT NULL CHECK(price_fen>0),fee_fen INTEGER NOT NULL CHECK(fee_fen>=0),trade_date TEXT NOT NULL,created_at TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS execution_fill_account ON execution_fill(account_id,trade_date);
CREATE TABLE IF NOT EXISTS execution_cash(id TEXT PRIMARY KEY,account_id TEXT NOT NULL REFERENCES paper_strategy(id),fill_id TEXT NOT NULL UNIQUE REFERENCES execution_fill(id),amount_fen INTEGER NOT NULL CHECK(amount_fen<>0),created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS execution_lot(id TEXT PRIMARY KEY,account_id TEXT NOT NULL REFERENCES paper_strategy(id),symbol TEXT NOT NULL,fill_id TEXT NOT NULL UNIQUE REFERENCES execution_fill(id),trade_date TEXT NOT NULL,quantity INTEGER NOT NULL CHECK(quantity>0),remaining INTEGER NOT NULL CHECK(remaining>=0 AND remaining<=quantity),cost_fen INTEGER NOT NULL CHECK(cost_fen>0));
CREATE INDEX IF NOT EXISTS execution_lot_available ON execution_lot(account_id,symbol,trade_date);
CREATE TABLE IF NOT EXISTS execution_nav(id TEXT PRIMARY KEY,run_id TEXT NOT NULL UNIQUE REFERENCES execution_run(id),account_id TEXT NOT NULL REFERENCES paper_strategy(id),cash_fen INTEGER NOT NULL CHECK(cash_fen>=0),market_fen INTEGER CHECK(market_fen>=0),equity_fen INTEGER CHECK(equity_fen>=0),created_at TEXT NOT NULL);
"#)
}
fn uid() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub fn cash(c: &Connection, id: &str) -> Result<i64, AppError> {
    Ok(c.query_row("SELECT COALESCE((SELECT SUM(amount_fen) FROM paper_cash_ledger WHERE strategy_id=?1),0)+COALESCE((SELECT SUM(amount_fen) FROM execution_cash WHERE account_id=?1),0)",[id],|r|r.get(0))?)
}

// This typed boundary accepts only trusted server-side adapter output. Missing
// provenance is an explicit block; ordinary quote freshness is not proof of it.
#[derive(Clone, Debug)]
struct Quote {
    price: i64,
    at: DateTime<Utc>,
    upper: Option<i64>,
    lower: Option<i64>,
    tradable: Option<bool>,
    corporate_actions_clear: bool,
    available: Option<i64>,
}
fn quote_gate(q: &Quote, now: DateTime<Utc>) -> Result<(), String> {
    if q.price <= 0 {
        return Err("报价无效".into());
    }
    if (now - q.at).num_seconds() < 0 || (now - q.at).num_seconds() > 10 {
        return Err("报价超过10秒或时间异常".into());
    }
    if q.tradable != Some(true) {
        return Err("缺少已验证的当日交易状态，或股票停牌".into());
    }
    if !q.corporate_actions_clear {
        return Err("缺少当日除权及股份变动核验".into());
    }
    match (q.lower, q.upper) {
        (Some(l), Some(u)) if l > 0 && u >= l && q.price >= l && q.price <= u => (),
        _ => return Err("缺少有效当日涨跌停价；禁止按固定10%推算".into()),
    }
    if q.available.unwrap_or(0) <= 0 {
        return Err("缺少可成交盘口数量".into());
    }
    Ok(())
}
fn session(now: DateTime<Utc>) -> bool {
    let t = now.with_timezone(&FixedOffset::east_opt(28800).unwrap());
    let s = t.num_seconds_from_midnight();
    (34200..41400).contains(&s) || (46800..53820).contains(&s)
}
// Integer ceiling division: never underestimate costs by truncation.
fn cost(notional: i64, sell: bool) -> i64 {
    let ceil = |n: i64, d: i64| (n + d - 1) / d;
    ceil(notional * 3, 10000).max(500)
        + ceil(notional, 100000)
        + if sell { ceil(notional * 5, 10000) } else { 0 }
}
fn quantity(c: &Connection, id: &str, sym: &str, date: Option<&str>) -> Result<i64, AppError> {
    Ok(c.query_row("SELECT COALESCE(SUM(remaining),0) FROM execution_lot WHERE account_id=?1 AND symbol=?2 AND (?3 IS NULL OR trade_date<?3)",params![id,sym,date],|r|r.get(0))?)
}
fn positions(c: &Connection, id: &str) -> Result<Vec<Value>, AppError> {
    let mut s=c.prepare("SELECT l.symbol,COALESCE(i.name,l.symbol),SUM(l.remaining),SUM(l.cost_fen*l.remaining/l.quantity) FROM execution_lot l LEFT JOIN instruments i ON i.symbol=l.symbol WHERE l.account_id=?1 AND l.remaining>0 GROUP BY l.symbol ORDER BY l.symbol")?;
    let rows = s
        .query_map([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let date = Utc::now()
        .with_timezone(&FixedOffset::east_opt(28800).unwrap())
        .format("%Y-%m-%d")
        .to_string();
    rows.into_iter().map(|(symbol,name,qty,cost)|Ok(json!({"symbol":symbol,"name":name,"quantity":qty,"available":quantity(c,id,&symbol,Some(&date))?,"cost_fen":cost}))).collect()
}
// One all-or-nothing fill, including cash and FIFO lots, inside caller transaction.
fn fill(
    c: &Connection,
    order: &str,
    id: &str,
    sym: &str,
    sell: bool,
    qty: i64,
    q: &Quote,
    now: DateTime<Utc>,
    equity: i64,
    gross: i64,
) -> Result<(i64, i64), AppError> {
    quote_gate(q, now).map_err(AppError::bad_request)?;
    let date = now
        .with_timezone(&FixedOffset::east_opt(28800).unwrap())
        .format("%Y-%m-%d")
        .to_string();
    if qty <= 0 || (!sell && qty % 100 != 0) || qty > q.available.unwrap_or(0) {
        return Err(AppError::bad_request("数量不符合整手或盘口限制"));
    }
    // Five bps adverse slippage, one-fen ticks. Never clamp an illegal price.
    let slip = (q.price * 5 + 9999) / 10000;
    let price = if sell { q.price - slip } else { q.price + slip };
    if price < q.lower.unwrap() || price > q.upper.unwrap() {
        return Err(AppError::bad_request("滑点后价格触及涨跌停限制"));
    }
    let notional = price
        .checked_mul(qty)
        .ok_or_else(|| AppError::bad_request("金额溢出"))?;
    let fee = cost(notional, sell);
    let balance = cash(c, id)?;
    if sell {
        let total = quantity(c, id, sym, None)?;
        let avail = quantity(c, id, sym, Some(&date))?;
        if qty > avail || (qty % 100 != 0 && qty != total) {
            return Err(AppError::bad_request("T+1可用不足或零股未一次卖出"));
        }
    } else {
        if balance < notional + fee {
            return Err(AppError::bad_request("可用资金不足（含费用）"));
        }
        if (quantity(c, id, sym, None)? + qty) * price > equity / 5
            || gross + notional > equity * 4 / 5
        {
            return Err(AppError::bad_request("超过单股20%或总仓80%限制"));
        }
    }
    let fid = uid();
    let at = now.to_rfc3339();
    c.execute(
        "INSERT INTO execution_fill VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![
            fid,
            order,
            id,
            sym,
            if sell { "sell" } else { "buy" },
            qty,
            price,
            fee,
            date,
            at
        ],
    )?;
    c.execute(
        "INSERT INTO execution_cash VALUES(?1,?2,?3,?4,?5)",
        params![
            uid(),
            id,
            fid,
            if sell {
                notional - fee
            } else {
                -notional - fee
            },
            at
        ],
    )?;
    if !sell {
        c.execute(
            "INSERT INTO execution_lot VALUES(?1,?2,?3,?4,?5,?6,?6,?7)",
            params![uid(), id, sym, fid, date, qty, notional + fee],
        )?;
    } else {
        let mut s=c.prepare("SELECT id,remaining FROM execution_lot WHERE account_id=?1 AND symbol=?2 AND trade_date<?3 AND remaining>0 ORDER BY trade_date,rowid")?;
        let lots = s
            .query_map(params![id, sym, date], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut left = qty;
        for (lot, n) in lots {
            let used = left.min(n);
            c.execute(
                "UPDATE execution_lot SET remaining=remaining-?1 WHERE id=?2",
                params![used, lot],
            )?;
            left -= used;
            if left == 0 {
                break;
            }
        }
    }
    c.execute(
        "UPDATE execution_order SET status='filled',price_fen=?1 WHERE id=?2",
        params![price, order],
    )?;
    Ok((notional, fee))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunRequest {
    request_key: String,
}
fn run(c: &Connection, id: &str, key: &str, now: DateTime<Utc>) -> Result<Value, AppError> {
    if key.trim().is_empty() || key.len() > 100 {
        return Err(AppError::bad_request("无效执行标识"));
    }
    let tx = c.unchecked_transaction()?;
    let account:Option<(String,String)>=tx.query_row("SELECT p.workspace_id,w.config FROM paper_strategy p JOIN research_workspace w ON w.id=p.workspace_id WHERE p.id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    let (workspace, raw) = account.ok_or_else(|| AppError::not_found("账户不存在"))?;
    if let Some(old) = tx
        .query_row(
            "SELECT id FROM execution_run WHERE account_id=?1 AND request_key=?2",
            params![id, key],
            |r| r.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(json!({"id":old,"reused":true}));
    }
    let archived:bool=tx.query_row("SELECT COALESCE((SELECT kind FROM paper_strategy_event WHERE strategy_id=?1 AND kind IN ('archive','restore') ORDER BY rowid DESC LIMIT 1),'restore')='archive'",[id],|r|r.get(0))?;
    if archived {
        return Err(AppError::bad_request("已归档账户禁止执行"));
    }
    let config: crate::trading_research::Config =
        serde_json::from_str(&raw).map_err(|_| AppError::internal("配置损坏"))?;
    crate::trading_research::validate(&config)?;
    let analysis:Option<(String,String)>=tx.query_row("SELECT id,report FROM research_analysis WHERE workspace_id=?1 ORDER BY created_at DESC,rowid DESC LIMIT 1",[&workspace],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    let report = analysis
        .as_ref()
        .and_then(|(_, r)| serde_json::from_str::<Value>(r).ok());
    let date = now
        .with_timezone(&FixedOffset::east_opt(28800).unwrap())
        .format("%Y-%m-%d")
        .to_string();
    let is_open: Option<i64> = tx
        .query_row(
            "SELECT is_open FROM trading_calendar WHERE trade_date=?1",
            [&date],
            |r| r.get(0),
        )
        .optional()?;
    let rid = uid();
    let at = now.to_rfc3339();
    let mut observations = vec![];
    for sym in &config.symbols {
        let quote: Option<(f64, String, Option<String>, String)> = tx
            .query_row(
                "SELECT close,source,provider_time,fetched_at FROM quote_snapshot WHERE symbol=?1",
                [sym],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        observations.push(json!({"symbol":sym,"quote":quote}));
    }
    let context = json!({"config":config,"quotes":observations,"calendar":{"date":date,"is_open":is_open,"provenance_verified":false},"fees":{"version":"cn-mainboard-cost-1","commission_bps":3,"minimum_fen":500,"sell_stamp_bps":5,"transfer_bps":0.1,"slippage_bps":5},"execution_adapter":"readiness-only","missing":["exchange_calendar_provenance","instrument_trading_status","daily_price_limits","corporate_actions","executable_book"]});
    tx.execute(
        "INSERT INTO execution_run VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            rid,
            id,
            key,
            workspace,
            analysis.as_ref().map(|a| &a.0),
            RULES,
            context.to_string(),
            at
        ],
    )?;
    for sym in &config.symbols {
        let mut target = 0.;
        let mut buying = false;
        let mut reasons = vec![];
        if let Some(r) = &report {
            if let Some(models) = r["models"].as_array() {
                for m in models {
                    if let Some(stocks) = m["stocks"].as_array() {
                        for stock in stocks
                            .iter()
                            .filter(|s| s["symbol"].as_str() == Some(sym.as_str()))
                        {
                            if stock["action"] == "buy_candidate" {
                                if let Some(w) = stock["account_ceiling"].as_f64() {
                                    if w.is_finite() && w > 0. {
                                        target += w;
                                        buying = true;
                                    }
                                }
                            }
                            if stock["status"] == "blocked" {
                                reasons.push(
                                    stock["reason"]
                                        .as_str()
                                        .unwrap_or("模型数据不足")
                                        .to_string(),
                                );
                            }
                        }
                    }
                }
            }
        }
        let mut gates = vec![];
        if !session(now) {
            gates.push("当前不在连续竞价时段".to_string())
        }
        if is_open != Some(1) {
            gates.push("当日交易日历缺失或休市".into())
        }
        if let Some(o) = observations
            .iter()
            .find(|o| o["symbol"].as_str() == Some(sym.as_str()))
        {
            match o["quote"][2]
                .as_str()
                .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
            {
                Some(t)
                    if (now - t.with_timezone(&Utc)).num_seconds() >= 0
                        && (now - t.with_timezone(&Utc)).num_seconds() <= 10 =>
                {
                    ()
                }
                Some(_) => gates.push("源报价超过10秒或源时间异常".into()),
                None => gates.push("缺少行情源时间，不能用抓取时间冒充实时报价时间".into()),
            }
        }
        gates.push("交易日历缺少官方来源凭证".into());
        gates.push("行情适配器尚未提供已核验的停牌状态、涨跌停价、除权状态及可成交盘口".into());
        if report.is_none() {
            gates.push("尚无当前配置的历史分析报告".into())
        }
        // Existing research is NOT an approved execution policy: no guessed sell or
        // stop-loss from reduce_review/cooling=0. Persist the blocker instead.
        gates.push("研究候选尚未通过独立样本外验证，且未绑定已审核的入场、退出执行版本".into());
        let reason = format!(
            "{}{}",
            gates.join("；"),
            if reasons.is_empty() {
                String::new()
            } else {
                format!("；模型限制：{}", reasons.join("；"))
            }
        );
        tx.execute(
            "INSERT INTO execution_order VALUES(?1,?2,?3,?4,?5,0,NULL,'rejected',?6,?7)",
            params![
                uid(),
                rid,
                id,
                sym,
                if buying { "buy" } else { "hold" },
                format!(
                    "目标上限 {:.2}%（未委托）；{reason}",
                    target.min(0.2) * 100.
                ),
                at
            ],
        )?;
    }
    let balance = cash(&tx, id)?;
    let has_positions: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM execution_lot WHERE account_id=?1 AND remaining>0)",
        [id],
        |r| r.get(0),
    )?;
    tx.execute(
        "INSERT INTO execution_nav VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![
            uid(),
            rid,
            id,
            balance,
            if has_positions { None } else { Some(0_i64) },
            if has_positions { None } else { Some(balance) },
            at
        ],
    )?;
    tx.commit()?;
    Ok(
        json!({"id":rid,"reused":false,"status":"blocked","filled":0,"checked":config.symbols.len()}),
    )
}
pub async fn execute(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(x): Json<RunRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    Ok(envelope(
        persistence::run_db(s, move |c| run(c, &id, &x.request_key, Utc::now())).await?,
    ))
}
fn records(c: &Connection, id: &str) -> Result<Value, AppError> {
    let exists: bool = c.query_row(
        "SELECT EXISTS(SELECT 1 FROM paper_strategy WHERE id=?1)",
        [id],
        |r| r.get(0),
    )?;
    if !exists {
        return Err(AppError::not_found("账户不存在"));
    }
    let mut q=c.prepare("SELECT o.id,o.run_id,o.symbol,COALESCE(i.name,o.symbol),o.side,o.quantity,o.price_fen,o.status,o.reason,o.created_at,f.fee_fen FROM execution_order o LEFT JOIN instruments i ON i.symbol=o.symbol LEFT JOIN execution_fill f ON f.order_id=o.id WHERE o.account_id=?1 ORDER BY o.rowid DESC LIMIT 200")?;
    let orders=q.query_map([id],|r|Ok(json!({"id":r.get::<_,String>(0)?,"run_id":r.get::<_,String>(1)?,"symbol":r.get::<_,String>(2)?,"name":r.get::<_,String>(3)?,"side":r.get::<_,String>(4)?,"quantity":r.get::<_,i64>(5)?,"price_fen":r.get::<_,Option<i64>>(6)?,"status":r.get::<_,String>(7)?,"reason":r.get::<_,String>(8)?,"created_at":r.get::<_,String>(9)?,"fee_fen":r.get::<_,Option<i64>>(10)?})))?.collect::<Result<Vec<_>,_>>()?;
    let mut q=c.prepare("SELECT id,analysis_id,rules_version,created_at FROM execution_run WHERE account_id=?1 ORDER BY rowid DESC LIMIT 50")?;
    let runs=q.query_map([id],|r|Ok(json!({"id":r.get::<_,String>(0)?,"analysis_id":r.get::<_,Option<String>>(1)?,"rules_version":r.get::<_,String>(2)?,"created_at":r.get::<_,String>(3)?})))?.collect::<Result<Vec<_>,_>>()?;
    let mut q=c.prepare("SELECT created_at,cash_fen,market_fen,equity_fen FROM execution_nav WHERE account_id=?1 ORDER BY rowid DESC LIMIT 500")?;
    let nav=q.query_map([id],|r|Ok(json!({"created_at":r.get::<_,String>(0)?,"cash_fen":r.get::<_,i64>(1)?,"market_fen":r.get::<_,Option<i64>>(2)?,"equity_fen":r.get::<_,Option<i64>>(3)?})))?.collect::<Result<Vec<_>,_>>()?;
    let mut q=c.prepare("SELECT x.amount_fen,x.created_at,f.symbol,f.side FROM execution_cash x JOIN execution_fill f ON f.id=x.fill_id WHERE x.account_id=?1 ORDER BY x.rowid DESC LIMIT 200")?;
    let mut ledger=q.query_map([id],|r|Ok(json!({"amount_fen":r.get::<_,i64>(0)?,"created_at":r.get::<_,String>(1)?,"symbol":r.get::<_,String>(2)?,"side":r.get::<_,String>(3)?})))?.collect::<Result<Vec<_>,_>>()?;
    let funding: (i64, String) = c.query_row(
        "SELECT amount_fen,created_at FROM paper_cash_ledger WHERE strategy_id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    ledger.push(
        json!({"amount_fen":funding.0,"created_at":funding.1,"symbol":"","side":"initial_funding"}),
    );
    Ok(
        json!({"orders":orders,"runs":runs,"positions":positions(c,id)?,"nav":nav,"cash_ledger":ledger,"cash_fen":cash(c,id)?,"execution_ready":false,"rules_version":RULES}),
    )
}
pub async fn list(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    Ok(envelope(
        persistence::run_db(s, move |c| records(c, &id)).await?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("PRAGMA foreign_keys=ON;CREATE TABLE instruments(symbol TEXT,name TEXT);CREATE TABLE trading_calendar(trade_date TEXT PRIMARY KEY,is_open INTEGER);CREATE TABLE quote_snapshot(symbol TEXT,close REAL,source TEXT,provider_time TEXT,fetched_at TEXT);").unwrap();
        crate::trading_research::init(&c).unwrap();
        crate::paper_strategy::init(&c).unwrap();
        init(&c).unwrap();
        for id in ["a", "b"] {
            c.execute("INSERT INTO research_workspace VALUES(?1,?2,'now')",params![id,json!({"name":id,"symbols":["600693.SH"],"allocations":[{"model_id":"ak-state-v1","weight":1.}],"sentiment_scope":"pool"}).to_string()]).unwrap();
            c.execute(
                "INSERT INTO paper_strategy VALUES(?1,?1,?1,50000000,'{}','now')",
                [id],
            )
            .unwrap();
            c.execute(
                "INSERT INTO paper_cash_ledger VALUES(?1,?1,50000000,'initial_funding','now')",
                [id],
            )
            .unwrap();
        }
        c
    }
    fn now() -> DateTime<Utc> {
        "2026-10-26T02:00:00Z".parse().unwrap()
    }
    fn quote() -> Quote {
        Quote {
            price: 1000,
            at: now(),
            upper: Some(1100),
            lower: Some(900),
            tradable: Some(true),
            corporate_actions_clear: true,
            available: Some(100000),
        }
    }
    fn order(c: &Connection, id: &str, sell: bool) {
        c.execute("INSERT OR IGNORE INTO execution_run VALUES('run','a','key','a',NULL,'test','{}','now')",[]).unwrap();
        c.execute("INSERT INTO execution_order VALUES(?1,'run','a',?1,?2,100,1000,'rejected','test','now')",params![id,if sell{"sell"}else{"buy"}]).unwrap();
    }
    #[test]
    fn cash_fifo_t1_and_account_isolation() {
        let c = setup();
        order(&c, "buy", false);
        let tx = c.unchecked_transaction().unwrap();
        let (n, f) = fill(
            &tx,
            "buy",
            "a",
            "600693.SH",
            false,
            100,
            &quote(),
            now(),
            50000000,
            0,
        )
        .unwrap();
        tx.commit().unwrap();
        assert_eq!(cash(&c, "a").unwrap(), 50000000 - n - f);
        assert_eq!(cash(&c, "b").unwrap(), 50000000);
        assert_eq!(
            quantity(&c, "a", "600693.SH", Some("2026-10-26")).unwrap(),
            0
        );
        order(&c, "sell", true);
        let tx = c.unchecked_transaction().unwrap();
        assert!(fill(
            &tx,
            "sell",
            "a",
            "600693.SH",
            true,
            100,
            &quote(),
            now(),
            50000000,
            0
        )
        .is_err());
        drop(tx);
        let tomorrow = now() + chrono::Duration::days(1);
        let mut q = quote();
        q.at = tomorrow;
        let tx = c.unchecked_transaction().unwrap();
        let (n2, f2) = fill(
            &tx,
            "sell",
            "a",
            "600693.SH",
            true,
            100,
            &q,
            tomorrow,
            50000000,
            100000,
        )
        .unwrap();
        tx.commit().unwrap();
        assert_eq!(quantity(&c, "a", "600693.SH", None).unwrap(), 0);
        assert_eq!(cash(&c, "a").unwrap(), 50000000 - n - f + n2 - f2);
    }
    #[test]
    fn gates_reject_lot_stale_limit_cash_and_exposure() {
        let c = setup();
        order(&c, "o", false);
        let q = quote();
        assert!(fill(
            &c,
            "o",
            "a",
            "600693.SH",
            false,
            101,
            &q,
            now(),
            50000000,
            0
        )
        .is_err());
        assert!(fill(
            &c,
            "o",
            "a",
            "600693.SH",
            false,
            10000,
            &q,
            now(),
            50000000,
            0
        )
        .is_err());
        assert!(fill(&c, "o", "a", "600693.SH", false, 100, &q, now(), 100000, 0).is_err());
        assert!(fill(
            &c,
            "o",
            "a",
            "600693.SH",
            false,
            100,
            &q,
            now(),
            50000000,
            40000000
        )
        .is_err());
        let mut bad = q.clone();
        bad.at -= chrono::Duration::seconds(11);
        assert!(quote_gate(&bad, now()).is_err());
        bad = q.clone();
        bad.tradable = Some(false);
        assert!(quote_gate(&bad, now()).is_err());
        bad = q.clone();
        bad.upper = None;
        assert!(quote_gate(&bad, now()).is_err());
        bad = q.clone();
        bad.corporate_actions_clear = false;
        assert!(quote_gate(&bad, now()).is_err());
        assert_eq!(cash(&c, "a").unwrap(), 50000000);
    }
    #[test]
    fn atomic_rollback_and_duplicate_fill() {
        let c = setup();
        order(&c, "o", false);
        c.execute_batch("CREATE TRIGGER fail_lot BEFORE INSERT ON execution_lot BEGIN SELECT RAISE(ABORT,'disk');END;").unwrap();
        let tx = c.unchecked_transaction().unwrap();
        assert!(fill(
            &tx,
            "o",
            "a",
            "600693.SH",
            false,
            100,
            &quote(),
            now(),
            50000000,
            0
        )
        .is_err());
        drop(tx);
        assert_eq!(cash(&c, "a").unwrap(), 50000000);
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM execution_fill", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        c.execute_batch("DROP TRIGGER fail_lot").unwrap();
        let tx = c.unchecked_transaction().unwrap();
        fill(
            &tx,
            "o",
            "a",
            "600693.SH",
            false,
            100,
            &quote(),
            now(),
            50000000,
            0,
        )
        .unwrap();
        tx.commit().unwrap();
        let balance = cash(&c, "a").unwrap();
        let tx = c.unchecked_transaction().unwrap();
        assert!(fill(
            &tx,
            "o",
            "a",
            "600693.SH",
            false,
            100,
            &quote(),
            now(),
            50000000,
            0
        )
        .is_err());
        drop(tx);
        assert_eq!(cash(&c, "a").unwrap(), balance);
    }
    #[test]
    fn checks_are_idempotent_and_never_fabricate_trades() {
        let c = setup();
        let a = run(&c, "a", "k", now()).unwrap();
        let b = run(&c, "a", "k", now()).unwrap();
        assert_eq!(a["id"], b["id"]);
        assert_eq!(b["reused"], true);
        let r = records(&c, "a").unwrap();
        assert_eq!(r["orders"].as_array().unwrap().len(), 1);
        assert_eq!(r["orders"][0]["status"], "rejected");
        assert_eq!(r["cash_fen"], 50000000);
        assert!(r["positions"].as_array().unwrap().is_empty());
        assert!(records(&c, "b").unwrap()["orders"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(run(&c, "missing", "k", now()).is_err());
    }
    #[test]
    fn fees_and_sessions() {
        assert_eq!(cost(100000, false), 501);
        assert_eq!(cost(100000, true), 551);
        assert!(session(now()));
        assert!(!session("2026-10-26T04:00:00Z".parse().unwrap()));
        assert!(!session("2026-10-26T06:57:00Z".parse().unwrap()));
    }
}
