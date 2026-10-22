use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use rusqlite::{params, Connection};
use std::collections::HashMap;

use crate::db::now_iso;

struct Stock {
    symbol: &'static str,
    name: &'static str,
    pinyin: &'static str,
    industry: &'static str,
    board: &'static str,
    base: f64,
    shares: f64,
    float_shares: f64,
    pe: f64,
    pb: f64,
    controller: &'static str,
    region: &'static str,
    business: &'static str,
}

const STOCKS: &[Stock] = &[
    Stock {
        symbol: "600519.SH",
        name: "贵州茅台",
        pinyin: "gzmt",
        industry: "食品饮料",
        board: "主板",
        base: 1680.0,
        shares: 12.56,
        float_shares: 12.56,
        pe: 28.0,
        pb: 8.5,
        controller: "贵州省国资委",
        region: "贵州",
        business: "茅台酒及系列酒的生产与销售。",
    },
    Stock {
        symbol: "000858.SZ",
        name: "五粮液",
        pinyin: "wly",
        industry: "食品饮料",
        board: "主板",
        base: 142.0,
        shares: 38.82,
        float_shares: 38.80,
        pe: 18.0,
        pb: 4.2,
        controller: "宜宾市国资委",
        region: "四川",
        business: "白酒及系列酒的生产与销售。",
    },
    Stock {
        symbol: "601318.SH",
        name: "中国平安",
        pinyin: "zgpa",
        industry: "保险",
        board: "主板",
        base: 52.0,
        shares: 182.1,
        float_shares: 108.3,
        pe: 9.0,
        pb: 1.1,
        controller: "无实际控制人",
        region: "广东",
        business: "保险、银行、资产管理及金融科技服务。",
    },
    Stock {
        symbol: "600036.SH",
        name: "招商银行",
        pinyin: "zsyh",
        industry: "银行",
        board: "主板",
        base: 38.0,
        shares: 252.2,
        float_shares: 206.3,
        pe: 7.0,
        pb: 1.0,
        controller: "招商局集团",
        region: "广东",
        business: "公司及零售银行、财富管理、金融市场业务。",
    },
    Stock {
        symbol: "000333.SZ",
        name: "美的集团",
        pinyin: "mdjt",
        industry: "家电",
        board: "主板",
        base: 68.0,
        shares: 76.5,
        float_shares: 70.2,
        pe: 14.0,
        pb: 3.0,
        controller: "何享健家族",
        region: "广东",
        business: "家用空调、消费电器、机器人与自动化系统。",
    },
    Stock {
        symbol: "601166.SH",
        name: "兴业银行",
        pinyin: "xyyh",
        industry: "银行",
        board: "主板",
        base: 18.0,
        shares: 207.7,
        float_shares: 207.6,
        pe: 5.0,
        pb: 0.6,
        controller: "福建省财政厅",
        region: "福建",
        business: "商业银行、绿色金融与同业业务。",
    },
    Stock {
        symbol: "600900.SH",
        name: "长江电力",
        pinyin: "cjdl",
        industry: "电力",
        board: "主板",
        base: 28.0,
        shares: 244.7,
        float_shares: 244.7,
        pe: 20.0,
        pb: 3.2,
        controller: "三峡集团",
        region: "湖北",
        business: "水力发电、配售电与综合能源服务。",
    },
    Stock {
        symbol: "000651.SZ",
        name: "格力电器",
        pinyin: "gldq",
        industry: "家电",
        board: "主板",
        base: 42.0,
        shares: 56.3,
        float_shares: 55.8,
        pe: 8.0,
        pb: 1.8,
        controller: "珠海市国资委",
        region: "广东",
        business: "空调、生活电器与工业制品。",
    },
    Stock {
        symbol: "300750.SZ",
        name: "宁德时代",
        pinyin: "ndsd",
        industry: "新能源",
        board: "创业板",
        base: 210.0,
        shares: 44.0,
        float_shares: 39.5,
        pe: 22.0,
        pb: 4.5,
        controller: "曾毓群",
        region: "福建",
        business: "动力电池、储能电池与电池回收。",
    },
    Stock {
        symbol: "002594.SZ",
        name: "比亚迪",
        pinyin: "byd",
        industry: "汽车",
        board: "主板",
        base: 260.0,
        shares: 29.1,
        float_shares: 20.8,
        pe: 25.0,
        pb: 4.8,
        controller: "王传福",
        region: "广东",
        business: "新能源汽车、动力电池与电子代工。",
    },
    Stock {
        symbol: "601899.SH",
        name: "紫金矿业",
        pinyin: "zjky",
        industry: "有色",
        board: "主板",
        base: 17.0,
        shares: 263.0,
        float_shares: 225.0,
        pe: 16.0,
        pb: 3.5,
        controller: "福建省国资委",
        region: "福建",
        business: "铜、金、锌等矿产资源的勘探与开发。",
    },
    Stock {
        symbol: "600030.SH",
        name: "中信证券",
        pinyin: "zxzq",
        industry: "券商",
        board: "主板",
        base: 26.0,
        shares: 148.2,
        float_shares: 118.0,
        pe: 15.0,
        pb: 1.5,
        controller: "中信集团",
        region: "广东",
        business: "证券经纪、投行、资产管理与国际业务。",
    },
    Stock {
        symbol: "300059.SZ",
        name: "东方财富",
        pinyin: "dfcf",
        industry: "券商",
        board: "创业板",
        base: 22.0,
        shares: 158.0,
        float_shares: 133.0,
        pe: 30.0,
        pb: 3.8,
        controller: "其实",
        region: "上海",
        business: "互联网证券、基金销售与金融数据服务。",
    },
];

const INDICES: &[(&str, &str, f64)] = &[
    ("000001.SH", "上证指数", 3200.0),
    ("399001.SZ", "深证成指", 10500.0),
    ("399006.SZ", "创业板指", 2100.0),
    ("000300.SH", "沪深300", 3800.0),
    ("000905.SH", "中证500", 5600.0),
];

const SECTORS: &[(&str, &[&str])] = &[
    ("食品饮料", &["600519.SH", "000858.SZ"]),
    ("银行", &["600036.SH", "601166.SH"]),
    ("保险", &["601318.SH"]),
    ("家电", &["000333.SZ", "000651.SZ"]),
    ("电力", &["600900.SH"]),
    ("新能源", &["300750.SZ"]),
    ("汽车", &["002594.SZ"]),
    ("有色", &["601899.SH"]),
    ("券商", &["600030.SH", "300059.SZ"]),
];

fn lcg(seed: &mut u64) -> f64 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*seed >> 33) as f64) / (u32::MAX as f64)
}

const CN_HOLIDAYS: &[&str] = &[
    "2025-01-01",
    "2025-01-28",
    "2025-01-29",
    "2025-01-30",
    "2025-01-31",
    "2025-02-03",
    "2025-02-04",
    "2025-04-04",
    "2025-05-01",
    "2025-05-02",
    "2025-05-05",
    "2025-05-31",
    "2025-06-02",
    "2025-10-01",
    "2025-10-02",
    "2025-10-03",
    "2025-10-06",
    "2025-10-07",
    "2025-10-08",
    "2026-01-01",
    "2026-01-02",
    "2026-02-16",
    "2026-02-17",
    "2026-02-18",
    "2026-02-19",
    "2026-02-20",
    "2026-02-23",
    "2026-04-06",
    "2026-05-01",
    "2026-05-04",
    "2026-05-05",
    "2026-06-19",
    "2026-09-25",
    "2026-10-01",
    "2026-10-02",
    "2026-10-05",
    "2026-10-06",
    "2026-10-07",
];

fn is_holiday(date: NaiveDate) -> bool {
    CN_HOLIDAYS.contains(&date.format("%Y-%m-%d").to_string().as_str())
}

fn trade_dates() -> Vec<NaiveDate> {
    let today = Local::now().date_naive();
    let mut dates = Vec::new();
    let mut d = today - Duration::days(420);
    while d <= today {
        if !matches!(d.weekday(), Weekday::Sat | Weekday::Sun) && !is_holiday(d) {
            dates.push(d);
        }
        d += Duration::days(1);
    }
    if dates.len() > 260 {
        dates.split_off(dates.len() - 260)
    } else {
        dates
    }
}

pub fn seed(conn: &Connection) -> rusqlite::Result<()> {
    let dates = trade_dates();
    let date_strings: Vec<String> = dates
        .iter()
        .map(|d| d.format("%Y-%m-%d").to_string())
        .collect();
    let first_date = date_strings
        .first()
        .cloned()
        .unwrap_or_else(|| "2025-10-21".into());
    let last_date = date_strings
        .last()
        .cloned()
        .unwrap_or_else(|| "2026-10-21".into());
    let now = now_iso();

    let tx = conn.unchecked_transaction()?;

    for d in &date_strings {
        tx.execute(
            "INSERT OR IGNORE INTO trading_calendar (trade_date, is_open) VALUES (?1, 1)",
            params![d],
        )?;
    }

    let mut prices: HashMap<&str, Vec<f64>> = HashMap::new();
    let mut closes_by_date: HashMap<String, Vec<f64>> = HashMap::new();

    for (idx, stock) in STOCKS.iter().enumerate() {
        tx.execute(
            "INSERT OR REPLACE INTO instruments (symbol, name, pinyin, exchange, board, industry, list_date, delist_date, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, 'listed')",
            params![stock.symbol, stock.name, stock.pinyin, if stock.symbol.ends_with(".SH") { "SH" } else { "SZ" }, stock.board, stock.industry, "2010-01-01"],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO company_profile (symbol, full_name, business, industry, list_date, controller, website, region, intro, source, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'demo-seed', ?10)",
            params![
                stock.symbol,
                format!("{}股份有限公司", stock.name),
                stock.business,
                stock.industry,
                "2010-01-01",
                stock.controller,
                format!("https://example.com/{}", stock.pinyin),
                stock.region,
                format!("{} 是一家以{}为主业的 A 股上市公司。本页为演示数据。", stock.name, stock.business),
                now
            ],
        )?;

        let mut seed_val = 0x9E3779B97F4A7C15u64 ^ (idx as u64 * 7919);
        let mut prev_close = stock.base;
        let mut series = Vec::with_capacity(date_strings.len());
        for (i, d) in date_strings.iter().enumerate() {
            let wave = ((i as f64 / 24.0) + idx as f64 * 0.7).sin() * 0.07
                + ((i as f64 / 73.0) + idx as f64 * 0.3).cos() * 0.045;
            let noise = (lcg(&mut seed_val) - 0.5) * 0.012;
            let close = (stock.base * (1.0 + wave + noise)).max(1.0);
            let open = (prev_close * (1.0 + (lcg(&mut seed_val) - 0.5) * 0.008)).max(1.0);
            let high = open.max(close) * (1.0 + lcg(&mut seed_val) * 0.012);
            let low = open.min(close) * (1.0 - lcg(&mut seed_val) * 0.012);
            prev_close = close;
            let volume = stock.float_shares * 100_000_000.0 * (0.0015 + lcg(&mut seed_val) * 0.004);
            let amount = volume * close;
            let turnover = volume / (stock.float_shares * 100_000_000.0) * 100.0;
            tx.execute(
                "INSERT OR REPLACE INTO quote_daily (symbol, trade_date, open, high, low, close, volume, amount, adj_factor, turnover) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1.0, ?9)",
                params![stock.symbol, d, open, high, low, close, volume, amount, turnover],
            )?;
            let total_mv = stock.shares * 100_000_000.0 * close;
            let float_mv = stock.float_shares * 100_000_000.0 * close;
            let pe = stock.pe * (close / stock.base);
            let pb = stock.pb * (close / stock.base);
            tx.execute(
                "INSERT OR REPLACE INTO daily_basic (symbol, trade_date, total_shares, float_shares, total_mv, float_mv, pe_ttm, pb, turnover) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![stock.symbol, d, stock.shares * 100_000_000.0, stock.float_shares * 100_000_000.0, total_mv, float_mv, pe, pb, turnover],
            )?;
            series.push(close);
            closes_by_date
                .entry(d.clone())
                .or_default()
                .push(close / stock.base);
        }
        prices.insert(stock.symbol, series);

        for (period, announce) in [
            ("2023-12-31", "2024-03-28"),
            ("2024-12-31", "2025-03-27"),
            ("2025-12-31", "2026-03-26"),
            ("2026-03-31", "2026-04-28"),
            ("2026-06-30", "2026-08-27"),
        ] {
            let scale = if period.starts_with("2026") {
                0.55
            } else {
                1.0
            };
            let revenue = stock.shares * 100_000_000.0 * stock.base * 0.45 * scale;
            let net_profit = revenue * (0.08 + (idx as f64 % 5.0) * 0.015);
            let assets = revenue * 2.4;
            let liabilities = assets * (0.35 + (idx as f64 % 4.0) * 0.06);
            let equity = assets - liabilities;
            tx.execute(
                "INSERT OR REPLACE INTO financial_report (symbol, report_period, announce_date, statement_type, version, effective_at, supersedes, payload_hash, is_current, revenue, net_profit, total_assets, total_liabilities, equity, operating_cash_flow, collected_at) VALUES (?1, ?2, ?3, ?4, 1, ?3, NULL, ?11, 1, ?5, ?6, ?7, ?8, ?9, ?10, ?12)",
                params![stock.symbol, period, announce, if period.ends_with("12-31") { "annual" } else { "quarterly" }, revenue, net_profit, assets, liabilities, equity, net_profit * 1.15, format!("seed-{}-{}-v1", stock.symbol, period), now_iso()],
            )?;
            tx.execute(
                "INSERT OR REPLACE INTO financial_indicator (symbol, report_period, roe, gross_margin, net_margin, debt_ratio, eps, bps) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    stock.symbol,
                    period,
                    8.0 + (idx as f64 % 7.0) * 1.6,
                    35.0 + (idx as f64 % 9.0) * 2.2,
                    net_profit / revenue * 100.0,
                    liabilities / assets * 100.0,
                    net_profit / (stock.shares * 100_000_000.0),
                    equity / (stock.shares * 100_000_000.0)
                ],
            )?;
        }
    }

    // One seeded correction demonstrates immutable versions and point-in-time reads.
    tx.execute(
        "INSERT INTO financial_report (symbol, report_period, announce_date, statement_type, version, effective_at, supersedes, payload_hash, is_current, revenue, net_profit, total_assets, total_liabilities, equity, operating_cash_flow, collected_at)
         SELECT symbol, report_period, '2026-09-10', statement_type, 2, '2026-09-10', 1, 'seed-600519.SH-2026-06-30-v2', 1,
                revenue * 1.01, net_profit * 1.02, total_assets, total_liabilities, equity * 1.01, operating_cash_flow, ?1
         FROM financial_report
         WHERE symbol = '600519.SH' AND report_period = '2026-06-30' AND version = 1",
        params![now_iso()],
    )?;
    tx.execute(
        "UPDATE financial_report SET is_current = 0 WHERE symbol = '600519.SH' AND report_period = '2026-06-30' AND version = 1",
        [],
    )?;

    for (idx, (code, name, base)) in INDICES.iter().enumerate() {
        tx.execute(
            "INSERT OR REPLACE INTO index_catalog (code, name, market, category, status) VALUES (?1, ?2, ?3, 'broad', 'active')",
            params![code, name, if code.ends_with(".SH") { "SH" } else { "SZ" }],
        )?;
        for (i, d) in date_strings.iter().enumerate() {
            let avg = closes_by_date
                .get(d)
                .map(|v| v.iter().sum::<f64>() / v.len() as f64)
                .unwrap_or(1.0);
            let wave = ((i as f64 / 37.0) + idx as f64 * 0.9).sin() * 0.002;
            let close = base * avg * (1.0 + wave);
            let open = close * (1.0 - 0.002);
            let high = close * 1.006;
            let low = close * 0.994;
            let volume = 2.5e11 * (0.8 + (i as f64 * 0.013).sin() * 0.2);
            tx.execute(
                "INSERT OR REPLACE INTO index_quote (index_code, trade_date, open, high, low, close, volume, amount) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![code, d, open, high, low, close, volume, volume * close / 3200.0],
            )?;
        }
    }

    for stock in STOCKS {
        for (code, _, _) in INDICES {
            let include = match *code {
                "000300.SH" => true,
                "000001.SH" => !matches!(stock.symbol, "300750.SZ" | "300059.SZ" | "002594.SZ"),
                "399001.SZ" => matches!(
                    stock.symbol,
                    "000858.SZ"
                        | "000333.SZ"
                        | "000651.SZ"
                        | "002594.SZ"
                        | "300750.SZ"
                        | "300059.SZ"
                ),
                "399006.SZ" => matches!(stock.symbol, "300750.SZ" | "300059.SZ"),
                "000905.SH" => matches!(
                    stock.symbol,
                    "601166.SH"
                        | "600900.SH"
                        | "601899.SH"
                        | "600030.SH"
                        | "000651.SZ"
                        | "000333.SZ"
                ),
                _ => false,
            };
            if include {
                tx.execute(
                    "INSERT OR REPLACE INTO index_member (index_code, symbol, in_date, out_date, weight) VALUES (?1, ?2, ?3, NULL, ?4)",
                    params![code, stock.symbol, first_date, 1.0 / 13.0],
                )?;
            }
        }
    }

    for (sector_name, members) in SECTORS {
        tx.execute(
            "INSERT OR IGNORE INTO sector_catalog (name, sector_type, source, status) VALUES (?1, 'industry', 'demo-seed', 'active')",
            params![sector_name],
        )?;
        let sector_id: i64 = tx.query_row(
            "SELECT id FROM sector_catalog WHERE name = ?1",
            params![sector_name],
            |r| r.get(0),
        )?;
        for symbol in *members {
            tx.execute(
                "INSERT OR REPLACE INTO sector_member (sector_id, symbol, in_date, out_date, weight) VALUES (?1, ?2, ?3, NULL, ?4)",
                params![sector_id, symbol, first_date, 1.0 / members.len() as f64],
            )?;
        }
    }

    tx.execute(
        "INSERT OR IGNORE INTO watchlist_group (id, name, sort) VALUES ('grp-default', '核心观察', 0)",
        [],
    )?;
    tx.execute(
        "INSERT OR IGNORE INTO watchlist_group (id, name, sort) VALUES ('grp-banks', '银行与保险', 1)",
        [],
    )?;
    for (i, symbol) in ["600519.SH", "300750.SZ", "601318.SH", "600036.SH"]
        .iter()
        .enumerate()
    {
        tx.execute(
            "INSERT OR IGNORE INTO watchlist_item (id, group_id, symbol, note, sort, created_at) VALUES (?1, 'grp-default', ?2, '', ?3, ?4)",
            params![format!("item-default-{i}"), symbol, i as i64, now],
        )?;
    }
    for (i, symbol) in ["600036.SH", "601166.SH", "601318.SH"].iter().enumerate() {
        tx.execute(
            "INSERT OR IGNORE INTO watchlist_item (id, group_id, symbol, note, sort, created_at) VALUES (?1, 'grp-banks', ?2, '', ?3, ?4)",
            params![format!("item-banks-{i}"), symbol, i as i64, now],
        )?;
    }

    let defaults = [
        ("theme", "dark"),
        ("up_color", "red"),
        ("default_home", "/"),
        ("timezone", "Asia/Shanghai"),
        ("density", "compact"),
        ("poll_interval", "30"),
    ];
    for (k, v) in defaults {
        tx.execute(
            "INSERT OR REPLACE INTO app_setting (key, value) VALUES (?1, ?2)",
            params![k, v],
        )?;
    }

    for (dataset, coverage, missing, anomalies) in [
        ("instruments", 1.0, 0, 0),
        ("quote_daily", 0.998, 12, 1),
        ("daily_basic", 0.996, 26, 0),
        ("financial_report", 0.985, 8, 0),
        ("index_quote", 0.999, 1, 0),
        ("company_profile", 0.98, 2, 0),
    ] {
        tx.execute(
            "INSERT OR REPLACE INTO quality_report (dataset, coverage, missing, duplicates, anomalies, updated_at) VALUES (?1, ?2, ?3, 0, ?4, ?5)",
            params![dataset, coverage, missing, anomalies, now],
        )?;
    }

    for (id, dataset, status, progress, message) in [
        (
            "job-seed-1",
            "instruments",
            "succeeded",
            100,
            "12 只演示证券，已去重",
        ),
        (
            "job-seed-2",
            "quote_daily",
            "succeeded",
            100,
            "260 个交易日演示行情",
        ),
        (
            "job-seed-3",
            "financial_report",
            "failed",
            62,
            "演示任务：数据源限频，可重试",
        ),
    ] {
        tx.execute(
            "INSERT OR REPLACE INTO data_job (id, dataset, status, progress, message, started_at, finished_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, dataset, status, progress, message, now, if status == "succeeded" { Some(now.clone()) } else { None }],
        )?;
    }

    tx.commit()?;
    let _ = last_date;
    Ok(())
}

pub fn purge_holiday_data(conn: &Connection) -> rusqlite::Result<()> {
    for holiday in CN_HOLIDAYS {
        conn.execute(
            "DELETE FROM trading_calendar WHERE trade_date = ?1",
            rusqlite::params![holiday],
        )?;
        conn.execute(
            "DELETE FROM quote_daily WHERE trade_date = ?1 AND source = 'demo'",
            rusqlite::params![holiday],
        )?;
        conn.execute(
            "DELETE FROM index_quote WHERE trade_date = ?1 AND source = 'demo'",
            rusqlite::params![holiday],
        )?;
    }
    Ok(())
}
