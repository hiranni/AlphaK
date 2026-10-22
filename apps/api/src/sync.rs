use chrono::{Utc, Weekday};
use rusqlite::{params, Connection, OptionalExtension};

use crate::db::now_iso;
use crate::error::AppError;
use crate::models::{DataJob, QualityReport};

pub const DATASETS: &[&str] = &[
    "instruments",
    "quote_daily",
    "daily_basic",
    "financial_report",
    "index_quote",
    "company_profile",
    "market_snapshot",
];

pub fn is_schedule_window(weekday: Weekday, hour: u32, minute: u32) -> bool {
    if matches!(weekday, Weekday::Sat | Weekday::Sun) {
        return false;
    }
    (hour == 15 && minute == 10) || (hour == 18 && minute == 0)
}

pub fn scheduled_datasets(hour: u32) -> &'static [&'static str] {
    if hour == 18 {
        &["market_snapshot"]
    } else {
        &[
            "quote_daily",
            "daily_basic",
            "financial_report",
            "index_quote",
            "company_profile",
            "market_snapshot",
        ]
    }
}

pub fn list_jobs(conn: &Connection) -> Result<Vec<DataJob>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, dataset, status, progress, message, started_at, finished_at
         FROM data_job ORDER BY started_at DESC LIMIT 30",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(DataJob {
            id: r.get(0)?,
            dataset: r.get(1)?,
            status: r.get(2)?,
            progress: r.get(3)?,
            message: r.get(4)?,
            started_at: r.get(5)?,
            finished_at: r.get(6)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn run_request(
    conn: &Connection,
    dataset: &str,
    request_id: Option<&str>,
) -> Result<Vec<DataJob>, AppError> {
    run_internal(conn, dataset, false, request_id)
}

pub fn run_scheduled(
    conn: &Connection,
    dataset: &str,
    window: &str,
) -> Result<Vec<DataJob>, AppError> {
    run_internal(conn, dataset, false, Some(window))
}

fn run_internal(
    conn: &Connection,
    dataset: &str,
    force_fail: bool,
    request_id: Option<&str>,
) -> Result<Vec<DataJob>, AppError> {
    if !DATASETS.contains(&dataset) {
        return Err(AppError::bad_request(format!(
            "unknown dataset: {dataset}; allowed: {}",
            DATASETS.join(", ")
        )));
    }

    let request_id = request_id
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let key = format!(
        "{}:{}:{}",
        dataset,
        Utc::now().format("%Y-%m-%d"),
        request_id
    );
    let existing: Option<(String, String)> = conn
        .query_row(
            "SELECT id, status FROM data_job WHERE idempotency_key = ?1 ORDER BY started_at DESC LIMIT 1",
            params![key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;

    let id = if let Some((id, status)) = existing {
        match status.as_str() {
            "succeeded" | "queued" | "running" => return list_jobs(conn),
            _ => {
                conn.execute(
                    "UPDATE data_job SET status = 'running', progress = 0, message = '重试中', started_at = ?1, finished_at = NULL, attempt = attempt + 1, updated_at = ?1 WHERE id = ?2",
                    params![now_iso(), id],
                )?;
                id
            }
        }
    } else {
        let id = format!("job-{}", uuid::Uuid::new_v4());
        conn.execute(
            "INSERT INTO data_job (id, dataset, status, progress, message, started_at, finished_at, idempotency_key, attempt, updated_at)
             VALUES (?1, ?2, 'queued', 0, '排队中', ?3, NULL, ?4, 1, ?3)",
            params![id, dataset, now_iso(), key],
        )?;
        conn.execute(
            "UPDATE data_job SET status = 'running', progress = 20, message = '同步中', updated_at = ?1 WHERE id = ?2",
            params![now_iso(), id],
        )?;
        id
    };

    let max_attempts = 3;
    let mut attempt = 1;
    let result = loop {
        let attempt_result = if force_fail {
            Err(AppError::internal("simulated source timeout"))
        } else {
            sync_dataset(conn, dataset)
        };
        match attempt_result {
            Ok(message) => break Ok(message),
            Err(err) if attempt < max_attempts => {
                attempt += 1;
                conn.execute(
                    "UPDATE data_job SET status = 'running', progress = 20, message = ?1, attempt = ?2, updated_at = ?3 WHERE id = ?4",
                    params![format!("重试 {}/{}：{}", attempt, max_attempts, err.message), attempt, now_iso(), id],
                )?;
                tracing::warn!(dataset, attempt, error = %err.message, "sync retry");
                std::thread::sleep(std::time::Duration::from_millis(100 * attempt as u64));
            }
            Err(err) => break Err(err),
        }
    };
    match result {
        Ok(message) => {
            conn.execute(
                "UPDATE data_job SET status = 'succeeded', progress = 100, message = ?1, attempt = ?2, finished_at = ?3, updated_at = ?3 WHERE id = ?4",
                params![message, attempt, now_iso(), id],
            )?;
            recompute_quality(conn)?;
        }
        Err(err) => {
            conn.execute(
                "UPDATE data_job SET status = 'failed', message = ?1, attempt = ?2, finished_at = ?3, updated_at = ?3 WHERE id = ?4",
                params![err.message, attempt, now_iso(), id],
            )?;
            tracing::error!(dataset, attempt, error = %err.message, "sync failed after retries");
        }
    }
    list_jobs(conn)
}

pub fn backfill(
    conn: &Connection,
    dataset: &str,
    start: &str,
    end: &str,
    operator: &str,
) -> Result<usize, AppError> {
    if operator.trim().is_empty() || start > end {
        return Err(AppError::bad_request(
            "operator and valid range are required",
        ));
    }
    let adapter = fixture_adapter()?;
    let changed = match dataset {
        "quote_daily" => {
            let bars = crate::adapter::MarketDataAdapter::bars(&adapter, "600519.SH")
                .map_err(|err| AppError::internal(err.to_string()))?;
            crate::ingest::sync_bars(conn, &bars, start, end)?
        }
        "financial_report" => {
            let reports = crate::adapter::MarketDataAdapter::financials(&adapter, "600519.SH")
                .map_err(|err| AppError::internal(err.to_string()))?;
            let selected: Vec<_> = reports
                .into_iter()
                .filter(|report| {
                    report.announce_date.as_str() >= start && report.announce_date.as_str() <= end
                })
                .collect();
            crate::ingest::sync_financials(conn, &selected)?
        }
        _ => {
            return Err(AppError::bad_request(
                "backfill supports quote_daily and financial_report",
            ))
        }
    };
    conn.execute(
        "INSERT INTO backfill_audit (dataset, start_date, end_date, changed, operator, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![dataset, start, end, changed as i64, operator, now_iso()],
    )?;
    Ok(changed)
}

fn sync_dataset(conn: &Connection, dataset: &str) -> Result<String, AppError> {
    if dataset == "market_snapshot" {
        crate::market::refresh(conn)?;
        return Ok("市场快照已更新，仅统计真实行情".into());
    }
    let adapter = fixture_adapter()?;
    let changed = match dataset {
        "instruments" => {
            let instruments = crate::adapter::MarketDataAdapter::instruments(&adapter)
                .map_err(|err| AppError::internal(err.to_string()))?;
            let count = crate::ingest::sync_instruments(conn, &instruments)?;
            crate::ingest::sync_calendar(conn, &adapter.calendar())?;
            count
        }
        "quote_daily" => {
            let bars = crate::adapter::MarketDataAdapter::bars(&adapter, "600519.SH")
                .map_err(|err| AppError::internal(err.to_string()))?;
            crate::ingest::sync_quote_incremental(
                conn,
                &bars,
                "0000-00-00",
                "9999-12-31",
                "600519.SH",
            )?
        }
        "financial_report" => {
            let reports = crate::adapter::MarketDataAdapter::financials(&adapter, "600519.SH")
                .map_err(|err| AppError::internal(err.to_string()))?;
            crate::ingest::sync_financials(conn, &reports)?
        }
        "market_snapshot" => {
            crate::market::refresh(conn)?;
            scalar(conn, "SELECT COUNT(*) FROM market_snapshot")? as usize
        }
        "daily_basic" | "index_quote" | "company_profile" => {
            let changed = crate::ingest::sync_reference_dataset(
                conn,
                crate::adapter::reference_fixture_json(),
                dataset,
            )?;
            crate::ingest::record_checkpoint(conn, "reference", dataset)?;
            changed
        }
        _ => return Err(AppError::bad_request("unknown dataset")),
    };
    let count: i64 = match dataset {
        "instruments" => scalar(conn, "SELECT COUNT(*) FROM instruments")?,
        "quote_daily" => scalar(conn, "SELECT COUNT(*) FROM quote_daily")?,
        "financial_report" => scalar(
            conn,
            "SELECT COUNT(*) FROM financial_report WHERE is_current = 1",
        )?,
        "market_snapshot" => scalar(conn, "SELECT COUNT(*) FROM market_snapshot")?,
        "daily_basic" | "index_quote" | "company_profile" => {
            scalar(conn, &format!("SELECT COUNT(*) FROM {dataset}"))?
        }
        _ => 0,
    };
    if count == 0 {
        return Err(AppError::internal(format!(
            "{dataset} has no rows; source unavailable"
        )));
    }
    Ok(format!(
        "同步完成：{dataset} 变更 {changed}，当前 {count} 行（mock adapter）"
    ))
}

fn fixture_adapter() -> Result<crate::adapter::MockAdapter, AppError> {
    if !cfg!(test) && std::env::var("ALPHAK_ALLOW_DEMO_SYNC").as_deref() != Ok("1") {
        return Err(AppError::bad_request(
            "演示同步已禁用；行情请使用公开源同步，未接入真实源的数据集暂不可同步",
        ));
    }
    crate::adapter::MockAdapter::embedded()
        .map_err(|error| AppError::internal(format!("adapter fixture: {error}")))
}

fn scalar(conn: &Connection, sql: &str) -> Result<i64, AppError> {
    Ok(conn.query_row(sql, [], |r| r.get(0))?)
}

pub fn recompute_quality(conn: &Connection) -> Result<Vec<QualityReport>, AppError> {
    let max_date: String = conn.query_row(
        "SELECT COALESCE(MAX(trade_date), '') FROM trading_calendar",
        [],
        |r| r.get(0),
    )?;
    let instruments: i64 = conn.query_row(
        "SELECT COUNT(*) FROM instruments WHERE status <> 'delisted' AND list_date <= ?1",
        params![max_date],
        |r| r.get(0),
    )?;
    let dates: i64 = scalar(
        conn,
        "SELECT COUNT(*) FROM trading_calendar WHERE is_open = 1",
    )?;
    let index_count: i64 = scalar(conn, "SELECT COUNT(*) FROM index_catalog")?;
    let now = now_iso();
    let quote_expected = instruments * dates;
    let quote_rows: i64 = scalar(conn, "SELECT COUNT(*) FROM quote_daily")?;
    let quote_anomalies: i64 = scalar(
        conn,
        "SELECT COUNT(*) FROM quote_daily WHERE close <= 0 OR high < low OR volume < 0",
    )?;
    let daily_expected = instruments * dates;
    let daily_rows: i64 = scalar(conn, "SELECT COUNT(*) FROM daily_basic")?;
    let daily_anomalies: i64 = scalar(
        conn,
        "SELECT COUNT(*) FROM daily_basic WHERE total_mv < 0 OR float_mv < 0",
    )?;
    let financial_rows: i64 = scalar(
        conn,
        "SELECT COUNT(*) FROM financial_report WHERE is_current = 1",
    )?;
    let financial_expected = instruments * 5;
    let profile_rows: i64 = scalar(conn, "SELECT COUNT(*) FROM company_profile")?;
    let index_expected = index_count * dates;
    let index_rows: i64 = scalar(conn, "SELECT COUNT(*) FROM index_quote")?;
    let sector_count: i64 = scalar(conn, "SELECT COUNT(*) FROM sector_catalog")?;
    let sector_with_members: i64 = scalar(
        conn,
        "SELECT COUNT(DISTINCT sector_id) FROM sector_member WHERE out_date IS NULL",
    )?;

    let reports = vec![
        ("company_profile", profile_rows, instruments, 0),
        ("daily_basic", daily_rows, daily_expected, daily_anomalies),
        ("financial_report", financial_rows, financial_expected, 0),
        ("index_quote", index_rows, index_expected, 0),
        ("instruments", instruments, instruments, 0),
        ("quote_daily", quote_rows, quote_expected, quote_anomalies),
        ("sector_catalog", sector_with_members, sector_count, 0),
    ];
    for (dataset, rows, expected, anomalies) in reports {
        let coverage = if expected > 0 {
            (rows as f64 / expected as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let missing = (expected - rows).max(0);
        conn.execute(
            "INSERT OR REPLACE INTO quality_report (dataset, coverage, missing, duplicates, anomalies, updated_at)
             VALUES (?1, ?2, ?3, 0, ?4, ?5)",
            params![dataset, coverage, missing, anomalies, now],
        )?;
    }
    let mut stmt = conn.prepare(
        "SELECT dataset, coverage, missing, duplicates, anomalies, updated_at FROM quality_report ORDER BY dataset",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(QualityReport {
            dataset: r.get(0)?,
            coverage: r.get(1)?,
            missing: r.get(2)?,
            duplicates: r.get(3)?,
            anomalies: r.get(4)?,
            updated_at: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_adapter_initializes_and_syncs_idempotently() {
        let state = crate::db::test_state();
        let conn = state.db.lock().unwrap();
        conn.execute("DELETE FROM financial_report", []).unwrap();
        conn.execute("DELETE FROM daily_basic", []).unwrap();
        conn.execute("DELETE FROM quote_daily", []).unwrap();
        conn.execute("DELETE FROM instruments", []).unwrap();
        conn.execute("DELETE FROM trading_calendar", []).unwrap();

        sync_dataset(&conn, "instruments").unwrap();
        sync_dataset(&conn, "quote_daily").unwrap();
        sync_dataset(&conn, "financial_report").unwrap();

        let instruments: i64 = conn
            .query_row("SELECT COUNT(*) FROM instruments", [], |r| r.get(0))
            .unwrap();
        let calendar: i64 = conn
            .query_row("SELECT COUNT(*) FROM trading_calendar", [], |r| r.get(0))
            .unwrap();
        let quotes: i64 = conn
            .query_row("SELECT COUNT(*) FROM quote_daily", [], |r| r.get(0))
            .unwrap();
        let reports: i64 = conn
            .query_row("SELECT COUNT(*) FROM financial_report", [], |r| r.get(0))
            .unwrap();
        assert_eq!(instruments, 2);
        assert_eq!(calendar, 3);
        assert_eq!(quotes, 3);
        assert_eq!(reports, 1);

        sync_dataset(&conn, "instruments").unwrap();
        sync_dataset(&conn, "quote_daily").unwrap();
        sync_dataset(&conn, "financial_report").unwrap();
        let quotes_again: i64 = conn
            .query_row("SELECT COUNT(*) FROM quote_daily", [], |r| r.get(0))
            .unwrap();
        let reports_again: i64 = conn
            .query_row("SELECT COUNT(*) FROM financial_report", [], |r| r.get(0))
            .unwrap();
        assert_eq!(quotes_again, quotes);
        assert_eq!(reports_again, reports);
    }

    #[test]
    fn backfill_is_scoped_and_audited() {
        let state = crate::db::test_state();
        let conn = state.db.lock().unwrap();
        conn.execute("UPDATE quote_daily SET close = 999 WHERE symbol = '600519.SH' AND trade_date = '2026-10-19'", []).unwrap();
        conn.execute(
            "INSERT INTO quote_staging (symbol, trade_date, open, high, low, close, volume, amount, adj_factor, status, reason, batch_id, ingested_at)
             VALUES ('600519.SH', '2026-10-19', 100, 101, 99, 100, 1, 1, 1, 'valid', '', 'old-batch', datetime('now'))",
            [],
        ).unwrap();
        let changed = backfill(&conn, "quote_daily", "2026-10-20", "2026-10-21", "tester").unwrap();
        assert_eq!(changed, 2);
        let outside: f64 = conn.query_row("SELECT close FROM quote_daily WHERE symbol = '600519.SH' AND trade_date = '2026-10-19'", [], |r| r.get(0)).unwrap();
        assert_eq!(outside, 999.0);
        let audit: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM backfill_audit WHERE operator = 'tester'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(audit, 1);
    }

    #[test]
    fn incremental_sync_fills_missing_day_and_records_checkpoint() {
        let state = crate::db::test_state();
        let conn = state.db.lock().unwrap();
        conn.execute(
            "DELETE FROM quote_daily WHERE symbol = '600519.SH' AND trade_date = '2026-10-21'",
            [],
        )
        .unwrap();
        sync_dataset(&conn, "quote_daily").unwrap();
        let restored: i64 = conn.query_row(
            "SELECT COUNT(*) FROM quote_daily WHERE symbol = '600519.SH' AND trade_date = '2026-10-21'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(restored, 1);
        let checkpoint: String = conn
            .query_row(
                "SELECT checkpoint FROM sync_checkpoint WHERE dataset = 'quote_daily'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!checkpoint.is_empty());
    }

    #[test]
    fn a_missing_trading_day_is_reported_as_missing() {
        let state = crate::db::test_state();
        let conn = state.db.lock().unwrap();
        conn.execute(
            "DELETE FROM quote_daily WHERE trade_date = '2026-10-21'",
            [],
        )
        .unwrap();
        let reports = recompute_quality(&conn).unwrap();
        let quote = reports.iter().find(|r| r.dataset == "quote_daily").unwrap();
        assert!(quote.coverage < 1.0);
        assert!(quote.missing > 0);
    }

    #[test]
    fn reference_datasets_do_not_overwrite_each_other() {
        let state = crate::db::test_state();
        let conn = state.db.lock().unwrap();
        let before: f64 = conn.query_row(
            "SELECT total_mv FROM daily_basic WHERE symbol = '600519.SH' AND trade_date = '2026-10-21'",
            [],
            |r| r.get(0),
        ).unwrap();
        sync_dataset(&conn, "company_profile").unwrap();
        let after_profile: f64 = conn.query_row(
            "SELECT total_mv FROM daily_basic WHERE symbol = '600519.SH' AND trade_date = '2026-10-21'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(before, after_profile);
        sync_dataset(&conn, "daily_basic").unwrap();
        let after_daily: f64 = conn.query_row(
            "SELECT total_mv FROM daily_basic WHERE symbol = '600519.SH' AND trade_date = '2026-10-21'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_ne!(before, after_daily);
    }

    #[test]
    fn reference_sync_records_checkpoint() {
        let state = crate::db::test_state();
        let conn = state.db.lock().unwrap();
        sync_dataset(&conn, "company_profile").unwrap();
        let checkpoint: String = conn
            .query_row(
                "SELECT checkpoint FROM sync_checkpoint WHERE dataset = 'reference'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(checkpoint, "company_profile");
    }

    #[test]
    fn reference_quality_covers_reference_datasets() {
        let conn = crate::db::init_db(":memory:").unwrap();
        let reports = recompute_quality(&conn).unwrap();
        for dataset in [
            "index_quote",
            "daily_basic",
            "company_profile",
            "sector_catalog",
        ] {
            let report = reports
                .iter()
                .find(|r| r.dataset == dataset)
                .expect(dataset);
            assert!(
                report.coverage >= 0.95,
                "{dataset} coverage {}",
                report.coverage
            );
            assert_eq!(report.missing, 0, "{dataset} missing");
        }
    }

    #[test]
    fn schedule_windows_are_weekday_only() {
        assert!(is_schedule_window(Weekday::Mon, 15, 10));
        assert!(is_schedule_window(Weekday::Fri, 18, 0));
        assert!(!is_schedule_window(Weekday::Sat, 15, 10));
        assert!(!is_schedule_window(Weekday::Sun, 18, 0));
        assert!(!is_schedule_window(Weekday::Mon, 15, 11));
        assert_eq!(scheduled_datasets(18), &["market_snapshot"]);
    }
}

#[cfg(test)]
mod retry_tests {
    use super::*;

    #[test]
    fn empty_source_is_retried_three_times_and_fails() {
        let state = crate::db::test_state();
        {
            let conn = state.db.lock().unwrap();
        }
        {
            let conn = state.db.lock().unwrap();
            let _ = run_internal(&conn, "quote_daily", true, None).unwrap();
        }
        let conn = state.db.lock().unwrap();
        let (status, attempt): (String, i64) = conn
            .query_row(
                "SELECT status, attempt FROM data_job WHERE dataset = 'quote_daily' ORDER BY started_at DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "failed");
        assert_eq!(attempt, 3);
    }
}
