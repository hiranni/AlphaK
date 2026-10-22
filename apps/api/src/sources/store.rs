use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db::now_iso;
use crate::error::AppError;

use super::{DailyBar, SourceBatch};

#[derive(Debug, Clone, Serialize)]
pub struct PublishResult {
    pub dataset: String,
    pub source: String,
    pub batch_id: String,
    pub inserted: usize,
    pub updated: usize,
    pub skipped: usize,
    pub adj_pending: usize,
}

pub fn publish_daily(
    conn: &Connection,
    batch: &SourceBatch<Vec<DailyBar>>,
    scope: &[String],
    start: &str,
    end: &str,
    batch_id: &str,
) -> Result<PublishResult, AppError> {
    let tx = conn.unchecked_transaction()?;
    let now = now_iso();
    let mut inserted = 0;
    let mut updated = 0;
    let mut skipped = 0;
    let mut adj_pending = 0;
    for symbol in scope {
        tx.execute(
            "DELETE FROM quote_daily WHERE symbol = ?1 AND source = 'demo'",
            params![symbol],
        )?;
    }
    for bar in &batch.data {
        if !scope.iter().any(|symbol| symbol == &bar.symbol)
            || bar.date.as_str() < start
            || bar.date.as_str() > end
        {
            skipped += 1;
            continue;
        }
        bar.validate().map_err(AppError::invalid_data)?;
        let existed: bool = tx
            .query_row(
                "SELECT 1 FROM quote_daily WHERE symbol = ?1 AND trade_date = ?2",
                params![bar.symbol, bar.date],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);
        let factor = bar.adj_factor.unwrap_or(1.0);
        let adj_source = if bar.adj_factor.is_some() {
            batch.source.as_str()
        } else {
            adj_pending += 1;
            ""
        };
        let adj_batch = if bar.adj_factor.is_some() {
            batch_id
        } else {
            ""
        };
        tx.execute(
            "INSERT INTO quote_daily
             (symbol, trade_date, open, high, low, close, volume, amount, adj_factor, turnover,
              source, batch_id, fetched_at, adj_source, adj_batch_id, amount_known)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(symbol, trade_date) DO UPDATE SET
               open = excluded.open, high = excluded.high, low = excluded.low, close = excluded.close,
               volume = excluded.volume,
               amount = CASE WHEN excluded.amount_known = 0 THEN quote_daily.amount ELSE excluded.amount END,
               amount_known = CASE WHEN excluded.amount_known = 0 THEN quote_daily.amount_known ELSE 1 END,
               adj_factor = CASE WHEN excluded.adj_source = '' THEN quote_daily.adj_factor ELSE excluded.adj_factor END,
               adj_source = CASE WHEN excluded.adj_source = '' THEN quote_daily.adj_source ELSE excluded.adj_source END,
               adj_batch_id = CASE WHEN excluded.adj_batch_id = '' THEN quote_daily.adj_batch_id ELSE excluded.adj_batch_id END,
               source = excluded.source, batch_id = excluded.batch_id, fetched_at = excluded.fetched_at",
            params![
                bar.symbol,
                bar.date,
                bar.open,
                bar.high,
                bar.low,
                bar.close,
                bar.volume,
                bar.amount,
                factor,
                batch.source.as_str(),
                batch_id,
                now,
                adj_source,
                adj_batch,
                if bar.amount_known { 1 } else { 0 },
            ],
        )?;
        if existed {
            updated += 1;
        } else {
            inserted += 1;
        }
    }
    tx.commit()?;
    Ok(PublishResult {
        dataset: batch.dataset.clone(),
        source: batch.source.as_str().to_string(),
        batch_id: batch_id.to_string(),
        inserted,
        updated,
        skipped,
        adj_pending,
    })
}

pub fn publish_index(
    conn: &Connection,
    batch: &SourceBatch<Vec<DailyBar>>,
    scope: &[String],
    start: &str,
    end: &str,
    batch_id: &str,
) -> Result<PublishResult, AppError> {
    let tx = conn.unchecked_transaction()?;
    let now = now_iso();
    let mut inserted = 0;
    let mut updated = 0;
    let mut skipped = 0;
    for code in scope {
        tx.execute(
            "DELETE FROM index_quote WHERE index_code = ?1 AND source = 'demo'",
            params![code],
        )?;
    }
    for bar in &batch.data {
        if !scope.iter().any(|code| code == &bar.symbol)
            || bar.date.as_str() < start
            || bar.date.as_str() > end
        {
            skipped += 1;
            continue;
        }
        bar.validate().map_err(AppError::invalid_data)?;
        let existed: bool = tx
            .query_row(
                "SELECT 1 FROM index_quote WHERE index_code = ?1 AND trade_date = ?2",
                params![bar.symbol, bar.date],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);
        tx.execute(
            "INSERT INTO index_quote
             (index_code, trade_date, open, high, low, close, volume, amount, source, batch_id, fetched_at, amount_known)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(index_code, trade_date) DO UPDATE SET
               open = excluded.open, high = excluded.high, low = excluded.low, close = excluded.close,
               volume = excluded.volume,
               amount = CASE WHEN excluded.amount_known = 0 THEN index_quote.amount ELSE excluded.amount END,
               amount_known = CASE WHEN excluded.amount_known = 0 THEN index_quote.amount_known ELSE 1 END,
               source = excluded.source, batch_id = excluded.batch_id, fetched_at = excluded.fetched_at",
            params![
                bar.symbol,
                bar.date,
                bar.open,
                bar.high,
                bar.low,
                bar.close,
                bar.volume,
                bar.amount,
                batch.source.as_str(),
                batch_id,
                now,
                if bar.amount_known { 1 } else { 0 },
            ],
        )?;
        if existed {
            updated += 1;
        } else {
            inserted += 1;
        }
    }
    tx.commit()?;
    Ok(PublishResult {
        dataset: batch.dataset.clone(),
        source: batch.source.as_str().to_string(),
        batch_id: batch_id.to_string(),
        inserted,
        updated,
        skipped,
        adj_pending: 0,
    })
}

pub fn mark_fresh(
    conn: &Connection,
    dataset: &str,
    batch_id: &str,
    source: &str,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO data_freshness (dataset, status, last_success_at, last_source, last_batch_id, last_error, updated_at)
         VALUES (?1, 'fresh', ?2, ?3, ?4, '', ?2)
         ON CONFLICT(dataset) DO UPDATE SET status = 'fresh', last_success_at = excluded.last_success_at,
           last_source = excluded.last_source, last_batch_id = excluded.last_batch_id, last_error = '', updated_at = excluded.updated_at",
        params![dataset, now_iso(), source, batch_id],
    )?;
    Ok(())
}

pub fn mark_stale(conn: &Connection, dataset: &str, error: &str) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO data_freshness (dataset, status, last_success_at, last_source, last_batch_id, last_error, updated_at)
         VALUES (?1, 'stale', NULL, NULL, NULL, ?2, ?3)
         ON CONFLICT(dataset) DO UPDATE SET status = 'stale', last_error = excluded.last_error, updated_at = excluded.updated_at",
        params![dataset, error, now_iso()],
    )?;
    Ok(())
}

pub fn freshness(conn: &Connection) -> Result<Vec<serde_json::Value>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT dataset, status, last_success_at, last_source, last_batch_id, last_error, updated_at FROM data_freshness ORDER BY dataset",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(serde_json::json!({
            "dataset": r.get::<_, String>(0)?,
            "status": r.get::<_, String>(1)?,
            "last_success_at": r.get::<_, Option<String>>(2)?,
            "last_source": r.get::<_, Option<String>>(3)?,
            "last_batch_id": r.get::<_, Option<String>>(4)?,
            "last_error": r.get::<_, Option<String>>(5)?,
            "updated_at": r.get::<_, String>(6)?,
        }))
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn publish_snapshot(
    conn: &Connection,
    batch: &SourceBatch<super::Snapshot>,
    batch_id: &str,
) -> Result<(), AppError> {
    let snapshot = &batch.data;
    snapshot.validate().map_err(AppError::invalid_data)?;
    conn.execute(
        "INSERT OR REPLACE INTO quote_snapshot
         (symbol, name, trade_date, open, high, low, close, prev_close, volume, amount, change_pct, source, batch_id, fetched_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            snapshot.symbol,
            snapshot.name,
            snapshot.date,
            snapshot.open,
            snapshot.high,
            snapshot.low,
            snapshot.close,
            snapshot.prev_close,
            snapshot.volume,
            snapshot.amount,
            snapshot.change_pct,
            batch.source.as_str(),
            batch_id,
            now_iso(),
        ],
    )?;
    Ok(())
}

pub fn mark_partial(conn: &Connection, dataset: &str, error: &str) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO data_freshness (dataset, status, last_success_at, last_source, last_batch_id, last_error, updated_at)
         VALUES (?1, 'partial', NULL, NULL, NULL, ?2, ?3)
         ON CONFLICT(dataset) DO UPDATE SET status = 'partial', last_error = excluded.last_error, updated_at = excluded.updated_at",
        params![dataset, error, now_iso()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::{DailyBar, Snapshot, SourceBatch, SourceId};

    fn bar() -> DailyBar {
        DailyBar {
            symbol: "600519.SH".into(),
            date: "2026-10-21".into(),
            open: 1300.0,
            high: 1320.0,
            low: 1290.0,
            close: 1309.69,
            volume: 33514.0,
            amount: 4403823070.0,
            amount_known: true,
            adj_factor: Some(1.01),
        }
    }

    #[test]
    fn publish_records_source_and_batch() {
        let conn = crate::db::init_db(":memory:").unwrap();
        let batch = SourceBatch {
            dataset: "quote_daily".into(),
            source: SourceId::Tencent,
            attempts: 1,
            data: vec![bar()],
            adj_source: Some(SourceId::Tencent),
            adj_error: None,
        };
        let result = publish_daily(
            &conn,
            &batch,
            &["600519.SH".into()],
            "2026-10-21",
            "2026-10-21",
            "batch-1",
        )
        .unwrap();
        assert_eq!(result.inserted + result.updated, 1);
        let row: (String, String, String, f64) = conn
            .query_row(
                "SELECT source, batch_id, adj_source, adj_factor FROM quote_daily WHERE symbol = '600519.SH' AND trade_date = '2026-10-21'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(row.0, "tencent");
        assert_eq!(row.1, "batch-1");
        assert_eq!(row.2, "tencent");
        assert!((row.3 - 1.01).abs() < 1e-9);
    }

    #[test]
    fn unknown_amount_does_not_zero_existing_value() {
        let conn = crate::db::init_db(":memory:").unwrap();
        conn.execute(
            "UPDATE quote_daily SET source = 'eastmoney', amount = 5017886932.274232, amount_known = 1 WHERE symbol = '600519.SH' AND trade_date = '2026-10-21'",
            [],
        )
        .unwrap();
        let existing: f64 = conn
            .query_row(
                "SELECT amount FROM quote_daily WHERE symbol = '600519.SH' AND trade_date = '2026-10-21'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let batch = SourceBatch {
            dataset: "quote_daily".into(),
            source: SourceId::Tencent,
            attempts: 1,
            data: vec![DailyBar {
                symbol: "600519.SH".into(),
                date: "2026-10-21".into(),
                open: 1300.0,
                high: 1320.0,
                low: 1290.0,
                close: 1309.69,
                volume: 33514.0,
                amount: 0.0,
                amount_known: false,
                adj_factor: Some(1.0),
            }],
            adj_source: Some(SourceId::Tencent),
            adj_error: None,
        };
        publish_daily(
            &conn,
            &batch,
            &["600519.SH".into()],
            "2026-10-21",
            "2026-10-21",
            "amount-1",
        )
        .unwrap();
        let (amount, known): (f64, i64) = conn
            .query_row(
                "SELECT amount, amount_known FROM quote_daily WHERE symbol = '600519.SH' AND trade_date = '2026-10-21'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(amount, existing);
        assert_eq!(known, 1);
    }

    #[test]
    fn retry_queue_preserves_exact_scope() {
        let conn = crate::db::init_db(":memory:").unwrap();
        queue_retry(
            &conn,
            "quote_daily",
            "000858.SZ",
            "2024-01-02",
            "2024-01-31",
            "timeout",
        )
        .unwrap();
        let tasks = pending_retries(&conn, Some("quote_daily")).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].symbol, "000858.SZ");
        assert_eq!(tasks[0].start, "2024-01-02");
        assert_eq!(tasks[0].end, "2024-01-31");
        clear_retry(
            &conn,
            "quote_daily",
            "000858.SZ",
            "2024-01-02",
            "2024-01-31",
        )
        .unwrap();
        assert!(!has_pending(&conn, "quote_daily").unwrap());
    }

    #[test]
    fn stale_keeps_last_success_data() {
        let conn = crate::db::init_db(":memory:").unwrap();
        mark_fresh(&conn, "quote_daily", "batch-1", "tencent").unwrap();
        mark_stale(&conn, "quote_daily", "both sources failed").unwrap();
        let status: String = conn
            .query_row(
                "SELECT status FROM data_freshness WHERE dataset = 'quote_daily'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "stale");
        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM quote_daily", [], |r| r.get(0))
            .unwrap();
        assert!(rows > 0);
        mark_fresh(&conn, "quote_daily", "batch-2", "eastmoney").unwrap();
        let status: String = conn
            .query_row(
                "SELECT status FROM data_freshness WHERE dataset = 'quote_daily'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "fresh");
    }

    #[test]
    fn publishes_snapshot() {
        let conn = crate::db::init_db(":memory:").unwrap();
        let batch = SourceBatch {
            dataset: "snapshot".into(),
            source: SourceId::Eastmoney,
            attempts: 1,
            data: Snapshot {
                symbol: "600519.SH".into(),
                name: "贵州茅台".into(),
                date: "2026-10-21".into(),
                open: 1331.88,
                high: 1331.98,
                low: 1302.36,
                close: 1309.69,
                prev_close: 1331.88,
                volume: 33514.0,
                amount: 4403823070.0,
                change_pct: -1.67,
            },
            adj_source: None,
            adj_error: None,
        };
        publish_snapshot(&conn, &batch, "snap-1").unwrap();
        let source: String = conn
            .query_row(
                "SELECT source FROM quote_snapshot WHERE symbol = '600519.SH'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(source, "eastmoney");
    }
}

#[derive(Debug, Clone)]
pub struct RetryTask {
    pub dataset: String,
    pub symbol: String,
    pub start: String,
    pub end: String,
    pub attempts: i64,
    pub last_error: String,
}

pub fn queue_retry(
    conn: &Connection,
    dataset: &str,
    symbol: &str,
    start: &str,
    end: &str,
    error: &str,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO source_retry_task (dataset, symbol, start_date, end_date, attempts, last_error, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?6)
         ON CONFLICT(dataset, symbol, start_date, end_date) DO UPDATE SET
           last_error = excluded.last_error,
           updated_at = excluded.updated_at",
        params![dataset, symbol, start, end, error, now_iso()],
    )?;
    Ok(())
}

pub fn clear_retry(
    conn: &Connection,
    dataset: &str,
    symbol: &str,
    start: &str,
    end: &str,
) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM source_retry_task WHERE dataset = ?1 AND symbol = ?2 AND start_date = ?3 AND end_date = ?4",
        params![dataset, symbol, start, end],
    )?;
    Ok(())
}

pub fn pending_retries(
    conn: &Connection,
    dataset: Option<&str>,
) -> Result<Vec<RetryTask>, AppError> {
    let sql = if dataset.is_some() {
        "SELECT dataset, symbol, start_date, end_date, attempts, last_error FROM source_retry_task WHERE dataset = ?1 ORDER BY created_at"
    } else {
        "SELECT dataset, symbol, start_date, end_date, attempts, last_error FROM source_retry_task ORDER BY created_at"
    };
    let mut stmt = conn.prepare(sql)?;
    let map_row = |r: &rusqlite::Row<'_>| {
        Ok(RetryTask {
            dataset: r.get(0)?,
            symbol: r.get(1)?,
            start: r.get(2)?,
            end: r.get(3)?,
            attempts: r.get(4)?,
            last_error: r.get(5)?,
        })
    };
    let rows = match dataset {
        Some(dataset) => stmt
            .query_map(params![dataset], map_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?,
        None => stmt
            .query_map([], map_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?,
    };
    Ok(rows)
}

pub fn has_pending(conn: &Connection, dataset: &str) -> Result<bool, AppError> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM source_retry_task WHERE dataset = ?1 LIMIT 1",
            params![dataset],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false))
}
