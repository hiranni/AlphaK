use std::sync::{Arc, Mutex};

use chrono::Utc;
use rusqlite::Connection;

use crate::seed;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub sources: Arc<crate::sources::SourceHub>,
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

pub fn init_db(path: &str) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS instruments (
            symbol TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            pinyin TEXT NOT NULL,
            exchange TEXT NOT NULL,
            board TEXT NOT NULL,
            industry TEXT NOT NULL,
            list_date TEXT NOT NULL,
            delist_date TEXT,
            status TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS trading_calendar (
            trade_date TEXT PRIMARY KEY,
            is_open INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS quote_daily (
            symbol TEXT NOT NULL,
            trade_date TEXT NOT NULL,
            open REAL NOT NULL,
            high REAL NOT NULL,
            low REAL NOT NULL,
            close REAL NOT NULL,
            volume REAL NOT NULL,
            amount REAL NOT NULL,
            adj_factor REAL NOT NULL DEFAULT 1.0,
            turnover REAL NOT NULL DEFAULT 0,
            PRIMARY KEY (symbol, trade_date)
        );
        CREATE INDEX IF NOT EXISTS idx_quote_date ON quote_daily(trade_date);

        CREATE TABLE IF NOT EXISTS financial_report (
            symbol TEXT NOT NULL,
            report_period TEXT NOT NULL,
            announce_date TEXT NOT NULL,
            statement_type TEXT NOT NULL,
            version INTEGER NOT NULL DEFAULT 1,
            effective_at TEXT NOT NULL,
            supersedes INTEGER,
            payload_hash TEXT NOT NULL,
            is_current INTEGER NOT NULL DEFAULT 1,
            revenue REAL NOT NULL,
            net_profit REAL NOT NULL,
            total_assets REAL NOT NULL,
            total_liabilities REAL NOT NULL,
            equity REAL NOT NULL,
            operating_cash_flow REAL NOT NULL,
            collected_at TEXT NOT NULL,
            PRIMARY KEY (symbol, report_period, statement_type, version)
        );

        CREATE TABLE IF NOT EXISTS financial_indicator (
            symbol TEXT NOT NULL,
            report_period TEXT NOT NULL,
            roe REAL NOT NULL,
            gross_margin REAL NOT NULL,
            net_margin REAL NOT NULL,
            debt_ratio REAL NOT NULL,
            eps REAL NOT NULL,
            bps REAL NOT NULL,
            PRIMARY KEY (symbol, report_period)
        );

        CREATE TABLE IF NOT EXISTS quote_staging (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            symbol TEXT NOT NULL,
            trade_date TEXT NOT NULL,
            open REAL NOT NULL,
            high REAL NOT NULL,
            low REAL NOT NULL,
            close REAL NOT NULL,
            volume REAL NOT NULL,
            amount REAL NOT NULL,
            adj_factor REAL NOT NULL,
            status TEXT NOT NULL,
            reason TEXT NOT NULL DEFAULT '',
            batch_id TEXT NOT NULL DEFAULT '',
            ingested_at TEXT NOT NULL,
            UNIQUE(symbol, trade_date)
        );

        CREATE TABLE IF NOT EXISTS daily_basic (
            symbol TEXT NOT NULL,
            trade_date TEXT NOT NULL,
            total_shares REAL NOT NULL,
            float_shares REAL NOT NULL,
            total_mv REAL NOT NULL,
            float_mv REAL NOT NULL,
            pe_ttm REAL,
            pb REAL,
            turnover REAL NOT NULL,
            PRIMARY KEY (symbol, trade_date)
        );

        CREATE TABLE IF NOT EXISTS company_profile (
            symbol TEXT PRIMARY KEY,
            full_name TEXT NOT NULL,
            business TEXT NOT NULL,
            industry TEXT NOT NULL,
            list_date TEXT NOT NULL,
            controller TEXT NOT NULL,
            website TEXT NOT NULL,
            region TEXT NOT NULL,
            intro TEXT NOT NULL,
            source TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS index_catalog (
            code TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            market TEXT NOT NULL,
            category TEXT NOT NULL,
            status TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS index_quote (
            index_code TEXT NOT NULL,
            trade_date TEXT NOT NULL,
            open REAL NOT NULL,
            high REAL NOT NULL,
            low REAL NOT NULL,
            close REAL NOT NULL,
            volume REAL NOT NULL,
            amount REAL NOT NULL,
            PRIMARY KEY (index_code, trade_date)
        );

        CREATE TABLE IF NOT EXISTS index_member (
            index_code TEXT NOT NULL,
            symbol TEXT NOT NULL,
            in_date TEXT NOT NULL,
            out_date TEXT,
            weight REAL NOT NULL,
            PRIMARY KEY (index_code, symbol, in_date)
        );

        CREATE TABLE IF NOT EXISTS sector_catalog (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            sector_type TEXT NOT NULL,
            source TEXT NOT NULL,
            status TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sector_member (
            sector_id INTEGER NOT NULL,
            symbol TEXT NOT NULL,
            in_date TEXT NOT NULL,
            out_date TEXT,
            weight REAL NOT NULL,
            PRIMARY KEY (sector_id, symbol, in_date)
        );

        CREATE TABLE IF NOT EXISTS watchlist_group (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            sort INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS watchlist_item (
            id TEXT PRIMARY KEY,
            group_id TEXT NOT NULL,
            symbol TEXT NOT NULL,
            note TEXT NOT NULL DEFAULT '',
            sort INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            UNIQUE(group_id, symbol),
            FOREIGN KEY(group_id) REFERENCES watchlist_group(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS app_setting (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS data_job (
            id TEXT PRIMARY KEY,
            dataset TEXT NOT NULL,
            status TEXT NOT NULL,
            progress INTEGER NOT NULL,
            message TEXT NOT NULL,
            started_at TEXT NOT NULL,
            finished_at TEXT,
            idempotency_key TEXT,
            attempt INTEGER NOT NULL DEFAULT 1,
            updated_at TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS market_snapshot (
            trade_date TEXT PRIMARY KEY,
            payload TEXT NOT NULL,
            computed_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS quote_snapshot (
            symbol TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            trade_date TEXT NOT NULL,
            open REAL NOT NULL,
            high REAL NOT NULL,
            low REAL NOT NULL,
            close REAL NOT NULL,
            prev_close REAL NOT NULL,
            volume REAL NOT NULL,
            amount REAL NOT NULL,
            change_pct REAL NOT NULL,
            source TEXT NOT NULL,
            batch_id TEXT NOT NULL,
            fetched_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS live_cache (
            symbol TEXT NOT NULL,
            dataset TEXT NOT NULL,
            payload TEXT,
            source TEXT NOT NULL DEFAULT '',
            fetched_at INTEGER NOT NULL DEFAULT 0,
            attempted_at INTEGER NOT NULL DEFAULT 0,
            failures INTEGER NOT NULL DEFAULT 0,
            error TEXT NOT NULL DEFAULT '',
            PRIMARY KEY(symbol, dataset)
        );

        CREATE TABLE IF NOT EXISTS source_retry_task (
            dataset TEXT NOT NULL,
            symbol TEXT NOT NULL,
            start_date TEXT NOT NULL,
            end_date TEXT NOT NULL,
            attempts INTEGER NOT NULL DEFAULT 0,
            last_error TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (dataset, symbol, start_date, end_date)
        );

        CREATE TABLE IF NOT EXISTS data_freshness (
            dataset TEXT PRIMARY KEY,
            status TEXT NOT NULL,
            last_success_at TEXT,
            last_source TEXT,
            last_batch_id TEXT,
            last_error TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS data_source_config (
            id TEXT PRIMARY KEY,
            provider TEXT NOT NULL,
            enabled INTEGER NOT NULL,
            base_url TEXT NOT NULL,
            secret_env TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sync_checkpoint (
            dataset TEXT PRIMARY KEY,
            checkpoint TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS backfill_audit (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            dataset TEXT NOT NULL,
            start_date TEXT NOT NULL,
            end_date TEXT NOT NULL,
            changed INTEGER NOT NULL,
            operator TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS reference_correction (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            dataset TEXT NOT NULL,
            record_key TEXT NOT NULL,
            field TEXT NOT NULL,
            old_value TEXT NOT NULL,
            new_value TEXT NOT NULL,
            operator TEXT NOT NULL,
            corrected_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS quality_report (
            dataset TEXT PRIMARY KEY,
            coverage REAL NOT NULL,
            missing INTEGER NOT NULL,
            duplicates INTEGER NOT NULL,
            anomalies INTEGER NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
    )?;

    migrate_financial_report(&conn)?;
    migrate_data_job(&conn)?;
    migrate_quote_staging(&conn)?;
    migrate_source_columns(&conn)?;

    let count: i64 = conn.query_row("SELECT COUNT(*) FROM instruments", [], |r| r.get(0))?;
    if count == 0 {
        seed::seed(&conn)?;
    }
    conn.execute_batch(
        "DROP INDEX IF EXISTS idx_financial_current;
         CREATE UNIQUE INDEX idx_financial_current ON financial_report(symbol, report_period, statement_type) WHERE is_current = 1;",
    )?;
    crate::seed::purge_holiday_data(&conn)?;
    conn.execute(
        "INSERT OR IGNORE INTO data_source_config (id, provider, enabled, base_url, secret_env, updated_at) VALUES ('akshare', 'akshare', 1, '', 'ALPHAK_AKSHARE_TOKEN', ?1)",
        rusqlite::params![now_iso()],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO data_source_config (id, provider, enabled, base_url, secret_env, updated_at) VALUES ('mock', 'mock', 1, '', '', ?1)",
        rusqlite::params![now_iso()],
    )?;
    Ok(conn)
}

fn migrate_financial_report(conn: &Connection) -> rusqlite::Result<()> {
    let has_version = conn
        .prepare("PRAGMA table_info(financial_report)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .iter()
        .any(|column| column == "version");
    if has_version {
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_financial_current ON financial_report(symbol, is_current)",
            [],
        )?;
        return Ok(());
    }

    // Older demo databases used (symbol, report_period, statement_type) as PK.
    // Rebuild once so a correction can be stored as a new immutable version.
    conn.execute_batch(
        r#"
        BEGIN;
        ALTER TABLE financial_report RENAME TO financial_report_legacy;
        CREATE TABLE financial_report (
            symbol TEXT NOT NULL,
            report_period TEXT NOT NULL,
            announce_date TEXT NOT NULL,
            statement_type TEXT NOT NULL,
            version INTEGER NOT NULL DEFAULT 1,
            effective_at TEXT NOT NULL,
            supersedes INTEGER,
            payload_hash TEXT NOT NULL,
            is_current INTEGER NOT NULL DEFAULT 1,
            revenue REAL NOT NULL,
            net_profit REAL NOT NULL,
            total_assets REAL NOT NULL,
            total_liabilities REAL NOT NULL,
            equity REAL NOT NULL,
            operating_cash_flow REAL NOT NULL,
            collected_at TEXT NOT NULL,
            PRIMARY KEY (symbol, report_period, statement_type, version)
        );
        INSERT INTO financial_report
            (symbol, report_period, announce_date, statement_type, version, effective_at,
             supersedes, payload_hash, is_current, revenue, net_profit, total_assets,
             total_liabilities, equity, operating_cash_flow, collected_at)
        SELECT symbol, report_period, announce_date, statement_type, 1, announce_date,
               NULL, lower(hex(randomblob(32))), 1, revenue, net_profit, total_assets,
               total_liabilities, equity, operating_cash_flow, datetime('now')
        FROM financial_report_legacy;
        DROP TABLE financial_report_legacy;
        CREATE INDEX IF NOT EXISTS idx_financial_current ON financial_report(symbol, is_current);
        COMMIT;
        "#,
    )?;
    Ok(())
}

fn migrate_data_job(conn: &Connection) -> rusqlite::Result<()> {
    let columns = conn
        .prepare("PRAGMA table_info(data_job)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if !columns.iter().any(|column| column == "idempotency_key") {
        conn.execute_batch(
            "ALTER TABLE data_job ADD COLUMN idempotency_key TEXT;
             ALTER TABLE data_job ADD COLUMN attempt INTEGER NOT NULL DEFAULT 1;
             ALTER TABLE data_job ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';",
        )?;
    }
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_data_job_key ON data_job(idempotency_key)",
        [],
    )?;
    Ok(())
}

fn migrate_quote_staging(conn: &Connection) -> rusqlite::Result<()> {
    let columns = conn
        .prepare("PRAGMA table_info(quote_staging)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if !columns.iter().any(|column| column == "batch_id") {
        conn.execute(
            "ALTER TABLE quote_staging ADD COLUMN batch_id TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    Ok(())
}

fn migrate_source_columns(conn: &Connection) -> rusqlite::Result<()> {
    let tables: Vec<(&str, Vec<(&str, &str)>)> = vec![
        (
            "quote_daily",
            vec![
                ("source", "TEXT NOT NULL DEFAULT 'demo'"),
                ("batch_id", "TEXT NOT NULL DEFAULT ''"),
                ("fetched_at", "TEXT NOT NULL DEFAULT ''"),
                ("amount_known", "INTEGER NOT NULL DEFAULT 1"),
                ("adj_source", "TEXT NOT NULL DEFAULT ''"),
                ("adj_batch_id", "TEXT NOT NULL DEFAULT ''"),
            ],
        ),
        (
            "index_quote",
            vec![
                ("source", "TEXT NOT NULL DEFAULT 'demo'"),
                ("batch_id", "TEXT NOT NULL DEFAULT ''"),
                ("fetched_at", "TEXT NOT NULL DEFAULT ''"),
                ("amount_known", "INTEGER NOT NULL DEFAULT 1"),
            ],
        ),
    ];
    for (table, columns) in tables {
        let existing = conn
            .prepare(&format!("PRAGMA table_info({table})"))?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (column, declaration) in columns {
            if !existing.iter().any(|name| name == column) {
                conn.execute(
                    &format!("ALTER TABLE {table} ADD COLUMN {column} {declaration}"),
                    [],
                )?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
pub fn test_state() -> AppState {
    let conn = init_db(":memory:").expect("in-memory db");
    AppState {
        db: Arc::new(Mutex::new(conn)),
        sources: Arc::new(crate::sources::SourceHub::new()),
    }
}
