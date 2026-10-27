//! Frozen paper-account registration. No matching or broker order path.
use crate::{
    db::AppState,
    error::{envelope, ApiResponse, AppError},
    persistence,
    trading_research::{validate, Config},
};
use axum::{
    extract::{Path, State},
    Json,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
pub fn init(c: &Connection) -> rusqlite::Result<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS paper_strategy(id TEXT PRIMARY KEY,request_key TEXT NOT NULL UNIQUE,workspace_id TEXT NOT NULL UNIQUE REFERENCES research_workspace(id),initial_cash_fen INTEGER NOT NULL CHECK(initial_cash_fen>0),policy TEXT NOT NULL,created_at TEXT NOT NULL); CREATE TABLE IF NOT EXISTS paper_cash_ledger(id TEXT PRIMARY KEY,strategy_id TEXT NOT NULL UNIQUE REFERENCES paper_strategy(id),amount_fen INTEGER NOT NULL CHECK(amount_fen>0),kind TEXT NOT NULL CHECK(kind='initial_funding'),created_at TEXT NOT NULL); CREATE TABLE IF NOT EXISTS paper_strategy_event(id TEXT PRIMARY KEY,strategy_id TEXT NOT NULL REFERENCES paper_strategy(id),kind TEXT NOT NULL CHECK(kind IN ('archive','restore','reconfigure')),payload TEXT NOT NULL,created_at TEXT NOT NULL); CREATE INDEX IF NOT EXISTS paper_strategy_event_recent ON paper_strategy_event(strategy_id,created_at DESC);")
}
fn policy() -> Value {
    json!({"version":"a-share-registration-v1","execution_status":"awaiting_execution_engine","currency":"CNY","initial_positions":[],"max_symbols":8,"max_single_position":0.20,"max_gross_position":0.80,"shorting":false,"leverage":false,"buy_lot":100,"price_tick_fen":1,"sell_rule":"T+1；仅卖出已交收可用持仓；零股按交易所规则一次性卖出","sessions":"Asia/Shanghai 09:30–11:30、13:00–14:57；暂不支持集合竞价","calendar":"交易所交易日历，非工作日推断；缺失则禁止交易","limit_rule":"逐股读取官方当日涨跌停价、风险警示、新股例外、停牌状态；缺失则禁止交易，不统一假定10%","data_gate":"报价过期、复权口径不一致或除权信息不完整时禁止交易","costs":"撮合前必须配置并版本化佣金、最低佣金、卖出印花税、过户费与滑点；当前无成交","execution_gate":"无撮合引擎，无自动下单；本记录仅初始化资金与研究配置"})
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    request_key: String,
    config: Config,
    initial_cash_fen: i64,
}
fn validate_symbols(c: &Connection, config: &Config) -> Result<(), AppError> {
    for symbol in &config.symbols {
        let valid:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM instruments WHERE symbol=?1 AND exchange IN ('SH','SZ','SSE','SZSE') AND status='active')",[symbol],|r|r.get(0))?;
        let mainboard = symbol.ends_with(".SH")
            && (symbol.starts_with("600")
                || symbol.starts_with("601")
                || symbol.starts_with("603")
                || symbol.starts_with("605"))
            || symbol.ends_with(".SZ")
                && (symbol.starts_with("000")
                    || symbol.starts_with("001")
                    || symbol.starts_with("002")
                    || symbol.starts_with("003"));
        if !valid || !mainboard {
            return Err(AppError::bad_request(format!(
                "当前策略仅接受有效沪深主板股票：{symbol}"
            )));
        }
    }
    Ok(())
}
fn register(c: &Connection, x: Create) -> Result<Value, AppError> {
    validate(&x.config)?;
    if x.request_key.trim().is_empty()
        || x.request_key.len() > 100
        || x.initial_cash_fen <= 0
        || x.initial_cash_fen > 100_000_000_000
    {
        return Err(AppError::bad_request("无效初始化金额或请求标识"));
    }
    let tx = c.unchecked_transaction()?;
    let raw = serde_json::to_string(&x.config).map_err(|e| AppError::internal(e.to_string()))?;
    let old:Option<(String,String,i64)>=tx.query_row("SELECT p.id,w.config,p.initial_cash_fen FROM paper_strategy p JOIN research_workspace w ON w.id=p.workspace_id WHERE request_key=?1",[&x.request_key],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    if let Some((id, config, cash)) = old {
        if config != raw || cash != x.initial_cash_fen {
            return Err(AppError::bad_request("请求标识已用于不同配置"));
        }
        return Ok(json!({"id":id,"created":false}));
    }
    validate_symbols(&tx, &x.config)?;
    let id = uuid::Uuid::new_v4().to_string();
    let workspace = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    tx.execute(
        "INSERT INTO research_workspace VALUES(?1,?2,?3)",
        params![workspace, raw, now],
    )?;
    tx.execute(
        "INSERT INTO paper_strategy VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            id,
            x.request_key,
            workspace,
            x.initial_cash_fen,
            policy().to_string(),
            now
        ],
    )?;
    tx.execute(
        "INSERT INTO paper_cash_ledger VALUES(?1,?2,?3,'initial_funding',?4)",
        params![
            uuid::Uuid::new_v4().to_string(),
            id,
            x.initial_cash_fen,
            now
        ],
    )?;
    tx.commit()?;
    Ok(json!({"id":id,"workspace_id":workspace,"created":true}))
}
pub async fn create(
    State(s): State<AppState>,
    Json(x): Json<Create>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    Ok(envelope(
        persistence::run_db(s, move |c| register(c, x)).await?,
    ))
}
pub async fn list(State(s): State<AppState>) -> Result<Json<ApiResponse<Value>>, AppError> {
    let out=persistence::run_db(s,|c|{let mut q=c.prepare("SELECT p.id,p.workspace_id,p.initial_cash_fen,p.policy,p.created_at,w.config,COALESCE((SELECT kind FROM paper_strategy_event e WHERE e.strategy_id=p.id AND kind IN ('archive','restore') ORDER BY rowid DESC LIMIT 1),'restore'),((SELECT SUM(amount_fen) FROM paper_cash_ledger l WHERE l.strategy_id=p.id)+COALESCE((SELECT SUM(amount_fen) FROM execution_cash e WHERE e.account_id=p.id),0)) FROM paper_strategy p JOIN research_workspace w ON w.id=p.workspace_id ORDER BY p.created_at DESC")?;
 let rows=q.query_map([],|r|Ok(json!({"id":r.get::<_,String>(0)?,"workspace_id":r.get::<_,String>(1)?,"initial_cash_fen":r.get::<_,i64>(2)?,"policy":serde_json::from_str::<Value>(&r.get::<_,String>(3)?).unwrap_or(Value::Null),"created_at":r.get::<_,String>(4)?,"config":serde_json::from_str::<Value>(&r.get::<_,String>(5)?).unwrap_or(Value::Null),"archived":r.get::<_,String>(6)?=="archive","cash_fen":r.get::<_,i64>(7)?})))?.collect::<Result<Vec<_>,_>>()?;Ok(json!(rows))}).await?;
    Ok(envelope(out))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Update {
    action: String,
    config: Option<Config>,
}
fn change(c: &Connection, id: String, x: Update) -> Result<Value, AppError> {
    let tx = c.unchecked_transaction()?;
    let old: Option<String> = tx
        .query_row(
            "SELECT workspace_id FROM paper_strategy WHERE id=?1",
            [&id],
            |r| r.get(0),
        )
        .optional()?;
    let old = old.ok_or_else(|| AppError::not_found("账户不存在"))?;
    let archived:bool=tx.query_row("SELECT COALESCE((SELECT kind FROM paper_strategy_event WHERE strategy_id=?1 AND kind IN ('archive','restore') ORDER BY rowid DESC LIMIT 1),'restore')='archive'",[&id],|r|r.get(0))?;
    let now = chrono::Utc::now().to_rfc3339();
    let payload = match x.action.as_str() {
        "archive" | "restore" => {
            if x.config.is_some() {
                return Err(AppError::bad_request("状态操作不接受配置"));
            }
            json!({"previous_archived":archived})
        }
        "reconfigure" => {
            if archived {
                return Err(AppError::bad_request("请先恢复账户"));
            }
            let config = x
                .config
                .ok_or_else(|| AppError::bad_request("缺少策略配置"))?;
            validate(&config)?;
            validate_symbols(&tx, &config)?;
            let workspace = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO research_workspace VALUES(?1,?2,?3)",
                params![workspace, serde_json::to_string(&config).unwrap(), now],
            )?;
            tx.execute(
                "UPDATE paper_strategy SET workspace_id=?1 WHERE id=?2",
                params![workspace, id],
            )?;
            json!({"previous_workspace_id":old,"workspace_id":workspace})
        }
        _ => return Err(AppError::bad_request("无效操作")),
    };
    tx.execute(
        "INSERT INTO paper_strategy_event VALUES(?1,?2,?3,?4,?5)",
        params![
            uuid::Uuid::new_v4().to_string(),
            id,
            x.action,
            payload.to_string(),
            now
        ],
    )?;
    tx.commit()?;
    Ok(json!({"id":id,"result":payload}))
}
pub async fn update(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(x): Json<Update>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    Ok(envelope(
        persistence::run_db(s, move |c| change(c, id, x)).await?,
    ))
}
pub async fn events(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    Ok(envelope(persistence::run_db(s,move|c|{
 let exists:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM paper_strategy WHERE id=?1)",[&id],|r|r.get(0))?;if !exists{return Err(AppError::not_found("账户不存在"))}
 let mut q=c.prepare("SELECT kind,payload,created_at FROM paper_strategy_event WHERE strategy_id=?1 ORDER BY rowid DESC")?;
 let rows=q.query_map([id],|r|Ok(json!({"kind":r.get::<_,String>(0)?,"payload":serde_json::from_str::<Value>(&r.get::<_,String>(1)?).unwrap_or(Value::Null),"created_at":r.get::<_,String>(2)?})))?.collect::<Result<Vec<_>,_>>()?;Ok(json!(rows))
 }).await?))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn funding_is_atomic_and_idempotent() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("PRAGMA foreign_keys=ON;CREATE TABLE instruments(symbol TEXT,exchange TEXT,status TEXT);INSERT INTO instruments VALUES('600693.SH','SH','active');").unwrap();
        crate::trading_research::init(&c).unwrap();
        init(&c).unwrap();
        let make = || Create {
            request_key: "test".into(),
            initial_cash_fen: 50_000_000,
            config: Config {
                name: "测试".into(),
                symbols: vec!["600693.SH".into()],
                allocations: vec![crate::trading_research::Allocation {
                    model_id: "ak-state-v1".into(),
                    weight: 1.,
                }],
                sentiment_scope: "pool".into(),
            },
        };
        register(&c, make()).unwrap();
        register(&c, make()).unwrap();
        assert_eq!(
            c.query_row("SELECT SUM(amount_fen) FROM paper_cash_ledger", [], |r| r
                .get::<_, i64>(
                0
            ))
            .unwrap(),
            50_000_000
        );
        let id: String = c
            .query_row("SELECT id FROM paper_strategy", [], |r| r.get(0))
            .unwrap();
        change(
            &c,
            id.clone(),
            Update {
                action: "archive".into(),
                config: None,
            },
        )
        .unwrap();
        assert!(change(
            &c,
            id.clone(),
            Update {
                action: "reconfigure".into(),
                config: Some(make().config)
            }
        )
        .is_err());
        change(
            &c,
            id.clone(),
            Update {
                action: "restore".into(),
                config: None,
            },
        )
        .unwrap();
        let old: String = c
            .query_row("SELECT workspace_id FROM paper_strategy", [], |r| r.get(0))
            .unwrap();
        change(
            &c,
            id.clone(),
            Update {
                action: "reconfigure".into(),
                config: Some(make().config),
            },
        )
        .unwrap();
        let new: String = c
            .query_row("SELECT workspace_id FROM paper_strategy", [], |r| r.get(0))
            .unwrap();
        assert_ne!(old, new);
        assert_eq!(
            c.query_row("SELECT SUM(amount_fen) FROM paper_cash_ledger", [], |r| r
                .get::<_, i64>(
                0
            ))
            .unwrap(),
            50_000_000
        );
        let mut conflict = make();
        conflict.initial_cash_fen = 1;
        assert!(register(&c, conflict).is_err());
        let mut bad = make();
        bad.request_key = "bad".into();
        bad.config.symbols = vec!["600160.SH".into()];
        assert!(register(&c, bad).is_err());
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM research_workspace", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
    }
}
