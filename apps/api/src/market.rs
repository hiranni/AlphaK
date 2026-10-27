use chrono::{Datelike, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};

use crate::db::now_iso;
use crate::error::AppError;
use crate::models::{Breadth, IndexCard, MarketOverview, SectorRow};

struct QuoteRow {
    close: f64,
    amount: f64,
}

fn pct(current: f64, prev: f64) -> f64 {
    if prev.abs() < f64::EPSILON {
        0.0
    } else {
        (current / prev - 1.0) * 100.0
    }
}

fn two_quotes(conn: &Connection, symbol: &str, as_of: &str) -> Result<Vec<QuoteRow>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT close, amount FROM quote_daily WHERE symbol = ?1 AND source <> 'demo' AND trade_date <= ?2 ORDER BY trade_date DESC LIMIT 2",
    )?;
    let rows = stmt.query_map(params![symbol, as_of], |r| {
        Ok(QuoteRow {
            close: r.get(0)?,
            amount: r.get(1)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn limit_percent(board: &str, name: &str) -> f64 {
    if name.contains("ST") && !board.contains("创业板") && !board.contains("科创板") {
        return 5.0;
    }
    if board.contains("创业板") || board.contains("科创板") {
        return 20.0;
    }
    if board.contains("北") || board.contains("北交所") {
        return 30.0;
    }
    10.0
}

pub fn cached(conn: &Connection) -> Result<Option<(MarketOverview, String)>, AppError> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT payload, computed_at FROM market_snapshot ORDER BY trade_date DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((payload, computed_at)) = row else {
        return Ok(None);
    };
    let snapshot = match serde_json::from_str::<MarketOverview>(&payload) {
        Ok(snapshot) => snapshot,
        Err(_) => return Ok(None),
    };
    if snapshot.is_demo || snapshot.source == "demo-seed" || snapshot.source.contains("+demo") {
        return Ok(None);
    }
    if NaiveDate::parse_from_str(&snapshot.as_of, "%Y-%m-%d")
        .map(|date| date.weekday().number_from_monday() > 5)
        .unwrap_or(false)
    {
        return Ok(None);
    }
    Ok(Some((snapshot, computed_at)))
}

#[allow(dead_code)]
pub fn get(conn: &Connection, force: bool) -> Result<MarketOverview, AppError> {
    let as_of: String = conn
        .query_row("SELECT MAX(trade_date) FROM quote_daily", [], |r| {
            r.get::<_, Option<String>>(0)
        })?
        .unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string());

    if !force {
        let payload: Option<String> = conn
            .query_row(
                "SELECT payload FROM market_snapshot WHERE trade_date = ?1",
                params![as_of],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(payload) = payload {
            if let Ok(snapshot) = serde_json::from_str::<MarketOverview>(&payload) {
                return Ok(snapshot);
            }
        }
    }
    refresh(conn)
}

pub fn refresh(conn: &Connection) -> Result<MarketOverview, AppError> {
    let real_sources: Option<String> = conn.query_row(
        "SELECT GROUP_CONCAT(DISTINCT source) FROM (SELECT source FROM quote_daily WHERE source <> 'demo' UNION SELECT source FROM index_quote WHERE source <> 'demo')",
        [],
        |r| r.get(0),
    )?;
    let as_of: String = conn.query_row(
        "SELECT MAX(trade_date) FROM (SELECT trade_date FROM quote_daily WHERE source <> 'demo' UNION ALL SELECT trade_date FROM index_quote WHERE source <> 'demo') d WHERE strftime('%w', trade_date) NOT IN ('0','6') AND NOT EXISTS (SELECT 1 FROM trading_calendar c WHERE c.trade_date=d.trade_date AND c.is_open=0)",
        [], |r| r.get::<_, Option<String>>(0),
    )?.unwrap_or_default();

    let mut indices = Vec::new();
    let mut stmt = conn.prepare("SELECT c.code, c.name, q.close, q.amount, q.amount_known FROM index_catalog c JOIN index_quote q ON q.index_code = c.code WHERE q.trade_date = ?1 AND q.source <> 'demo' ORDER BY c.code")?;
    let cards = stmt.query_map(params![as_of], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, f64>(2)?,
            r.get::<_, f64>(3)?,
            r.get::<_, i64>(4)? != 0,
        ))
    })?;
    for card in cards {
        let (code, name, close, amount, amount_known) = card?;
        let prev: Option<f64> = conn
            .query_row(
                "SELECT close FROM index_quote WHERE index_code = ?1 AND source <> 'demo' AND trade_date < ?2 ORDER BY trade_date DESC LIMIT 1",
                params![code, as_of],
                |r| r.get(0),
            )
            .optional()?;
        indices.push(IndexCard {
            change_points: prev.map(|previous| close - previous),
            code,
            name,
            close,
            change_pct: pct(close, prev.unwrap_or(close)),
            amount,
            amount_known,
        });
    }

    let mut stmt = conn.prepare(
        "SELECT q.symbol, q.close, q.amount, i.board, i.name, q.amount_known
         FROM quote_daily q JOIN instruments i ON i.symbol = q.symbol
         WHERE q.trade_date = ?1 AND q.source <> 'demo'",
    )?;
    let rows = stmt.query_map(params![as_of], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, f64>(1)?,
            r.get::<_, f64>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, i64>(5)? != 0,
        ))
    })?;
    let (mut advance, mut decline, mut flat, mut limit_up, mut limit_down) = (0, 0, 0, 0, 0);
    let mut total_amount = 0.0;
    let mut amount_known = true;
    for row in rows {
        let (symbol, close, amount, board, name, row_amount_known) = row?;
        amount_known = amount_known && row_amount_known;
        let prev: Option<f64> = conn
            .query_row(
                "SELECT close FROM quote_daily WHERE symbol = ?1 AND source <> 'demo' AND trade_date < ?2 ORDER BY trade_date DESC LIMIT 1",
                params![symbol, as_of],
                |r| r.get(0),
            )
            .optional()?;
        total_amount += amount;
        let previous = prev.unwrap_or(close);
        let change = pct(close, previous);
        if change > 0.05 {
            advance += 1;
        } else if change < -0.05 {
            decline += 1;
        } else {
            flat += 1;
        }
        let pct_limit = limit_percent(&board, &name);
        let up_price = (previous * (1.0 + pct_limit / 100.0) * 100.0).round() / 100.0;
        let down_price = (previous * (1.0 - pct_limit / 100.0) * 100.0).round() / 100.0;
        if close >= up_price - 0.001 {
            limit_up += 1;
        }
        if close <= down_price + 0.001 {
            limit_down += 1;
        }
    }

    let mut sectors = Vec::new();
    let mut stmt = conn.prepare("SELECT id, name, sector_type FROM sector_catalog WHERE source NOT IN ('demo', 'demo-seed', 'mock') ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (id, name, sector_type) = row?;
        let mut member_stmt = conn.prepare("SELECT m.symbol, i.name FROM sector_member m JOIN instruments i ON i.symbol = m.symbol WHERE m.sector_id = ?1 AND m.in_date <= ?2 AND (m.out_date IS NULL OR m.out_date > ?2)")?;
        let members = member_stmt
            .query_map(params![id, as_of], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut sum = 0.0;
        let mut count = 0.0;
        let mut leading = (String::new(), String::new(), f64::MIN);
        let mut turnover = 0.0;
        for (symbol, stock_name) in members {
            let quotes = two_quotes(conn, &symbol, &as_of)?;
            if let (Some(latest), Some(prev)) = (quotes.first(), quotes.get(1)) {
                let change = pct(latest.close, prev.close);
                sum += change;
                count += 1.0;
                turnover += latest.amount;
                if change > leading.2 {
                    leading = (symbol, stock_name, change);
                }
            }
        }
        sectors.push(SectorRow {
            id,
            name,
            sector_type,
            change_pct: if count > 0.0 { sum / count } else { 0.0 },
            leading_symbol: leading.0,
            leading_name: leading.1,
            turnover,
        });
    }
    let sector_cache:Option<(Option<String>,String,i64,i64)>=conn.query_row("SELECT payload,source,fetched_at,failures FROM live_cache WHERE symbol='000001.SH' AND dataset='sectors'",[],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional()?;
    let (mut sector_source, mut sector_updated_at, mut sector_stale) = (String::new(), None, false);
    if let Some((Some(payload), source, updated, failures)) = sector_cache {
        if let Ok(rows) = serde_json::from_str::<Vec<SectorRow>>(&payload) {
            if !rows.is_empty() {
                sectors = rows;
                sector_source = source;
                sector_updated_at = Some(updated);
                sector_stale = failures > 0 || chrono::Utc::now().timestamp() - updated > 180;
            }
        }
    }
    sectors.sort_by(|a, b| {
        b.change_pct
            .partial_cmp(&a.change_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let today = Local::now().format("%Y-%m-%d").to_string();
    let is_trading_day: Option<i64> = conn
        .query_row(
            "SELECT is_open FROM trading_calendar WHERE trade_date = ?1",
            params![today],
            |r| r.get(0),
        )
        .optional()?;
    let snapshot = MarketOverview {
        sector_source,
        sector_updated_at,
        sector_stale,
        as_of: as_of.clone(),
        is_trading_day: is_trading_day.unwrap_or(0) == 1,
        indices,
        breadth: Breadth {
            advance,
            decline,
            flat,
            limit_up,
            limit_down,
            total_amount,
            amount_known,
        },
        sectors,
        source: real_sources.unwrap_or_else(|| "pending".into()),
        updated_at: now_iso(),
        is_demo: false,
    };
    let payload = serde_json::to_string(&snapshot)
        .map_err(|err| AppError::internal(format!("snapshot serialise: {err}")))?;
    conn.execute(
        "INSERT OR REPLACE INTO market_snapshot (trade_date, payload, computed_at) VALUES (?1, ?2, ?3)",
        params![as_of, payload, now_iso()],
    )?;
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekend_rows_do_not_hide_trading_day_samples() {
        let conn = crate::db::init_db(":memory:").unwrap();
        conn.execute("UPDATE quote_daily SET source='tencent'", [])
            .unwrap();
        conn.execute("UPDATE index_quote SET source='tencent'", [])
            .unwrap();
        let before = refresh(&conn).unwrap();
        conn.execute("UPDATE index_quote SET trade_date='2099-10-25' WHERE rowid=(SELECT MAX(rowid) FROM index_quote)", []).unwrap();
        // Explicitly closed dates are ignored even if an upstream mislabels a bar.
        conn.execute(
            "INSERT OR REPLACE INTO trading_calendar(trade_date,is_open) VALUES('2099-10-25',0)",
            [],
        )
        .unwrap();
        let after = refresh(&conn).unwrap();
        assert_eq!(before.as_of, after.as_of);
        assert_eq!(
            before.breadth.advance + before.breadth.decline + before.breadth.flat,
            after.breadth.advance + after.breadth.decline + after.breadth.flat
        );
    }

    #[test]
    fn limit_rules_follow_board() {
        assert_eq!(limit_percent("主板", "贵州茅台"), 10.0);
        assert_eq!(limit_percent("创业板", "宁德时代"), 20.0);
        assert_eq!(limit_percent("科创板", "科创示例"), 20.0);
        assert_eq!(limit_percent("主板", "ST示例"), 5.0);
        assert_eq!(limit_percent("北交所", "北交示例"), 30.0);
    }

    #[test]
    fn demo_seed_is_not_market_data() {
        let conn = crate::db::init_db(":memory:").unwrap();
        let snapshot = refresh(&conn).unwrap();
        assert!(snapshot.indices.is_empty());
        assert!(snapshot.sectors.is_empty());
        assert_eq!(
            snapshot.breadth.advance + snapshot.breadth.decline + snapshot.breadth.flat,
            0
        );
        assert!(snapshot.as_of.is_empty());
        assert_eq!(snapshot.source, "pending");
        assert!(!snapshot.is_demo);
    }

    #[test]
    fn mixed_database_only_aggregates_real_quotes() {
        let conn = crate::db::init_db(":memory:").unwrap();
        conn.execute(
            "UPDATE quote_daily SET source = 'tencent' WHERE symbol = '600519.SH'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE index_quote SET source = 'tencent' WHERE index_code = '000001.SH'",
            [],
        )
        .unwrap();
        let snapshot = refresh(&conn).unwrap();
        assert_eq!(snapshot.indices.len(), 1);
        assert_eq!(snapshot.indices[0].code, "000001.SH");
        let mut stmt = conn.prepare("SELECT close FROM index_quote WHERE index_code = '000001.SH' AND source <> 'demo' ORDER BY trade_date DESC LIMIT 2").unwrap();
        let closes = stmt
            .query_map([], |row| row.get::<_, f64>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            snapshot.indices[0].change_points,
            Some(closes[0] - closes[1])
        );
        conn.execute("DELETE FROM index_quote WHERE index_code = '000001.SH' AND trade_date < (SELECT MAX(trade_date) FROM index_quote WHERE index_code = '000001.SH')", []).unwrap();
        assert_eq!(refresh(&conn).unwrap().indices[0].change_points, None);
        assert_eq!(
            snapshot.breadth.advance + snapshot.breadth.decline + snapshot.breadth.flat,
            1
        );
        assert!(snapshot.sectors.is_empty());
        assert_eq!(snapshot.source, "tencent");
        assert!(!snapshot.is_demo);
    }

    #[test]
    fn cached_demo_snapshot_is_rejected() {
        let conn = crate::db::init_db(":memory:").unwrap();
        let mut snapshot = refresh(&conn).unwrap();
        snapshot.is_demo = true;
        snapshot.source = "tencent+demo".into();
        conn.execute(
            "UPDATE market_snapshot SET payload = ?1",
            params![serde_json::to_string(&snapshot).unwrap()],
        )
        .unwrap();
        assert!(cached(&conn).unwrap().is_none());
    }

    #[test]
    fn chinext_ten_percent_is_not_limit_up() {
        let conn = crate::db::init_db(":memory:").unwrap();
        conn.execute("UPDATE quote_daily SET source = 'tencent'", [])
            .unwrap();
        let latest: String = conn
            .query_row(
                "SELECT MAX(trade_date) FROM quote_daily WHERE symbol = '300750.SZ'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let prev: f64 = conn
            .query_row(
                "SELECT close FROM quote_daily WHERE symbol = '300750.SZ' AND trade_date < ?1 ORDER BY trade_date DESC LIMIT 1",
                params![latest],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "UPDATE quote_daily SET close = ?1 WHERE symbol = '300750.SZ' AND trade_date = ?2",
            params![prev * 1.10, latest],
        )
        .unwrap();
        let snapshot = refresh(&conn).unwrap();
        assert_eq!(snapshot.breadth.limit_up, 0);

        conn.execute(
            "UPDATE quote_daily SET close = ?1 WHERE symbol = '300750.SZ' AND trade_date = ?2",
            params![prev * 1.20, latest],
        )
        .unwrap();
        let snapshot = refresh(&conn).unwrap();
        assert_eq!(snapshot.breadth.limit_up, 1);
    }
}
