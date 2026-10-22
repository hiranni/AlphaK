use chrono::{DateTime, FixedOffset, Utc};
use serde_json::Value;

use super::{DailyBar, Snapshot};

const BASE: &str = "https://push2.eastmoney.com";
const HISTORY: &str = "https://push2his.eastmoney.com";

fn secid(symbol: &str) -> Result<String, String> {
    let (code, exchange) = symbol
        .split_once('.')
        .ok_or_else(|| format!("invalid symbol: {symbol}"))?;
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("invalid symbol: {symbol}"));
    }
    let market = match exchange.to_ascii_uppercase().as_str() {
        "SH" => "1",
        "SZ" | "BJ" => "0",
        _ => return Err(format!("unsupported exchange: {exchange}")),
    };
    Ok(format!("{market}.{code}"))
}

pub async fn daily(
    client: &reqwest::Client,
    symbol: &str,
    start: &str,
    end: &str,
    adjust: &str,
) -> Result<Vec<DailyBar>, String> {
    let fqt = match adjust {
        "" => "0",
        "qfq" => "1",
        "hfq" => "2",
        other => return Err(format!("unsupported adjust: {other}")),
    };
    let response = client
        .get(format!("{HISTORY}/api/qt/stock/kline/get"))
        .query(&[
            ("secid", secid(symbol)?),
            ("klt", "101".into()),
            ("fqt", fqt.into()),
            ("beg", start.replace('-', "")),
            ("end", end.replace('-', "")),
            ("fields1", "f1,f2,f3,f4,f5,f6".into()),
            ("fields2", "f51,f52,f53,f54,f55,f56,f57".into()),
        ])
        .send()
        .await
        .map_err(|err| format!("eastmoney daily request: {err}"))?
        .error_for_status()
        .map_err(|err| format!("eastmoney daily status: {err}"))?;
    let text = response
        .text()
        .await
        .map_err(|err| format!("eastmoney daily body: {err}"))?;
    parse_daily(&text, symbol)
}

pub fn parse_daily(text: &str, symbol: &str) -> Result<Vec<DailyBar>, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|err| format!("eastmoney json: {err}"))?;
    if value["rc"].as_i64().unwrap_or(-1) != 0 {
        return Err(format!("eastmoney rc: {}", value["rc"]));
    }
    let rows = value["data"]["klines"]
        .as_array()
        .ok_or_else(|| "eastmoney response missing klines".to_string())?;
    let mut bars = Vec::with_capacity(rows.len());
    for row in rows {
        let line = row
            .as_str()
            .ok_or_else(|| "eastmoney kline not string".to_string())?;
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < 7 {
            return Err(format!("eastmoney kline fields: {line}"));
        }
        let bar = DailyBar {
            symbol: symbol.to_string(),
            date: fields[0].to_string(),
            open: parse_num(fields[1])?,
            close: parse_num(fields[2])?,
            high: parse_num(fields[3])?,
            low: parse_num(fields[4])?,
            volume: parse_num(fields[5])?,
            amount: parse_num(fields[6])?,
            amount_known: true,
            adj_factor: None,
        };
        bar.validate()?;
        bars.push(bar);
    }
    bars.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(bars)
}

pub async fn snapshot(client: &reqwest::Client, symbol: &str) -> Result<Snapshot, String> {
    let request = client
        .get(format!("{BASE}/api/qt/stock/get"))
        .query(&[
            ("secid", secid(symbol)?),
            (
                "fields",
                "f43,f44,f45,f46,f47,f48,f57,f58,f60,f86,f169,f170".into(),
            ),
        ])
        .send()
        .await;
    let direct = match request {
        Ok(response) => response
            .error_for_status()
            .map_err(|err| format!("eastmoney snapshot status: {err}")),
        Err(err) => Err(format!("eastmoney snapshot request: {err}")),
    };
    if let Ok(response) = direct {
        if let Ok(text) = response.text().await {
            if let Ok(snapshot) = parse_snapshot(&text, symbol) {
                return Ok(snapshot);
            }
        }
    }
    // The quote host intermittently rejects the realtime endpoint. The last
    // completed daily bar is a valid end-of-day snapshot and is safer than
    // failing the whole source; mark the name empty so callers use the
    // instrument master when rendering.
    let end = chrono::Local::now().format("%Y-%m-%d").to_string();
    let start = (chrono::Local::now().date_naive() - chrono::Duration::days(10))
        .format("%Y-%m-%d")
        .to_string();
    let bars = daily(client, symbol, &start, &end, "").await?;
    let last = bars
        .last()
        .ok_or_else(|| "eastmoney snapshot fallback empty".to_string())?;
    let prev = bars.iter().rev().nth(1);
    let snapshot = Snapshot {
        symbol: symbol.to_string(),
        name: String::new(),
        date: last.date.clone(),
        open: last.open,
        high: last.high,
        low: last.low,
        close: last.close,
        prev_close: prev.map(|bar| bar.close).unwrap_or(last.close),
        volume: last.volume,
        amount: last.amount,
        change_pct: prev
            .map(|bar| (last.close / bar.close - 1.0) * 100.0)
            .unwrap_or(0.0),
    };
    snapshot.validate()?;
    Ok(snapshot)
}

pub fn parse_snapshot(text: &str, symbol: &str) -> Result<Snapshot, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|err| format!("eastmoney json: {err}"))?;
    let data = &value["data"];
    if data.is_null() {
        return Err("eastmoney snapshot empty data".into());
    }
    let price = |key: &str| -> Result<f64, String> {
        let value = data[key]
            .as_f64()
            .ok_or_else(|| format!("eastmoney missing {key}"))?;
        if value <= 0.0 {
            return Err(format!("eastmoney invalid {key}"));
        }
        Ok(value / 100.0)
    };
    let close = price("f43")?;
    let prev_close = price("f60")?;
    let date = data["f86"]
        .as_i64()
        .and_then(|epoch| DateTime::<Utc>::from_timestamp(epoch, 0))
        .map(|utc| {
            utc.with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap())
                .format("%Y-%m-%d")
                .to_string()
        })
        .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
    let change_pct = data["f170"].as_f64().unwrap_or(0.0) / 100.0;
    let snapshot = Snapshot {
        symbol: symbol.to_string(),
        name: data["f58"].as_str().unwrap_or("").to_string(),
        date,
        open: price("f46")?,
        high: price("f44")?,
        low: price("f45")?,
        close,
        prev_close,
        volume: data["f47"].as_f64().unwrap_or(0.0),
        amount: data["f48"].as_f64().unwrap_or(0.0),
        change_pct,
    };
    snapshot.validate()?;
    Ok(snapshot)
}

fn parse_num(value: &str) -> Result<f64, String> {
    value
        .trim()
        .parse::<f64>()
        .map_err(|err| format!("eastmoney number {value}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_daily() {
        let text = r#"{"rc":0,"data":{"klines":["2026-10-08,1252.90,1255.79,1258.00,1242.00,25168,3145413157.00"]}}"#;
        let bars = parse_daily(text, "600519.SH").unwrap();
        assert_eq!(bars[0].close, 1255.79);
        assert_eq!(bars[0].volume, 25168.0);
    }

    #[test]
    fn parses_snapshot() {
        let text = r#"{"rc":0,"data":{"f43":130969,"f44":133198,"f45":130236,"f46":133188,"f47":33514,"f48":4403823070.0,"f57":"600519","f58":"贵州茅台","f60":133188,"f86":1792570298,"f169":-2219,"f170":-167}}"#;
        let snapshot = parse_snapshot(text, "600519.SH").unwrap();
        assert_eq!(snapshot.close, 1309.69);
        assert_eq!(snapshot.prev_close, 1331.88);
        assert_eq!(snapshot.date, "2026-10-21");
        assert!((snapshot.change_pct + 1.67).abs() < 1e-9);
    }
}
