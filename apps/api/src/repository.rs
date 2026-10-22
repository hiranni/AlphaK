#![allow(dead_code)]

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::AppError;
use crate::models::{FinancialReport, Instrument};

pub fn instrument(conn: &Connection, symbol: &str) -> Result<Instrument, AppError> {
    conn.query_row(
        "SELECT symbol, name, pinyin, exchange, board, industry, list_date, status FROM instruments WHERE symbol = ?1",
        params![symbol],
        |r| {
            Ok(Instrument {
                symbol: r.get(0)?,
                name: r.get(1)?,
                pinyin: r.get(2)?,
                exchange: r.get(3)?,
                board: r.get(4)?,
                industry: r.get(5)?,
                list_date: r.get(6)?,
                status: r.get(7)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| AppError::not_found(format!("证券不存在: {symbol}")))
}

pub fn quotes_between(
    conn: &Connection,
    symbol: &str,
    start: &str,
    end: &str,
) -> Result<Vec<(String, f64)>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT trade_date, close FROM quote_daily WHERE symbol = ?1 AND trade_date BETWEEN ?2 AND ?3 ORDER BY trade_date",
    )?;
    let rows = stmt.query_map(params![symbol, start, end], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn latest_quote(conn: &Connection, symbol: &str) -> Result<Option<(String, f64)>, AppError> {
    Ok(conn
        .query_row(
            "SELECT trade_date, close FROM quote_daily WHERE symbol = ?1 ORDER BY trade_date DESC LIMIT 1",
            params![symbol],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}

fn map_report(r: &rusqlite::Row<'_>) -> rusqlite::Result<FinancialReport> {
    Ok(FinancialReport {
        report_period: r.get(0)?,
        announce_date: r.get(1)?,
        statement_type: r.get(2)?,
        version: r.get(3)?,
        effective_at: r.get(4)?,
        supersedes: r.get(5)?,
        is_current: r.get::<_, i64>(6)? == 1,
        revenue: r.get(7)?,
        net_profit: r.get(8)?,
        total_assets: r.get(9)?,
        total_liabilities: r.get(10)?,
        equity: r.get(11)?,
        operating_cash_flow: r.get(12)?,
    })
}

pub fn correct_company_profile(
    conn: &Connection,
    symbol: &str,
    field: &str,
    value: &str,
    operator: &str,
) -> Result<(), AppError> {
    let column = match field {
        "business" | "controller" | "region" | "website" | "intro" | "full_name" => field,
        _ => return Err(AppError::bad_request("unsupported profile field")),
    };
    let old: String = conn
        .query_row(
            &format!("SELECT {column} FROM company_profile WHERE symbol = ?1"),
            params![symbol],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("暂无公司资料"))?;
    conn.execute(
        &format!("UPDATE company_profile SET {column} = ?1, updated_at = ?2 WHERE symbol = ?3"),
        params![value, crate::db::now_iso(), symbol],
    )?;
    conn.execute(
        "INSERT INTO reference_correction (dataset, record_key, field, old_value, new_value, operator, corrected_at)
         VALUES ('company_profile', ?1, ?2, ?3, ?4, ?5, ?6)",
        params![symbol, column, old, value, operator, crate::db::now_iso()],
    )?;
    Ok(())
}

pub fn current_reports(conn: &Connection, symbol: &str) -> Result<Vec<FinancialReport>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT report_period, announce_date, statement_type, version, effective_at, supersedes,
                is_current, revenue, net_profit, total_assets, total_liabilities, equity, operating_cash_flow
         FROM financial_report WHERE symbol = ?1 AND is_current = 1 ORDER BY report_period DESC",
    )?;
    let rows = stmt.query_map(params![symbol], map_report)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn reports_as_of(
    conn: &Connection,
    symbol: &str,
    as_of: &str,
) -> Result<Vec<FinancialReport>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT f.report_period, f.announce_date, f.statement_type, f.version, f.effective_at,
                f.supersedes, f.is_current, f.revenue, f.net_profit, f.total_assets,
                f.total_liabilities, f.equity, f.operating_cash_flow
         FROM financial_report f
         WHERE f.symbol = ?1 AND f.announce_date <= ?2
           AND f.version = (
             SELECT MAX(f2.version) FROM financial_report f2
             WHERE f2.symbol = f.symbol AND f2.report_period = f.report_period
               AND f2.statement_type = f.statement_type AND f2.announce_date <= ?2
           )
         ORDER BY f.report_period DESC",
    )?;
    let rows = stmt.query_map(params![symbol, as_of], map_report)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_by_symbol_range_and_report_period() {
        let conn = crate::db::init_db(":memory:").unwrap();
        assert_eq!(instrument(&conn, "600519.SH").unwrap().name, "贵州茅台");
        let quotes = quotes_between(&conn, "600519.SH", "2026-10-01", "2026-10-31").unwrap();
        assert!(!quotes.is_empty());
        assert!(latest_quote(&conn, "600519.SH").unwrap().is_some());
        let current = current_reports(&conn, "600519.SH").unwrap();
        assert!(current.iter().any(|r| r.version == 2));
        let historical = reports_as_of(&conn, "600519.SH", "2026-08-01").unwrap();
        assert!(historical.iter().all(|r| r.version == 1));
        assert!(instrument(&conn, "999999.SH").is_err());
    }
}
