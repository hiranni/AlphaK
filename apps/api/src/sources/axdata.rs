use chrono::Local;
use serde_json::{json, Value};

use super::{DailyBar, Snapshot};

const TIMEOUT_SECS: u64 = 12;

fn base() -> Option<String> {
    std::env::var("AXDATA_API_BASE")
        .ok()
        .map(|value| value.trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
}

fn token() -> Option<String> {
    std::env::var("AXDATA_API_TOKEN")
        .ok()
        .filter(|value| !value.is_empty())
}

pub fn enabled() -> bool {
    base().is_some()
}

pub(crate) async fn request(
    client: &reqwest::Client,
    interface: &str,
    params: Value,
    fields: &[&str],
) -> Result<Vec<Value>, String> {
    let base = base().ok_or("AXDATA_API_BASE is not configured")?;
    let url = format!("{base}/v1/request/{interface}");
    let mut builder = client
        .post(url)
        .json(&json!({"params": params, "fields": fields}));
    if let Some(token) = token() {
        builder = builder.bearer_auth(token);
    }
    let response =
        tokio::time::timeout(std::time::Duration::from_secs(TIMEOUT_SECS), builder.send())
            .await
            .map_err(|_| "axdata request timeout".to_string())?
            .map_err(|error| format!("axdata request: {error}"))?
            .error_for_status()
            .map_err(|error| format!("axdata status: {error}"))?;
    let payload: Value = response
        .json()
        .await
        .map_err(|error| format!("axdata json: {error}"))?;
    if payload["success"].as_bool() != Some(true) {
        return Err(format!(
            "axdata error: {}",
            payload["error"]["message"]
                .as_str()
                .unwrap_or("unknown source error")
        ));
    }
    payload["data"]
        .as_array()
        .cloned()
        .ok_or_else(|| "axdata response missing data".to_string())
}

pub async fn daily(
    client: &reqwest::Client,
    symbol: &str,
    start: &str,
    end: &str,
    adjust: &str,
    index: bool,
) -> Result<Vec<DailyBar>, String> {
    let interface = if index {
        "index_kline_tdx"
    } else {
        "stock_kline_daily_tdx"
    };
    let mut params = json!({"code": symbol});
    if !adjust.is_empty() {
        params["adjust"] = json!(adjust);
    }
    let rows = request(
        client,
        interface,
        params,
        &[
            "instrument_id",
            "trade_time",
            "open",
            "high",
            "low",
            "close",
            "volume",
            "amount",
        ],
    )
    .await?;
    let mut bars = Vec::new();
    for row in rows {
        let trade_time = row["trade_time"].as_str().unwrap_or("");
        let date = trade_time.get(..10).unwrap_or("");
        if date.len() != 10 || date < start || date > end {
            continue;
        }
        let number = |key: &str| {
            row[key]
                .as_f64()
                .ok_or_else(|| format!("axdata field missing: {key}"))
        };
        let bar = DailyBar {
            symbol: symbol.into(),
            date: date.into(),
            open: number("open")?,
            high: number("high")?,
            low: number("low")?,
            close: number("close")?,
            volume: number("volume")?,
            amount: number("amount").unwrap_or(0.0),
            amount_known: row.get("amount").is_some(),
            adj_factor: None,
        };
        bar.validate()?;
        bars.push(bar);
    }
    bars.sort_by(|left, right| left.date.cmp(&right.date));
    if bars.is_empty() {
        return Err("axdata daily range has no rows".into());
    }
    Ok(bars)
}

pub async fn snapshot(client: &reqwest::Client, symbol: &str) -> Result<Snapshot, String> {
    snapshots(client, &[symbol.to_string()])
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| "axdata snapshot empty".to_string())
}

pub async fn snapshots(
    client: &reqwest::Client,
    symbols: &[String],
) -> Result<Vec<Snapshot>, String> {
    if symbols.is_empty() {
        return Ok(Vec::new());
    }
    let rows = request(
        client,
        "stock_realtime_snapshot_tdx",
        json!({"code": symbols}),
        &[
            "instrument_id",
            "last_price",
            "pre_close",
            "open",
            "high",
            "low",
            "volume",
            "amount",
        ],
    )
    .await?;
    let mut snapshots = Vec::new();
    for row in rows {
        let symbol = row["instrument_id"].as_str().unwrap_or("").to_string();
        let number = |key: &str| {
            row[key]
                .as_f64()
                .ok_or_else(|| format!("axdata snapshot field missing: {key}"))
        };
        let close = number("last_price")?;
        let prev_close = number("pre_close")?;
        let snapshot = Snapshot {
            symbol,
            name: String::new(),
            date: Local::now().format("%Y-%m-%d").to_string(),
            open: number("open")?,
            high: number("high")?,
            low: number("low")?,
            close,
            prev_close,
            volume: number("volume")?,
            amount: number("amount").unwrap_or(0.0),
            change_pct: (close / prev_close - 1.0) * 100.0,
        };
        snapshot.validate()?;
        snapshots.push(snapshot);
    }
    if snapshots.is_empty() {
        return Err("axdata snapshot empty".into());
    }
    Ok(snapshots)
}

pub async fn seconds(
    client: &reqwest::Client,
    symbol: &str,
    seconds: u32,
) -> Result<Vec<crate::models::Bar>, String> {
    let rows = request(
        client,
        "stock_kline_second_tdx",
        json!({"code": symbol, "seconds": seconds, "adjust": "none"}),
        &[
            "instrument_id",
            "trade_time",
            "open",
            "high",
            "low",
            "close",
            "volume",
            "amount",
        ],
    )
    .await?;
    let mut bars = Vec::new();
    for row in rows {
        let trade_time = row["trade_time"].as_str().unwrap_or("");
        let timestamp = chrono::DateTime::parse_from_rfc3339(trade_time)
            .map_err(|_| format!("axdata seconds bad time: {trade_time}"))?
            .timestamp();
        let number = |key: &str| {
            row[key]
                .as_f64()
                .ok_or_else(|| format!("axdata seconds field missing: {key}"))
        };
        let bar = crate::models::Bar {
            date: timestamp.to_string(),
            open: number("open")?,
            high: number("high")?,
            low: number("low")?,
            close: number("close")?,
            volume: number("volume")?,
            amount: number("amount").unwrap_or(0.0),
            ma5: None,
            ma10: None,
            ma20: None,
            ma60: None,
            vol_ma5: None,
            vol_ma10: None,
            ema12: None,
            boll_upper: None,
            boll_mid: None,
            boll_lower: None,
            dif: None,
            dea: None,
            macd: None,
            k: None,
            d: None,
            j: None,
            rsi: None,
        };
        if bar.open <= 0.0 || bar.close <= 0.0 || bar.low <= 0.0 {
            return Err(format!("axdata seconds invalid price: {trade_time}"));
        }
        if bar.high < bar.low || bar.high < bar.open || bar.high < bar.close || bar.volume < 0.0 {
            return Err(format!("axdata seconds invalid bar: {trade_time}"));
        }
        bars.push(bar);
    }
    bars.sort_by(|left, right| left.date.cmp(&right.date));
    bars.dedup_by(|left, right| left.date == right.date);
    if bars.is_empty() {
        return Err("axdata seconds empty".into());
    }
    Ok(bars)
}
