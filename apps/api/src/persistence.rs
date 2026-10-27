//! Persistence policies, bounded caches and maintenance. Unknown datasets are never deleted.
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    db::AppState,
    error::{envelope, ApiResponse, AppError},
    models::Bar,
};

pub const POLICY_VERSION: i64 = 1;
pub const DESIGN: &str = "openspec/changes/data-retention-and-sqlite-performance/design.md";
static COMMITS: AtomicU64 = AtomicU64::new(0);
static CHANGED: AtomicU64 = AtomicU64::new(0);
static BUSY: AtomicU64 = AtomicU64::new(0);
static LOCK_US: AtomicU64 = AtomicU64::new(0);
static WRITE_US: AtomicU64 = AtomicU64::new(0);

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    for (table, name, declaration) in [
        (
            "live_cache",
            "last_accessed_at",
            "INTEGER NOT NULL DEFAULT 0",
        ),
        ("live_cache", "policy_version", "INTEGER NOT NULL DEFAULT 1"),
        (
            "quote_snapshot",
            "observation_hash",
            "TEXT NOT NULL DEFAULT ''",
        ),
        ("quote_snapshot", "provider_time", "TEXT"),
        (
            "quote_snapshot",
            "last_checked_at",
            "TEXT NOT NULL DEFAULT ''",
        ),
    ] {
        let columns = tx
            .prepare(&format!("PRAGMA table_info({table})"))?
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if !columns.iter().any(|c| c == name) {
            tx.execute_batch(&format!(
                "ALTER TABLE {table} ADD COLUMN {name} {declaration}"
            ))?;
        }
    }
    tx.execute_batch(include_str!("persistence.sql"))?;
    tx.execute(
        "INSERT OR IGNORE INTO app_setting(key,value) VALUES('retention.auto_cleanup','false')",
        [],
    )?;
    let policies: Value =
        serde_json::from_str(include_str!("persistence-policies.json")).expect("policy JSON");
    for family in ["tables", "datasets"] {
        for (name, policy) in policies[family].as_object().unwrap() {
            tx.execute("INSERT INTO lifecycle_policy(dataset,policy_version,config,updated_at) VALUES(?1,?2,?3,?4) ON CONFLICT(dataset) DO UPDATE SET config=excluded.config,policy_version=excluded.policy_version WHERE lifecycle_policy.config<>excluded.config",params![format!("{family}/{name}"),POLICY_VERSION,policy.to_string(),Utc::now().timestamp()])?;
        }
    }
    tx.commit()
}

/// Stable structural fingerprint includes columns, defaults, keys, FKs, checks and indexes.
pub fn schema_report(conn: &Connection) -> rusqlite::Result<Value> {
    let names = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?.query_map([], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let mut out = serde_json::Map::new();
    for name in names {
        let sql = conn.prepare("SELECT type,name,sql FROM sqlite_master WHERE tbl_name=?1 AND sql IS NOT NULL ORDER BY type,name")?.query_map([&name], |r| Ok(format!("{}|{}|{}", r.get::<_,String>(0)?, r.get::<_,String>(1)?, r.get::<_,String>(2)?.split_whitespace().collect::<Vec<_>>().join(" "))))?.collect::<rusqlite::Result<Vec<_>>>()?.join("\n");
        let columns = conn
            .prepare(&format!("PRAGMA table_info(\"{name}\")"))?
            .query_map([], |r| {
                Ok(format!(
                    "{} {}",
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        out.insert(
            name,
            json!({"sha256":hash(sql.as_bytes()),"columns":columns}),
        );
    }
    Ok(Value::Object(out))
}

pub async fn run_db<T: Send + 'static>(
    state: AppState,
    op: impl FnOnce(&Connection) -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    static LIMIT: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);
    let permit = tokio::time::timeout(Duration::from_secs(1), LIMIT.acquire())
        .await
        .map_err(|_| busy())?
        .map_err(|_| busy())?;
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let start = Instant::now();
        let conn = loop {
            match state.db.try_lock() {
                Ok(conn) => break conn,
                Err(std::sync::TryLockError::Poisoned(_)) => {
                    return Err(AppError::internal("database lock poisoned"))
                }
                Err(_) if start.elapsed() >= Duration::from_secs(1) => return Err(busy()),
                Err(_) => std::thread::sleep(Duration::from_millis(2)),
            }
        };
        LOCK_US.fetch_add(start.elapsed().as_micros() as u64, Ordering::Relaxed);
        op(&conn)
    })
    .await
    .map_err(|e| AppError::internal(e.to_string()))?;
    result
}
fn busy() -> AppError {
    BUSY.fetch_add(1, Ordering::Relaxed);
    AppError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "DB_BUSY",
        "数据库繁忙，请稍后重试",
    )
}

pub type PreparedQuotes = Vec<(crate::sources::Snapshot, String)>;

pub fn prepare_realtime(
    snapshots: &[crate::sources::Snapshot],
) -> Result<PreparedQuotes, AppError> {
    if snapshots.is_empty() || snapshots.len() > 20 {
        return Err(AppError::invalid_data("报价批次须为1至20只"));
    }
    let mut symbols = std::collections::HashSet::new();
    let mut rows = Vec::new();
    for s in snapshots {
        s.validate().map_err(AppError::invalid_data)?;
        crate::live::symbol_code(&s.symbol).map_err(AppError::invalid_data)?;
        if !symbols.insert(&s.symbol)
            || [
                s.open,
                s.high,
                s.low,
                s.close,
                s.prev_close,
                s.volume,
                s.amount,
                s.change_pct,
            ]
            .iter()
            .any(|n| !n.is_finite())
            || s.volume < 0.0
            || s.amount < 0.0
            || s.high < s.open.max(s.close)
            || s.low > s.open.min(s.close)
        {
            return Err(AppError::invalid_data("报价批次包含重复标的或无效OHLCV"));
        }
        // Provider has no trustworthy timestamp in this endpoint. Do not hash a fabricated date.
        rows.push((
            s.clone(),
            hash(
                serde_json::to_string(&json!([
                    s.symbol,
                    s.open,
                    s.high,
                    s.low,
                    s.close,
                    s.prev_close,
                    s.volume,
                    s.amount,
                    s.change_pct
                ]))
                .unwrap()
                .as_bytes(),
            ),
        ));
    }
    Ok(rows)
}

pub fn publish_realtime(
    conn: &Connection,
    snapshots: &[crate::sources::Snapshot],
    now: &str,
) -> Result<(), AppError> {
    publish_prepared(conn, prepare_realtime(snapshots)?, now)
}

pub fn publish_prepared(
    conn: &Connection,
    rows: PreparedQuotes,
    now: &str,
) -> Result<(), AppError> {
    let start = Instant::now();
    let tx = conn.unchecked_transaction()?;
    let mut changed = 0;
    for (s, h) in rows {
        changed += tx.execute("INSERT INTO quote_snapshot(symbol,name,trade_date,open,high,low,close,prev_close,volume,amount,change_pct,source,batch_id,fetched_at,observation_hash,provider_time,last_checked_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'axdata','realtime',?12,?13,NULL,?12) ON CONFLICT(symbol) DO UPDATE SET name=CASE WHEN excluded.name='' THEN quote_snapshot.name ELSE excluded.name END,trade_date=excluded.trade_date,open=excluded.open,high=excluded.high,low=excluded.low,close=excluded.close,prev_close=excluded.prev_close,volume=excluded.volume,amount=excluded.amount,change_pct=excluded.change_pct,source=excluded.source,batch_id=excluded.batch_id,fetched_at=excluded.fetched_at,observation_hash=excluded.observation_hash,provider_time=NULL,last_checked_at=excluded.last_checked_at WHERE quote_snapshot.observation_hash<>excluded.observation_hash", params![s.symbol,s.name,s.date,s.open,s.high,s.low,s.close,s.prev_close,s.volume,s.amount,s.change_pct,now,h])?;
        tx.execute(
            "UPDATE quote_snapshot SET last_checked_at=?2 WHERE symbol=?1 AND last_checked_at<>?2",
            params![s.symbol, now],
        )?;
    }
    tx.commit()?;
    COMMITS.fetch_add(1, Ordering::Relaxed);
    CHANGED.fetch_add(changed as u64, Ordering::Relaxed);
    WRITE_US.fetch_add(start.elapsed().as_micros() as u64, Ordering::Relaxed);
    Ok(())
}

pub fn store_bars(
    conn: &Connection,
    symbol: &str,
    period: &str,
    source: &str,
    bars: &[Bar],
    now: i64,
) -> Result<(), AppError> {
    let limit = if period.starts_with("second_") {
        360
    } else {
        1024
    };
    // Validate all input before taking the transaction; replace buckets, never add cumulative volume.
    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for b in bars {
        let timestamp: i64 = b
            .date
            .parse()
            .map_err(|_| AppError::invalid_data("bar timestamp"))?;
        if !seen.insert(timestamp)
            || [b.open, b.high, b.low, b.close, b.volume, b.amount]
                .iter()
                .any(|n| !n.is_finite())
            || b.low <= 0.0
            || b.low > b.open.min(b.close)
            || b.high < b.open.max(b.close)
            || b.volume < 0.0
            || b.amount < 0.0
        {
            return Err(AppError::invalid_data("bar OHLCV"));
        }
        let payload =
            serde_json::to_string(b).map_err(|e| AppError::invalid_data(e.to_string()))?;
        let h = hash(
            serde_json::to_string(&json!([b.open, b.high, b.low, b.close, b.volume, b.amount]))
                .unwrap()
                .as_bytes(),
        );
        rows.push((timestamp, payload, h));
    }
    rows.sort_by_key(|row| row.0);
    if rows.len() > limit as usize {
        rows.drain(..rows.len() - limit as usize);
    }
    let tx = conn.unchecked_transaction()?;
    for (timestamp, payload, h) in rows {
        tx.execute("INSERT INTO bar_cache(symbol,period,bucket_start,source,payload,observation_hash,revision,observed_at,last_accessed_at) VALUES(?1,?2,?3,?4,?5,?6,1,?7,?7) ON CONFLICT(symbol,period,bucket_start,source) DO UPDATE SET payload=excluded.payload,observation_hash=excluded.observation_hash,revision=bar_cache.revision+1,observed_at=excluded.observed_at WHERE bar_cache.observation_hash<>excluded.observation_hash",params![symbol,period,timestamp,source,payload,h,now])?;
    }
    tx.execute("DELETE FROM bar_cache WHERE symbol=?1 AND period=?2 AND source=?3 AND bucket_start NOT IN (SELECT bucket_start FROM bar_cache WHERE symbol=?1 AND period=?2 AND source=?3 ORDER BY bucket_start DESC LIMIT ?4)",params![symbol,period,source,limit])?;
    tx.commit()?;
    Ok(())
}

pub fn diagnostics(conn: &Connection) -> Result<Value, AppError> {
    let mut pragmas = serde_json::Map::new();
    for p in [
        "journal_mode",
        "foreign_keys",
        "busy_timeout",
        "synchronous",
        "wal_autocheckpoint",
        "page_count",
        "freelist_count",
        "page_size",
    ] {
        let value: rusqlite::types::Value =
            conn.query_row(&format!("PRAGMA {p}"), [], |r| r.get(0))?;
        pragmas.insert(
            p.into(),
            match value {
                rusqlite::types::Value::Integer(n) => json!(n),
                rusqlite::types::Value::Text(t) => json!(t),
                _ => Value::Null,
            },
        );
    }
    let path: String = conn.query_row(
        "SELECT file FROM pragma_database_list WHERE name='main'",
        [],
        |r| r.get(0),
    )?;
    let size = |suffix: &str| {
        std::fs::metadata(format!("{path}{suffix}"))
            .map(|m| m.len())
            .unwrap_or(0)
    };
    let names=conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?.query_map([],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let mut counts = serde_json::Map::new();
    for name in names {
        let n: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM \"{name}\""), [], |r| {
            r.get(0)
        })?;
        counts.insert(name, json!(n));
    }
    Ok(
        json!({"path":path,"sqlite_version":rusqlite::version(),"pragmas":pragmas,"db_bytes":size(""),"wal_bytes":size("-wal"),"shm_bytes":size("-shm"),"wal_alert":size("-wal")>=64*1024*1024,"rows":counts,"metrics":{"realtime_commits":COMMITS.load(Ordering::Relaxed),"changed_quotes":CHANGED.load(Ordering::Relaxed),"busy":BUSY.load(Ordering::Relaxed),"lock_wait_us":LOCK_US.load(Ordering::Relaxed),"write_us":WRITE_US.load(Ordering::Relaxed)}}),
    )
}

pub async fn retention(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    run_db(state,|conn| {
        let policies:Value=serde_json::from_str(include_str!("persistence-policies.json")).map_err(|e|AppError::internal(e.to_string()))?;
        Ok(envelope(json!({"policy_version":POLICY_VERSION,"design":DESIGN,"policies":policies,"diagnostics":diagnostics(conn)?})))
    }).await
}

#[derive(Deserialize)]
pub struct CleanupBody {
    pub plan_id: String,
    #[serde(default = "default_dry_run")]
    pub dry_run: bool,
}
fn default_dry_run() -> bool {
    true
}

fn candidates(conn: &Connection, now: i64) -> Result<Vec<Value>, AppError> {
    let mut out = Vec::new();
    // Deliberate allowlist: unknown datasets, operational state and all business tables are excluded.
    let policies: Value = serde_json::from_str(include_str!("persistence-policies.json")).unwrap();
    let mut stmt =
        conn.prepare("SELECT symbol,dataset,last_accessed_at,attempted_at FROM live_cache")?;
    for row in stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
        ))
    })? {
        let (symbol, dataset, access, attempt) = row?;
        let key = if dataset.starts_with("second_") {
            "second_*"
        } else if dataset.starts_with("minute_") {
            "minute_*"
        } else {
            &dataset
        };
        let Some(days) = policies["datasets"][key]["idle_days"].as_i64() else {
            continue;
        };
        if access == 0 || now - access < days * 86400 || now - attempt < 60 {
            continue;
        }
        let protected:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM subscription_lease WHERE symbol=?1 AND dataset=?2 AND expires_at>?3)",params![symbol,dataset,now],|r|r.get(0))?;
        if !protected {
            out.push(json!({"table":"live_cache","symbol":symbol,"dataset":dataset}));
        }
    }
    let mut stmt=conn.prepare("SELECT DISTINCT b.symbol,b.period FROM bar_cache b WHERE b.last_accessed_at>0 AND b.last_accessed_at<?1 AND NOT EXISTS(SELECT 1 FROM live_cache c WHERE c.symbol=b.symbol AND c.dataset=b.period AND (c.last_accessed_at>=?1 OR c.attempted_at>=?2))")?;
    for row in stmt.query_map([now - 7 * 86400, now - 60], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })? {
        let (symbol, dataset) = row?;
        let protected:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM subscription_lease WHERE symbol=?1 AND dataset=?2 AND expires_at>?3)",params![symbol,dataset,now],|r|r.get(0))?;
        if !protected {
            out.push(json!({"table":"bar_cache","symbol":symbol,"dataset":dataset}));
        }
    }
    Ok(out)
}

pub async fn preview(State(state): State<AppState>) -> Result<Json<ApiResponse<Value>>, AppError> {
    run_db(state,|conn|{
        let now=Utc::now().timestamp(); let id=uuid::Uuid::new_v4().to_string();let items=candidates(conn,now)?;
        conn.execute("INSERT INTO cleanup_plan(id,policy_version,created_at,expires_at,candidates,status) VALUES(?1,?2,?3,?4,?5,'preview')",params![id,POLICY_VERSION,now,now+600,json!({"policy_hash":hash(include_bytes!("persistence-policies.json")),"items":items}).to_string()])?;
        Ok(envelope(json!({"plan_id":id,"policy_version":POLICY_VERSION,"expires_at":now+600,"candidate_keys":items.len(),"items":items,"dry_run":true})))
    }).await
}

pub async fn cleanup(
    State(state): State<AppState>,
    Json(body): Json<CleanupBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    run_db(state, move |conn| {
        execute_cleanup(conn, &body, Utc::now().timestamp()).map(envelope)
    })
    .await
}
fn execute_cleanup(conn: &Connection, body: &CleanupBody, now: i64) -> Result<Value, AppError> {
    let plan: Option<(i64, i64, String, String)> = conn
        .query_row(
            "SELECT policy_version,expires_at,candidates,status FROM cleanup_plan WHERE id=?1",
            [&body.plan_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let (version, expires, payload, status) =
        plan.ok_or_else(|| AppError::not_found("清理计划不存在"))?;
    if status == "complete" {
        return cleanup_result(conn, &body.plan_id);
    }
    if version != POLICY_VERSION || now >= expires {
        return Err(AppError::bad_request(
            "清理计划已过期或政策已变化，请重新预览",
        ));
    }
    let saved: Value =
        serde_json::from_str(&payload).map_err(|e| AppError::internal(e.to_string()))?;
    if saved["policy_hash"].as_str()
        != Some(hash(include_bytes!("persistence-policies.json")).as_str())
    {
        return Err(AppError::bad_request("清理政策已变化，请重新预览"));
    }
    let planned: Vec<Value> = serde_json::from_value(saved["items"].clone())
        .map_err(|e| AppError::internal(e.to_string()))?;
    if body.dry_run {
        return Ok(json!({"plan_id":body.plan_id,"dry_run":true,"candidate_keys":planned.len()}));
    }
    let maintenance: Option<String> = conn
        .query_row(
            "SELECT value FROM app_setting WHERE key='retention.maintenance'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if maintenance.as_deref() == Some("true") {
        return Err(AppError::bad_request(
            "恢复维护模式禁止执行清理，请先完成恢复验收",
        ));
    }
    let mut deleted = 0usize;
    let mut skipped = 0usize;
    for item in planned {
        loop {
            let tx = conn.unchecked_transaction()?;
            // Recheck inside the same writer transaction. Frozen research inputs live independently.
            let fresh = candidates(&tx, now)?;
            if !fresh.contains(&item) {
                skipped += 1;
                break;
            }
            let symbol = item["symbol"].as_str().unwrap();
            let dataset = item["dataset"].as_str().unwrap();
            let n = if item["table"] == "live_cache" {
                tx.execute(
                    "DELETE FROM live_cache WHERE symbol=?1 AND dataset=?2",
                    params![symbol, dataset],
                )?
            } else {
                tx.execute("DELETE FROM bar_cache WHERE rowid IN (SELECT rowid FROM bar_cache WHERE symbol=?1 AND period=?2 LIMIT 1000)",params![symbol,dataset])?
            };
            tx.execute("INSERT INTO cleanup_audit(plan_id,table_name,record_key,deleted_rows,created_at) VALUES(?1,?2,?3,?4,?5)",params![body.plan_id,item["table"].as_str(),format!("{symbol}/{dataset}"),n,now])?;
            tx.commit()?;
            deleted += n;
            if item["table"] == "live_cache" || n < 1000 {
                break;
            }
        }
    }
    conn.execute(
        "UPDATE cleanup_plan SET status='complete' WHERE id=?1",
        [&body.plan_id],
    )?;
    Ok(
        json!({"plan_id":body.plan_id,"dry_run":false,"deleted_rows":deleted,"skipped_keys":skipped}),
    )
}
fn cleanup_result(conn: &Connection, id: &str) -> Result<Value, AppError> {
    let status: String = conn
        .query_row("SELECT status FROM cleanup_plan WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .optional()?
        .ok_or_else(|| AppError::not_found("清理计划不存在"))?;
    let deleted: i64 = conn.query_row(
        "SELECT COALESCE(SUM(deleted_rows),0) FROM cleanup_audit WHERE plan_id=?1",
        [id],
        |r| r.get(0),
    )?;
    Ok(json!({"plan_id":id,"status":status,"dry_run":false,"deleted_rows":deleted}))
}
pub async fn result(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    run_db(state, move |conn| cleanup_result(conn, &id).map(envelope)).await
}

pub fn load_bars(
    conn: &Connection,
    symbol: &str,
    period: &str,
) -> Result<Option<String>, AppError> {
    let mut stmt=conn.prepare("SELECT payload FROM bar_cache WHERE symbol=?1 AND period=?2 AND source=COALESCE(NULLIF((SELECT source FROM live_cache WHERE symbol=?1 AND dataset=?2),''),(SELECT source FROM bar_cache WHERE symbol=?1 AND period=?2 ORDER BY observed_at DESC,source LIMIT 1)) ORDER BY bucket_start")?;
    let rows = stmt
        .query_map(params![symbol, period], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if rows.is_empty() {
        return Ok(None);
    }
    let mut bars: Vec<Bar> = rows
        .into_iter()
        .map(|s| serde_json::from_str(&s))
        .collect::<Result<_, _>>()
        .map_err(|e| AppError::internal(e.to_string()))?;
    crate::indicators::apply(&mut bars);
    Ok(Some(
        serde_json::to_string(&bars).map_err(|e| AppError::internal(e.to_string()))?,
    ))
}

/// Frozen input bytes are never collected by the display-cache cleaner.
pub fn freeze_input(
    conn: &Connection,
    root: &std::path::Path,
    owner: &str,
    source: &str,
    available_at: Option<&str>,
    bytes: &[u8],
) -> Result<String, AppError> {
    use std::io::Write;
    let h = hash(bytes);
    std::fs::create_dir_all(root).map_err(|e| AppError::internal(e.to_string()))?;
    let path = root.join(&h);
    if !path.exists() {
        let tmp = root.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| AppError::internal(e.to_string()))?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| AppError::internal(e.to_string()))?;
        std::fs::rename(&tmp, &path).map_err(|e| AppError::internal(e.to_string()))?;
    }
    let actual = std::fs::read(&path).map_err(|e| AppError::internal(e.to_string()))?;
    if hash(&actual) != h {
        return Err(AppError::internal("input checksum mismatch"));
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute("INSERT OR IGNORE INTO input_manifest(hash,relative_path,size_bytes,source,available_at,created_at) VALUES(?1,?1,?2,?3,?4,?5)",params![h,bytes.len(),source,available_at,Utc::now().timestamp()])?;
    tx.execute(
        "INSERT OR IGNORE INTO data_reference(owner,input_hash,created_at) VALUES(?1,?2,?3)",
        params![owner, h, Utc::now().timestamp()],
    )?;
    tx.commit()?;
    Ok(h)
}
pub fn read_input(conn: &Connection, root: &std::path::Path, h: &str) -> Result<Vec<u8>, AppError> {
    let error = || {
        AppError::new(
            StatusCode::NOT_FOUND,
            "INPUT_UNAVAILABLE",
            "固定研究输入不可用",
        )
    };
    if h.len() != 64 || !h.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(error());
    }
    let found: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM input_manifest WHERE hash=?1)",
        [h],
        |r| r.get(0),
    )?;
    if !found {
        return Err(error());
    }
    let bytes = std::fs::read(root.join(h)).map_err(|_| error())?;
    if hash(&bytes) != h {
        return Err(error());
    }
    Ok(bytes)
}

#[derive(Deserialize)]
pub struct FreezeBody {
    pub owner: String,
    pub source: String,
    pub available_at: Option<String>,
    pub payload: Value,
}
fn input_root() -> std::path::PathBuf {
    std::env::var_os("ALPHAK_INPUT_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(
                std::env::var("ALPHAK_DB").unwrap_or_else(|_| "alphak.db".into()),
            )
            .with_extension("inputs")
        })
}
pub async fn freeze(
    State(state): State<AppState>,
    Json(body): Json<FreezeBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    if body.owner.trim().is_empty() || body.source.trim().is_empty() {
        return Err(AppError::bad_request("owner和source不能为空"));
    }
    if body
        .available_at
        .as_deref()
        .is_some_and(|s| chrono::DateTime::parse_from_rfc3339(s).is_err())
    {
        return Err(AppError::bad_request("available_at必须为RFC3339或null"));
    }
    let bytes =
        serde_json::to_vec(&body.payload).map_err(|e| AppError::invalid_data(e.to_string()))?;
    run_db(state, move |conn| {
        freeze_input(
            conn,
            &input_root(),
            &body.owner,
            &body.source,
            body.available_at.as_deref(),
            &bytes,
        )
        .map(|h| envelope(json!({"hash":h})))
    })
    .await
}
pub async fn frozen(
    State(state): State<AppState>,
    Path(h): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    run_db(state,move|conn|{let bytes=read_input(conn,&input_root(),&h)?;Ok(envelope(json!({"hash":h,"payload":serde_json::from_slice::<Value>(&bytes).map_err(|e|AppError::internal(e.to_string()))?})))}).await
}

pub fn observe_breadth(conn: &Connection, payload: &Value, now: i64) -> Result<(), AppError> {
    use chrono::{Datelike, Timelike};
    let time = payload["as_of"]
        .as_str()
        .and_then(|s| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok())
        .ok_or_else(|| AppError::invalid_data("市场宽度缺少真实时间"))?;
    let date = time.date().to_string();
    let is_open: Option<bool> = conn
        .query_row(
            "SELECT is_open FROM trading_calendar WHERE trade_date=?1",
            [&date],
            |r| r.get(0),
        )
        .optional()?;
    if time.weekday().number_from_monday() > 5 || is_open == Some(false) {
        return Err(AppError::invalid_data("休市日期市场宽度不发布为研究观测"));
    }
    let serialized = payload.to_string();
    let h = hash(serialized.as_bytes());
    let phase = if time.hour() >= 15 {
        "close"
    } else {
        "intraday"
    };
    conn.execute("INSERT OR IGNORE INTO market_breadth_observation(trade_date,source,scope,provider_time,observed_at,available_at,phase,payload,observation_hash) VALUES(?1,?2,?3,?4,?5,?5,?6,?7,?8)",params![date,payload["source"].as_str().unwrap_or("unknown"),payload["scope"].as_str().unwrap_or("unknown"),payload["as_of"].as_str(),now,phase,serialized,h])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn conn() -> Connection {
        crate::db::init_db(":memory:").unwrap()
    }
    fn quote(symbol: &str) -> crate::sources::Snapshot {
        crate::sources::Snapshot {
            symbol: symbol.into(),
            name: "test".into(),
            date: "2026-10-23".into(),
            open: 10.0,
            high: 12.0,
            low: 9.0,
            close: 11.0,
            prev_close: 10.0,
            volume: 100.0,
            amount: 1100.0,
            change_pct: 10.0,
        }
    }
    fn bar(time: i64) -> Bar {
        serde_json::from_value(json!({"date":time.to_string(),"open":10.0,"high":12.0,"low":9.0,"close":11.0,"volume":100.0,"amount":1100.0})).unwrap()
    }
    #[test]
    fn schema_and_policy_must_match_design() {
        let conn = conn();
        let report = schema_report(&conn).unwrap();
        let doc = include_str!(
            "../../../openspec/changes/data-retention-and-sqlite-performance/design.md"
        );
        let policies: Value =
            serde_json::from_str(include_str!("persistence-policies.json")).unwrap();
        assert_eq!(
            report.as_object().unwrap().len(),
            policies["tables"].as_object().unwrap().len(),
            "update policies + design.md"
        );
        for (name, item) in report.as_object().unwrap() {
            assert!(
                policies["tables"].get(name).is_some(),
                "{name} missing policy"
            );
            assert!(doc.contains(&format!("| {name} | {} | `{}` |",item["columns"].as_array().unwrap().iter().map(|v|v.as_str().unwrap()).collect::<Vec<_>>().join(", "),item["sha256"].as_str().unwrap())),"{name}: schema changed; update lifecycle design and run make persistence-doc-update");
        }
        assert!(
            doc.contains(&hash(include_bytes!("persistence-policies.json"))),
            "retention policy changed; update design.md"
        );
    }
    #[test]
    fn realtime_validation_atomicity_dedup_and_unknown_time() {
        let conn = conn();
        let a = quote("002463.SZ");
        let mut b = quote("600519.SH");
        b.volume = -1.0;
        assert!(publish_realtime(&conn, &[a.clone(), b], "t1").is_err());
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM quote_snapshot", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
        publish_realtime(&conn, std::slice::from_ref(&a), "t1").unwrap();
        publish_realtime(&conn, &[a], "t2").unwrap();
        let row: (String, String, Option<String>) = conn
            .query_row(
                "SELECT fetched_at,last_checked_at,provider_time FROM quote_snapshot",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(row, ("t1".into(), "t2".into(), None));
    }
    #[test]
    fn realtime_commit_failure_rolls_back_whole_batch() {
        let conn = conn();
        conn.execute_batch("CREATE TRIGGER fail_quote BEFORE INSERT ON quote_snapshot WHEN NEW.symbol='600519.SH' BEGIN SELECT RAISE(ABORT,'injected disk write failure'); END;").unwrap();
        assert!(publish_realtime(&conn, &[quote("002463.SZ"), quote("600519.SH")], "t").is_err());
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM quote_snapshot", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
    #[test]
    fn bar_windows_revisions_order_and_volume_are_bounded() {
        let conn = conn();
        let bars: Vec<_> = (0..400).rev().map(bar).collect();
        store_bars(&conn, "002463.SZ", "second_3", "fixture", &bars, 1).unwrap();
        store_bars(&conn, "002463.SZ", "second_3", "fixture", &bars, 2).unwrap();
        let row: (i64, i64) = conn
            .query_row("SELECT COUNT(*),MAX(revision) FROM bar_cache", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(row, (360, 1));
        let mut updated = bar(399);
        updated.volume = 150.0;
        store_bars(&conn, "002463.SZ", "second_3", "fixture", &[updated], 3).unwrap();
        let payload: String = conn
            .query_row(
                "SELECT payload FROM bar_cache WHERE bucket_start=399",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&payload).unwrap()["volume"],
            150.0
        );
        let rev: i64 = conn
            .query_row(
                "SELECT revision FROM bar_cache WHERE bucket_start=399",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rev, 2);
        let mut invalid = bar(400);
        invalid.high = 1.0;
        assert!(store_bars(
            &conn,
            "002463.SZ",
            "second_3",
            "fixture",
            &[bar(400), invalid],
            4
        )
        .is_err());
    }
    #[test]
    fn cleanup_protects_unknown_active_and_long_term_data() {
        let conn = conn();
        let now = 2_000_000_000;
        for (symbol, dataset) in [
            ("002463.SZ", "news"),
            ("600519.SH", "unknown"),
            ("600000.SH", "news"),
        ] {
            conn.execute("INSERT INTO live_cache(symbol,dataset,payload,last_accessed_at) VALUES(?1,?2,'{}',1)",params![symbol,dataset]).unwrap();
        }
        let items = candidates(&conn, now).unwrap();
        assert_eq!(items.len(), 2);
        conn.execute(
            "INSERT INTO cleanup_plan VALUES('plan',1,?1,?2,?3,'preview')",
            params![now, now + 600, json!({"policy_hash":hash(include_bytes!("persistence-policies.json")),"items":items}).to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO subscription_lease VALUES('test','600000.SH','news',?1)",
            [now + 30],
        )
        .unwrap();
        let watch_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM watchlist_item", [], |r| r.get(0))
            .unwrap();
        let dry: CleanupBody = serde_json::from_value(json!({"plan_id":"plan"})).unwrap();
        assert!(dry.dry_run);
        execute_cleanup(&conn, &dry, now).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM live_cache", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 3);
        let result = execute_cleanup(
            &conn,
            &CleanupBody {
                plan_id: "plan".into(),
                dry_run: false,
            },
            now,
        )
        .unwrap();
        assert_eq!(result["deleted_rows"], 1);
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM watchlist_item", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, watch_before);
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM live_cache", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2);
        assert_eq!(
            execute_cleanup(
                &conn,
                &CleanupBody {
                    plan_id: "plan".into(),
                    dry_run: false
                },
                now
            )
            .unwrap()["deleted_rows"],
            1
        );
    }
    #[test]
    fn cleanup_rejects_changed_policy_and_restore_maintenance() {
        let conn = conn();
        let body = CleanupBody {
            plan_id: "guarded".into(),
            dry_run: false,
        };
        let payload = json!({"policy_hash":"obsolete", "items":[]}).to_string();
        conn.execute(
            "INSERT INTO cleanup_plan VALUES('guarded',1,0,600,?1,'preview')",
            [payload],
        )
        .unwrap();
        assert!(execute_cleanup(&conn, &body, 1).is_err());
        let payload =
            json!({"policy_hash":hash(include_bytes!("persistence-policies.json")), "items":[]})
                .to_string();
        conn.execute("UPDATE cleanup_plan SET candidates=?1", [payload])
            .unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO app_setting VALUES('retention.maintenance','true')",
            [],
        )
        .unwrap();
        assert!(execute_cleanup(&conn, &body, 1).is_err());
        let dry = CleanupBody {
            plan_id: "guarded".into(),
            dry_run: true,
        };
        assert!(execute_cleanup(&conn, &dry, 1).is_ok());
    }
    #[test]
    fn expired_cleanup_plan_is_rejected() {
        let conn = conn();
        conn.execute(
            "INSERT INTO cleanup_plan VALUES('old',1,0,600,'[]','preview')",
            [],
        )
        .unwrap();
        assert!(execute_cleanup(
            &conn,
            &CleanupBody {
                plan_id: "old".into(),
                dry_run: false
            },
            600
        )
        .is_err());
    }
    #[test]
    fn frozen_inputs_survive_display_cleanup_and_detect_missing_bytes() {
        let conn = conn();
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let h = freeze_input(
            &conn,
            &root,
            "experiment-1",
            "fixture",
            Some("2026-10-23T15:00:00+08:00"),
            b"historical-input",
        )
        .unwrap();
        assert_eq!(read_input(&conn, &root, &h).unwrap(), b"historical-input");
        assert_eq!(
            freeze_input(
                &conn,
                &root,
                "experiment-2",
                "fixture",
                None,
                b"historical-input"
            )
            .unwrap(),
            h
        );
        std::fs::remove_file(root.join(&h)).unwrap();
        assert_eq!(
            read_input(&conn, &root, &h).unwrap_err().code,
            "INPUT_UNAVAILABLE"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn breadth_preserves_revisions_and_rejects_weekends() {
        let conn = conn();
        let mut data = json!({"as_of":"2026-10-23 15:00:00","source":"fixture","scope":"aggregate","advance":100});
        observe_breadth(&conn, &data, 10).unwrap();
        observe_breadth(&conn, &data, 11).unwrap();
        data["advance"] = json!(101);
        observe_breadth(&conn, &data, 12).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM market_breadth_observation", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(n, 2);
        data["as_of"] = json!("2026-10-25 12:00:00");
        assert!(observe_breadth(&conn, &data, 13).is_err());
    }
    #[test]
    fn file_migrations_are_idempotent_and_durable() {
        let path = std::env::temp_dir().join(format!("alphak-test-{}.db", uuid::Uuid::new_v4()));
        let conn = crate::db::init_db(path.to_str().unwrap()).unwrap();
        let before = schema_report(&conn).unwrap();
        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM quote_daily", [], |r| r.get(0))
            .unwrap();
        drop(conn);
        let conn = crate::db::init_db(path.to_str().unwrap()).unwrap();
        assert_eq!(schema_report(&conn).unwrap(), before);
        let after: i64 = conn
            .query_row("SELECT COUNT(*) FROM quote_daily", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, after);
        let d = diagnostics(&conn).unwrap();
        assert_eq!(d["pragmas"]["journal_mode"], "wal");
        assert_eq!(d["pragmas"]["busy_timeout"], 1000);
        assert_eq!(d["pragmas"]["synchronous"], 2);
        assert_eq!(d["pragmas"]["foreign_keys"], 1);
        drop(conn);
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn blocking_lock_timeout_is_finite() {
        let state = crate::db::test_state();
        let db = state.db.clone();
        let (ready_rx, ready_tx) = std::sync::mpsc::channel();
        let holder = std::thread::spawn(move || {
            let _conn = db.lock().unwrap();
            ready_rx.send(()).unwrap();
            std::thread::sleep(Duration::from_millis(1250));
        });
        ready_tx.recv().unwrap();
        let start = Instant::now();
        let result = run_db(state, |_| Ok(())).await;
        assert_eq!(result.unwrap_err().code, "DB_BUSY");
        assert!(start.elapsed() < Duration::from_secs(2));
        holder.join().unwrap();
    }
}

pub fn sampling_window(conn: &Connection, now: chrono::DateTime<Utc>) -> Result<bool, AppError> {
    use chrono::{Datelike, Timelike};
    let local = now.with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap());
    let open: Option<bool> = conn
        .query_row(
            "SELECT is_open FROM trading_calendar WHERE trade_date=?1",
            [local.date_naive().to_string()],
            |r| r.get(0),
        )
        .optional()?;
    let minute = local.hour() * 60 + local.minute();
    Ok(local.weekday().number_from_monday() <= 5
        && open != Some(false)
        && ((9 * 60 + 15..=11 * 60 + 30).contains(&minute)
            || (13 * 60..=15 * 60 + 5).contains(&minute)))
}

#[derive(Deserialize)]
pub struct LeaseBody {
    pub client_id: String,
    pub symbols: Vec<String>,
    pub dataset: String,
}
pub async fn lease(
    State(state): State<AppState>,
    Json(body): Json<LeaseBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    if body.client_id.is_empty()
        || body.client_id.len() > 100
        || body.symbols.len() > 120
        || !(body.dataset == "realtime"
            || body.dataset.starts_with("second_")
            || body.dataset.starts_with("minute_"))
    {
        return Err(AppError::bad_request("订阅参数无效"));
    }
    for symbol in &body.symbols {
        crate::live::symbol_code(symbol).map_err(AppError::bad_request)?;
    }
    run_db(state,move|conn|{
        let now=Utc::now();let active=sampling_window(conn,now)?;
        let tx=conn.unchecked_transaction()?;
        tx.execute("DELETE FROM subscription_lease WHERE client_id=?1 OR expires_at<=?2",params![body.client_id,now.timestamp()])?;
        for symbol in body.symbols{tx.execute("INSERT OR REPLACE INTO subscription_lease(client_id,symbol,dataset,expires_at) VALUES(?1,?2,?3,?4)",params![body.client_id,symbol,body.dataset,now.timestamp()+30])?;}
        tx.commit()?;Ok(envelope(json!({"expires_at":now.timestamp()+30,"sampling":active})))
    }).await
}

pub async fn checkpoint(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    run_db(state, |conn| {
        let (busy, pages, checkpointed): (i64, i64, i64) =
            conn.query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?;
        Ok(envelope(
            json!({"mode":"PASSIVE","busy":busy,"pages":pages,"checkpointed":checkpointed}),
        ))
    })
    .await
}

#[cfg(test)]
mod lease_tests {
    use super::*;
    #[test]
    fn lunch_weekends_holidays_and_closing_windows() {
        let conn = crate::db::init_db(":memory:").unwrap();
        let yes = |s: &str| {
            sampling_window(
                &conn,
                chrono::DateTime::parse_from_rfc3339(s)
                    .unwrap()
                    .with_timezone(&Utc),
            )
            .unwrap()
        };
        assert!(yes("2026-10-23T09:30:00+08:00"));
        assert!(!yes("2026-10-23T12:00:00+08:00"));
        assert!(yes("2026-10-23T15:02:00+08:00"));
        assert!(!yes("2026-10-23T16:00:00+08:00"));
        assert!(!yes("2026-10-25T10:00:00+08:00"));
    }
}
