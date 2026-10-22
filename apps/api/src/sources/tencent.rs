use encoding_rs::GB18030;
use serde_json::Value;

use super::{DailyBar, Snapshot};

const QUOTE: &str = "https://qt.gtimg.cn/q=";
const KLINE: &str = "https://web.ifzq.gtimg.cn/appstock/app/fqkline/get";

fn code(symbol: &str) -> Result<String, String> {
    let (code, exchange) = symbol
        .split_once('.')
        .ok_or_else(|| format!("invalid symbol: {symbol}"))?;
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("invalid symbol: {symbol}"));
    }
    let prefix = match exchange.to_ascii_uppercase().as_str() {
        "SH" => "sh",
        "SZ" => "sz",
        "BJ" => "bj",
        _ => return Err(format!("unsupported exchange: {exchange}")),
    };
    Ok(format!("{prefix}{code}"))
}

pub async fn daily(
    client: &reqwest::Client,
    symbol: &str,
    start: &str,
    end: &str,
    adjust: &str,
) -> Result<Vec<DailyBar>, String> {
    let key = match adjust {
        "" => "day",
        "qfq" => "qfqday",
        "hfq" => "hfqday",
        other => return Err(format!("unsupported adjust: {other}")),
    };
    let param = format!("{},day,{},{},320,{}", code(symbol)?, start, end, adjust);
    let response = client
        .get(KLINE)
        .query(&[("param", param)])
        .send()
        .await
        .map_err(|err| format!("tencent daily request: {err}"))?
        .error_for_status()
        .map_err(|err| format!("tencent daily status: {err}"))?;
    let text = response
        .text()
        .await
        .map_err(|err| format!("tencent daily body: {err}"))?;
    parse_daily(&text, symbol, key)
}

pub fn parse_daily(text: &str, symbol: &str, key: &str) -> Result<Vec<DailyBar>, String> {
    let value: Value = serde_json::from_str(text).map_err(|err| format!("tencent json: {err}"))?;
    if value["code"].as_i64().unwrap_or(-1) != 0 {
        return Err(format!("tencent code: {}", value["code"]));
    }
    let node = value["data"]
        .as_object()
        .and_then(|data| data.values().next())
        .ok_or_else(|| "tencent response missing data".to_string())?;
    let rows = node[key]
        .as_array()
        .or_else(|| node["day"].as_array())
        .ok_or_else(|| format!("tencent response missing {key}"))?;
    let mut bars = Vec::with_capacity(rows.len());
    for row in rows {
        let fields = row
            .as_array()
            .ok_or_else(|| "tencent kline not array".to_string())?;
        if fields.len() < 6 {
            return Err("tencent kline fields".into());
        }
        let bar = DailyBar {
            symbol: symbol.to_string(),
            date: field(fields, 0)?,
            open: parse_num(field(fields, 1)?)?,
            close: parse_num(field(fields, 2)?)?,
            high: parse_num(field(fields, 3)?)?,
            low: parse_num(field(fields, 4)?)?,
            volume: parse_num(field(fields, 5)?)?,
            amount: 0.0,
            amount_known: false,
            adj_factor: None,
        };
        bar.validate()?;
        bars.push(bar);
    }
    bars.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(bars)
}

pub async fn snapshot(client: &reqwest::Client, symbol: &str) -> Result<Snapshot, String> {
    let response = client
        .get(format!("{QUOTE}{}", code(symbol)?))
        .send()
        .await
        .map_err(|err| format!("tencent snapshot request: {err}"))?
        .error_for_status()
        .map_err(|err| format!("tencent snapshot status: {err}"))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("tencent snapshot body: {err}"))?;
    let (text, _, _) = GB18030.decode(&bytes);
    parse_snapshot(&text, symbol)
}

pub fn parse_snapshot(text: &str, symbol: &str) -> Result<Snapshot, String> {
    let payload = text
        .split_once("=\"")
        .and_then(|(_, rest)| rest.rsplit_once('"'))
        .map(|(value, _)| value)
        .ok_or_else(|| "tencent snapshot malformed".to_string())?;
    let fields: Vec<&str> = payload.split('~').collect();
    if fields.len() < 38 {
        return Err("tencent snapshot fields".into());
    }
    let num = |idx: usize| -> Result<f64, String> {
        fields
            .get(idx)
            .and_then(|value| value.parse::<f64>().ok())
            .ok_or_else(|| format!("tencent snapshot field {idx}"))
    };
    let date = fields[30]
        .get(0..8)
        .filter(|value| value.len() == 8)
        .map(|value| format!("{}-{}-{}", &value[0..4], &value[4..6], &value[6..8]))
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let close = num(3)?;
    let prev_close = num(4)?;
    let snapshot = Snapshot {
        symbol: symbol.to_string(),
        name: fields[1].to_string(),
        date,
        open: num(5)?,
        high: num(33)?,
        low: num(34)?,
        close,
        prev_close,
        volume: num(36)?,
        amount: num(37)? * 10_000.0,
        change_pct: num(32)?,
    };
    snapshot.validate()?;
    Ok(snapshot)
}

fn field(fields: &[Value], index: usize) -> Result<String, String> {
    fields
        .get(index)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("tencent field {index}"))
}

fn parse_num(value: String) -> Result<f64, String> {
    value
        .parse::<f64>()
        .map_err(|err| format!("tencent number {value}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_daily() {
        let text = r#"{"code":0,"data":{"sh600519":{"qfqday":[["2026-10-08","1252.900","1255.790","1258.000","1242.000","25168.000"]]}}}"#;
        let bars = parse_daily(text, "600519.SH", "qfqday").unwrap();
        assert_eq!(bars[0].close, 1255.79);
    }

    #[test]
    fn parses_snapshot() {
        let text = "v_sh600519=\"1~贵州茅台~600519~1309.69~1331.88~1331.88~33514~16345~17169~1309.59~2~1309.35~1~1309.30~1~1309.26~2~1309.00~1~1309.69~46~1309.80~10~1310.00~13~1310.05~7~1310.19~1~~20261021161453~-22.19~-1.67~1331.98~1302.36~1309.69/33514/4403823070~33514~440382\";";
        let snapshot = parse_snapshot(text, "600519.SH").unwrap();
        assert_eq!(snapshot.close, 1309.69);
        assert_eq!(snapshot.date, "2026-10-21");
        assert_eq!(snapshot.volume, 33514.0);
    }
}

/// Keyword lookup only: never downloads or scans the full market.
pub async fn search(
    client: &reqwest::Client,
    query: &str,
) -> Result<Vec<(String, String, String, String)>, String> {
    let text = client
        .get("https://smartbox.gtimg.cn/s3/")
        .query(&[("q", query), ("t", "all")])
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    parse_search(&text)
}

fn parse_search(text: &str) -> Result<Vec<(String, String, String, String)>, String> {
    let encoded = text
        .trim()
        .strip_prefix("v_hint=")
        .ok_or("invalid search response")?
        .trim_end_matches(';');
    let decoded: String = serde_json::from_str(encoded).map_err(|e| e.to_string())?;
    Ok(decoded
        .split('^')
        .filter_map(|row| {
            let fields: Vec<_> = row.split('~').collect();
            if fields.len() < 5
                || fields[4] != "GP-A"
                || !matches!(fields[0], "sh" | "sz" | "bj")
                || fields[1].len() != 6
                || !fields[1].bytes().all(|b| b.is_ascii_digit())
                || fields[2].is_empty()
            {
                return None;
            }
            Some((
                format!("{}.{}", fields[1], fields[0].to_uppercase()),
                fields[2].into(),
                fields[3].into(),
                fields[0].to_uppercase(),
            ))
        })
        .take(20)
        .collect())
}

#[cfg(test)]
mod search_tests {
    #[test]
    fn parses_a_share_names_and_filters_other_assets() {
        let rows = super::parse_search(r#"v_hint="sz~002463~\u6caa\u7535\u80a1\u4efd~hdgf~GP-A^hk~00700~Tencent~tx~GP^sh~bad~Invalid~x~GP-A";"#).unwrap();
        assert_eq!(
            rows,
            vec![(
                "002463.SZ".into(),
                "沪电股份".into(),
                "hdgf".into(),
                "SZ".into()
            )]
        );
        assert!(super::parse_search("bad").is_err());
        assert!(super::parse_search(r#"v_hint="";"#).unwrap().is_empty());
    }
}
