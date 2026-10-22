#![allow(dead_code)]

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};

use crate::adapter::BarDto;
use crate::error::AppError;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StageReport {
    pub accepted: usize,
    pub rejected: usize,
    pub promoted: usize,
}

fn validate(bar: &BarDto) -> Result<(), String> {
    if NaiveDate::parse_from_str(&bar.trade_date, "%Y-%m-%d").is_err() {
        return Err("invalid trade_date".into());
    }
    if bar.open <= 0.0 || bar.high <= 0.0 || bar.low <= 0.0 || bar.close <= 0.0 {
        return Err("price must be positive".into());
    }
    if bar.high < bar.low || bar.high < bar.open.max(bar.close) || bar.low > bar.open.min(bar.close)
    {
        return Err("OHLC range inconsistent".into());
    }
    if bar.volume < 0.0 || bar.amount < 0.0 {
        return Err("volume/amount must be non-negative".into());
    }
    if bar.adj_factor <= 0.0 {
        return Err("adj_factor must be positive".into());
    }
    Ok(())
}

pub fn stage_quotes(
    conn: &Connection,
    bars: &[BarDto],
    batch_id: &str,
) -> Result<StageReport, AppError> {
    let mut report = StageReport::default();
    for bar in bars {
        let (status, reason) = match validate(bar) {
            Ok(()) => {
                report.accepted += 1;
                ("valid", String::new())
            }
            Err(reason) => {
                report.rejected += 1;
                ("rejected", reason)
            }
        };
        conn.execute(
            "INSERT INTO quote_staging (symbol, trade_date, open, high, low, close, volume, amount, adj_factor, status, reason, batch_id, ingested_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, datetime('now'))
             ON CONFLICT(symbol, trade_date) DO UPDATE SET
                open = excluded.open, high = excluded.high, low = excluded.low,
                close = excluded.close, volume = excluded.volume, amount = excluded.amount,
                adj_factor = excluded.adj_factor, status = excluded.status,
                reason = excluded.reason, batch_id = excluded.batch_id, ingested_at = excluded.ingested_at",
            params![bar.symbol, bar.trade_date, bar.open, bar.high, bar.low, bar.close, bar.volume, bar.amount, bar.adj_factor, status, reason, batch_id],
        )?;
    }
    Ok(report)
}

pub fn promote_valid_quotes(conn: &Connection, batch_id: &str) -> Result<usize, AppError> {
    let tx = conn.unchecked_transaction()?;
    let promoted = tx.execute(
        "INSERT OR REPLACE INTO quote_daily (symbol, trade_date, open, high, low, close, volume, amount, adj_factor, turnover)
         SELECT symbol, trade_date, open, high, low, close, volume, amount, adj_factor, 0
         FROM quote_staging WHERE status = 'valid' AND batch_id = ?1",
        params![batch_id],
    )?;
    tx.execute(
        "DELETE FROM quote_staging WHERE status = 'valid' AND batch_id = ?1",
        params![batch_id],
    )?;
    tx.commit()?;
    Ok(promoted)
}

pub fn discard_batch(conn: &Connection, batch_id: &str) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM quote_staging WHERE batch_id = ?1",
        params![batch_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(trade_date: &str, close: f64, high: f64) -> BarDto {
        BarDto {
            symbol: "600519.SH".into(),
            trade_date: trade_date.into(),
            open: close,
            high,
            low: close - 1.0,
            close,
            volume: 1.0,
            amount: 1.0,
            adj_factor: 1.0,
        }
    }

    #[test]
    fn instrument_sync_preserves_pinyin_search() {
        let conn = crate::db::init_db(":memory:").unwrap();
        let adapter = crate::adapter::MockAdapter::embedded().unwrap();
        let instruments = crate::adapter::MarketDataAdapter::instruments(&adapter).unwrap();
        sync_instruments(&conn, &instruments).unwrap();
        let pinyin: String = conn
            .query_row(
                "SELECT pinyin FROM instruments WHERE symbol = '600519.SH'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pinyin, "gzmt");
        let hits: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM instruments WHERE lower(pinyin) LIKE '%gzmt%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1);
    }

    #[test]
    fn reprocessing_replaces_the_payload_that_was_validated() {
        let conn = crate::db::init_db(":memory:").unwrap();
        let bad = bar("2026-10-22", 1500.0, 1490.0);
        assert_eq!(stage_quotes(&conn, &[bad], "b1").unwrap().rejected, 1);
        let good = bar("2026-10-22", 1500.0, 1510.0);
        assert_eq!(stage_quotes(&conn, &[good], "b1").unwrap().accepted, 1);
        promote_valid_quotes(&conn, "b1").unwrap();
        let high: f64 = conn
            .query_row(
                "SELECT high FROM quote_daily WHERE symbol = '600519.SH' AND trade_date = '2026-10-22'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(high, 1510.0);
    }

    #[test]
    fn valid_payload_can_be_corrected_then_rejected() {
        let conn = crate::db::init_db(":memory:").unwrap();
        let first = bar("2026-10-23", 1500.0, 1510.0);
        assert_eq!(stage_quotes(&conn, &[first], "b2").unwrap().accepted, 1);
        let corrected = bar("2026-10-23", 1505.0, 1510.0);
        assert_eq!(stage_quotes(&conn, &[corrected], "b2").unwrap().accepted, 1);
        promote_valid_quotes(&conn, "b2").unwrap();
        let close: f64 = conn
            .query_row(
                "SELECT close FROM quote_daily WHERE symbol = '600519.SH' AND trade_date = '2026-10-23'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(close, 1505.0);

        let rejected = bar("2026-10-24", 1500.0, 1490.0);
        assert_eq!(stage_quotes(&conn, &[rejected], "b3").unwrap().rejected, 1);
        let promoted = promote_valid_quotes(&conn, "b3").unwrap();
        assert_eq!(promoted, 0);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM quote_daily WHERE symbol = '600519.SH' AND trade_date = '2026-10-24'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn late_lower_financial_version_does_not_become_current() {
        let conn = crate::db::init_db(":memory:").unwrap();
        conn.execute(
            "DELETE FROM financial_report WHERE symbol = '600519.SH' AND report_period = '2026-06-30' AND version = 1",
            [],
        ).unwrap();
        let adapter = crate::adapter::MockAdapter::embedded().unwrap();
        let mut reports =
            crate::adapter::MarketDataAdapter::financials(&adapter, "600519.SH").unwrap();
        sync_financials(&conn, &reports).unwrap();
        let current: (i64, i64) = conn.query_row(
            "SELECT COUNT(*), MAX(version) FROM financial_report WHERE symbol = '600519.SH' AND report_period = '2026-06-30' AND is_current = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        assert_eq!(current, (1, 2));

        reports[0].revenue += 1.0;
        assert!(sync_financials(&conn, &reports).is_err());
    }

    #[test]
    fn bad_rows_stay_out_of_quote_daily() {
        let conn = crate::db::init_db(":memory:").unwrap();
        let bars = vec![
            bar("2026-10-22", 1500.0, 1510.0),
            bar("2026-10-23", 1500.0, 1490.0),
        ];
        let report = stage_quotes(&conn, &bars, "b4").unwrap();
        assert_eq!(report.accepted, 1);
        assert_eq!(report.rejected, 1);
        let promoted = promote_valid_quotes(&conn, "b4").unwrap();
        assert_eq!(promoted, 1);
        let good: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM quote_daily WHERE symbol = '600519.SH' AND trade_date = '2026-10-22'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let bad: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM quote_daily WHERE trade_date = '2026-10-23'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(good, 1);
        assert_eq!(bad, 0);
    }
}

use crate::adapter::{FinancialDto, InstrumentDto};

pub fn sync_instruments(
    conn: &Connection,
    instruments: &[InstrumentDto],
) -> Result<usize, AppError> {
    let mut count = 0;
    for item in instruments {
        let pinyin = if item.pinyin.trim().is_empty() {
            item.symbol.to_lowercase()
        } else {
            item.pinyin.trim().to_lowercase()
        };
        conn.execute(
            "INSERT INTO instruments (symbol, name, pinyin, exchange, board, industry, list_date, delist_date, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8)
             ON CONFLICT(symbol) DO UPDATE SET name = excluded.name, pinyin = excluded.pinyin,
                 exchange = excluded.exchange, board = excluded.board, industry = excluded.industry,
                 list_date = excluded.list_date, status = excluded.status",
            params![item.symbol, item.name, pinyin, item.exchange, item.board, item.industry, item.list_date, item.status],
        )?;
        count += 1;
    }
    Ok(count)
}

pub fn sync_calendar(conn: &Connection, dates: &[String]) -> Result<usize, AppError> {
    let mut count = 0;
    for date in dates {
        conn.execute(
            "INSERT OR REPLACE INTO trading_calendar (trade_date, is_open) VALUES (?1, 1)",
            params![date],
        )?;
        count += 1;
    }
    Ok(count)
}

pub fn sync_bars(
    conn: &Connection,
    bars: &[BarDto],
    start: &str,
    end: &str,
) -> Result<usize, AppError> {
    let batch_id = format!("batch-{}", uuid::Uuid::new_v4());
    let selected: Vec<BarDto> = bars
        .iter()
        .filter(|bar| bar.trade_date.as_str() >= start && bar.trade_date.as_str() <= end)
        .cloned()
        .collect();
    let report = stage_quotes(conn, &selected, &batch_id)?;
    if report.rejected > 0 {
        discard_batch(conn, &batch_id)?;
        return Err(AppError::bad_request(format!(
            "{} bars rejected by quality gate",
            report.rejected
        )));
    }
    promote_valid_quotes(conn, &batch_id)
}

pub fn sync_financials(conn: &Connection, reports: &[FinancialDto]) -> Result<usize, AppError> {
    let tx = conn.unchecked_transaction()?;
    let mut count = 0;
    for report in reports {
        let existing: Option<(String, f64, f64, f64, f64, f64, f64)> = tx
            .query_row(
                "SELECT announce_date, revenue, net_profit, total_assets, total_liabilities, equity, operating_cash_flow
                 FROM financial_report WHERE symbol = ?1 AND report_period = ?2 AND statement_type = ?3 AND version = ?4",
                params![report.symbol, report.report_period, report.statement_type, report.version],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
            )
            .optional()?;
        if let Some(existing) = existing {
            let same = existing.0 == report.announce_date
                && (existing.1 - report.revenue).abs() < 1e-6
                && (existing.2 - report.net_profit).abs() < 1e-6
                && (existing.3 - report.total_assets).abs() < 1e-6
                && (existing.4 - report.total_liabilities).abs() < 1e-6
                && (existing.5 - report.equity).abs() < 1e-6
                && (existing.6 - report.operating_cash_flow).abs() < 1e-6;
            if !same {
                return Err(AppError::bad_request(format!(
                    "financial version conflict: {} {} v{}",
                    report.symbol, report.report_period, report.version
                )));
            }
            continue;
        }
        tx.execute(
            "INSERT INTO financial_report
             (symbol, report_period, announce_date, statement_type, version, effective_at, supersedes, payload_hash, is_current,
              revenue, net_profit, total_assets, total_liabilities, equity, operating_cash_flow, collected_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?3, NULL, ?6, 0, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                report.symbol,
                report.report_period,
                report.announce_date,
                report.statement_type,
                report.version,
                format!("sync-{}-{}-v{}", report.symbol, report.report_period, report.version),
                report.revenue,
                report.net_profit,
                report.total_assets,
                report.total_liabilities,
                report.equity,
                report.operating_cash_flow,
                crate::db::now_iso(),
            ],
        )?;
        tx.execute(
            "UPDATE financial_report SET is_current = 0
             WHERE symbol = ?1 AND report_period = ?2 AND statement_type = ?3",
            params![report.symbol, report.report_period, report.statement_type],
        )?;
        tx.execute(
            "UPDATE financial_report SET is_current = 1
             WHERE symbol = ?1 AND report_period = ?2 AND statement_type = ?3
               AND version = (SELECT MAX(version) FROM financial_report WHERE symbol = ?1 AND report_period = ?2 AND statement_type = ?3)",
            params![report.symbol, report.report_period, report.statement_type],
        )?;
        count += 1;
    }
    tx.commit()?;
    Ok(count)
}

pub fn sync_reference(conn: &Connection, json: &str) -> Result<usize, AppError> {
    sync_reference_dataset(conn, json, "all")
}

pub fn sync_reference_dataset(
    conn: &Connection,
    json: &str,
    dataset: &str,
) -> Result<usize, AppError> {
    let fixture: serde_json::Value =
        serde_json::from_str(json).map_err(|err| AppError::invalid_data(err.to_string()))?;
    let include = |name: &str| dataset == "all" || dataset == name;
    let mut changed = 0;
    if include("index_catalog") {
        if let Some(rows) = fixture["index_catalog"].as_array() {
            for row in rows {
                conn.execute(
                "INSERT OR REPLACE INTO index_catalog (code, name, market, category, status) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![row["code"].as_str(), row["name"].as_str(), row["market"].as_str(), row["category"].as_str(), row["status"].as_str()],
            )?;
                changed += 1;
            }
        }
    }
    if include("index_quote") {
        if let Some(rows) = fixture["index_quote"].as_array() {
            for row in rows {
                let close = row["close"].as_f64().unwrap_or(0.0);
                conn.execute(
                "INSERT OR REPLACE INTO index_quote (index_code, trade_date, open, high, low, close, volume, amount) VALUES (?1, ?2, ?3, ?3, ?3, ?3, 0, ?4)",
                params![row["index_code"].as_str(), row["trade_date"].as_str(), close, row["amount"].as_f64().unwrap_or(0.0)],
            )?;
                changed += 1;
            }
        }
    }
    if include("index_member") {
        if let Some(rows) = fixture["index_member"].as_array() {
            for row in rows {
                conn.execute(
                "INSERT OR REPLACE INTO index_member (index_code, symbol, in_date, out_date, weight) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![row["index_code"].as_str(), row["symbol"].as_str(), row["in_date"].as_str(), row["out_date"].as_str(), row["weight"].as_f64()],
            )?;
                changed += 1;
            }
        }
    }
    if include("sector_catalog") {
        if let Some(rows) = fixture["sector_catalog"].as_array() {
            for row in rows {
                conn.execute(
                "INSERT OR IGNORE INTO sector_catalog (name, sector_type, source, status) VALUES (?1, ?2, ?3, ?4)",
                params![row["name"].as_str(), row["sector_type"].as_str(), row["source"].as_str(), row["status"].as_str()],
            )?;
                changed += 1;
            }
        }
    }
    if include("sector_member") {
        if let Some(rows) = fixture["sector_member"].as_array() {
            for row in rows {
                let sector_id: i64 = conn.query_row(
                "SELECT id FROM sector_catalog WHERE name = (SELECT name FROM sector_catalog ORDER BY id LIMIT 1)",
                [],
                |r| r.get(0),
            )?;
                conn.execute(
                "INSERT OR REPLACE INTO sector_member (sector_id, symbol, in_date, out_date, weight) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![sector_id, row["symbol"].as_str(), row["in_date"].as_str(), row["out_date"].as_str(), row["weight"].as_f64()],
            )?;
                changed += 1;
            }
        }
    }
    if include("daily_basic") {
        if let Some(rows) = fixture["daily_basic"].as_array() {
            for row in rows {
                conn.execute(
                "INSERT OR REPLACE INTO daily_basic (symbol, trade_date, total_shares, float_shares, total_mv, float_mv, pe_ttm, pb, turnover) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![row["symbol"].as_str(), row["trade_date"].as_str(), row["total_shares"].as_f64(), row["float_shares"].as_f64(), row["total_mv"].as_f64(), row["float_mv"].as_f64(), row["pe_ttm"].as_f64(), row["pb"].as_f64(), row["turnover"].as_f64()],
            )?;
                changed += 1;
            }
        }
    }
    if include("company_profile") {
        if let Some(rows) = fixture["company_profile"].as_array() {
            for row in rows {
                conn.execute(
                "INSERT OR IGNORE INTO company_profile (symbol, full_name, business, industry, list_date, controller, website, region, intro, source, updated_at) VALUES (?1, ?2, '', ?3, '', '', '', '', '', ?4, ?5)",
                params![row["symbol"].as_str(), row["full_name"].as_str(), row["industry"].as_str(), row["source"].as_str(), row["updated_at"].as_str()],
            )?;
                changed += 1;
            }
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod reference_tests {
    use super::*;

    #[test]
    fn reference_fixture_is_idempotent() {
        let conn = crate::db::init_db(":memory:").unwrap();
        let first = sync_reference(&conn, crate::adapter::reference_fixture_json()).unwrap();
        assert!(first > 0);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM index_catalog", [], |r| r.get(0))
            .unwrap();
        let _ = sync_reference(&conn, crate::adapter::reference_fixture_json()).unwrap();
        let count_again: i64 = conn
            .query_row("SELECT COUNT(*) FROM index_catalog", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, count_again);
    }
}

pub fn sync_quote_incremental(
    conn: &Connection,
    bars: &[BarDto],
    start: &str,
    end: &str,
    symbol: &str,
) -> Result<usize, AppError> {
    let mut stmt = conn.prepare(
        "SELECT trade_date FROM trading_calendar WHERE is_open = 1 AND trade_date BETWEEN ?1 AND ?2 ORDER BY trade_date LIMIT 500",
    )?;
    let dates = stmt
        .query_map(params![start, end], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut missing = Vec::new();
    for date in dates {
        let exists: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM quote_daily WHERE symbol = ?1 AND trade_date = ?2",
                params![symbol, date],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_none() {
            missing.push(date);
        }
    }
    let selected: Vec<BarDto> = bars
        .iter()
        .filter(|bar| missing.iter().any(|date| date == &bar.trade_date))
        .cloned()
        .collect();
    let batch_id = format!("batch-{}", uuid::Uuid::new_v4());
    let report = stage_quotes(conn, &selected, &batch_id)?;
    if report.rejected > 0 {
        discard_batch(conn, &batch_id)?;
        return Err(AppError::bad_request("rejected bars in incremental sync"));
    }
    let changed = promote_valid_quotes(conn, &batch_id)?;
    conn.execute(
        "INSERT OR REPLACE INTO sync_checkpoint (dataset, checkpoint, updated_at) VALUES ('quote_daily', ?1, ?2)",
        params![end, crate::db::now_iso()],
    )?;
    Ok(changed)
}

pub fn record_checkpoint(
    conn: &Connection,
    dataset: &str,
    checkpoint: &str,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT OR REPLACE INTO sync_checkpoint (dataset, checkpoint, updated_at) VALUES (?1, ?2, ?3)",
        params![dataset, checkpoint, crate::db::now_iso()],
    )?;
    Ok(())
}
