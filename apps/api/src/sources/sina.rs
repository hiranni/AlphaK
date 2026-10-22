use super::{DailyBar, Snapshot};

pub async fn daily(
    client: &reqwest::Client,
    symbol: &str,
    start: &str,
    end: &str,
) -> Result<Vec<DailyBar>, String> {
    let code = crate::live::symbol_code(symbol)?;
    let body=client.get("https://quotes.sina.cn/cn/api/jsonp_v2.php/var%20_data=/CN_MarketDataService.getKLineData").query(&[("symbol",code),("scale","240".into()),("ma","no".into()),("datalen","1023".into())]).send().await.map_err(|error|error.to_string())?.error_for_status().map_err(|error|error.to_string())?.text().await.map_err(|error|error.to_string())?;
    parse_daily(&body, symbol, start, end)
}

pub fn parse_daily(
    body: &str,
    symbol: &str,
    start: &str,
    end: &str,
) -> Result<Vec<DailyBar>, String> {
    let first = body.find('[').ok_or("新浪日线数据为空")?;
    let last = body.rfind(']').ok_or("新浪日线格式错误")?;
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(&body[first..=last]).map_err(|error| error.to_string())?;
    let mut bars = Vec::new();
    for row in rows {
        let date = row["day"]
            .as_str()
            .unwrap_or("")
            .get(..10)
            .ok_or("新浪日期错误")?;
        if date < start || date > end {
            continue;
        }
        let number = |key: &str| {
            row[key]
                .as_f64()
                .or_else(|| row[key].as_str()?.parse().ok())
                .ok_or_else(|| format!("新浪字段缺失 {key}"))
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
            amount_known: row["amount"]
                .as_str()
                .and_then(|number| number.parse::<f64>().ok())
                .or_else(|| row["amount"].as_f64())
                .is_some(),
            adj_factor: None,
        };
        bar.validate()?;
        bars.push(bar);
    }
    bars.sort_by(|left, right| left.date.cmp(&right.date));
    if bars.is_empty() {
        return Err("新浪日线范围内无数据".into());
    }
    Ok(bars)
}

pub async fn snapshot(client: &reqwest::Client, symbol: &str) -> Result<Snapshot, String> {
    let body = client
        .get(format!(
            "https://hq.sinajs.cn/list={}",
            crate::live::symbol_code(symbol)?
        ))
        .header("Referer", "https://finance.sina.com.cn/")
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .bytes()
        .await
        .map_err(|error| error.to_string())?;
    let (body, _, _) = encoding_rs::GB18030.decode(&body);
    parse_snapshot(&body, symbol)
}

pub fn parse_snapshot(body: &str, symbol: &str) -> Result<Snapshot, String> {
    let payload = body
        .split_once("=\"")
        .and_then(|(_, rest)| rest.rsplit_once('"'))
        .map(|(payload, _)| payload)
        .ok_or("新浪快照格式错误")?;
    let fields = payload.split(',').collect::<Vec<_>>();
    let number = |index: usize| {
        fields
            .get(index)
            .and_then(|field| field.parse::<f64>().ok())
            .ok_or_else(|| format!("新浪快照字段缺失 {index}"))
    };
    let prev_close = number(2)?;
    let close = number(3)?;
    let snapshot = Snapshot {
        symbol: symbol.into(),
        name: fields.first().unwrap_or(&"").to_string(),
        date: fields.get(30).ok_or("新浪快照日期缺失")?.to_string(),
        open: number(1)?,
        prev_close,
        close,
        high: number(4)?,
        low: number(5)?,
        volume: number(8)?,
        amount: number(9)?,
        change_pct: (close / prev_close - 1.0) * 100.0,
    };
    snapshot.validate()?;
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn daily_rows_are_filtered_and_validated() {
        let body = r#"var data=([{"day":"2026-10-22","open":"10","high":"12","low":"9","close":"11","volume":"100","amount":"1100"}]);"#;
        let bars = parse_daily(body, "600519.SH", "2026-10-01", "2026-10-22").unwrap();
        assert_eq!(bars[0].amount, 1100.0);
        assert!(parse_daily(body, "600519.SH", "2026-10-23", "2026-10-24").is_err());
    }
}
