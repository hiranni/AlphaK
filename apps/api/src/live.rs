use chrono::{NaiveDateTime, TimeZone};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

use crate::{
    db::AppState,
    models::{Bar, BarsResponse},
};

pub fn symbol_code(symbol: &str) -> Result<String, String> {
    let (number, exchange) = symbol.split_once('.').ok_or("证券代码格式错误")?;
    if number.len() != 6
        || !number.bytes().all(|byte| byte.is_ascii_digit())
        || !matches!(exchange, "SH" | "SZ" | "BJ")
    {
        return Err("证券代码格式错误".into());
    }
    Ok(format!("{}{number}", exchange.to_lowercase()))
}

fn numeric(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|number| number.is_finite())
}

fn text(value: &Value) -> String {
    value.as_str().unwrap_or("").to_string()
}

fn json_body(body: &str) -> Result<Value, String> {
    let start = body
        .char_indices()
        .find(|(_, character)| matches!(character, '[' | '{'))
        .map(|(index, _)| index)
        .ok_or("数据源返回空内容")?;
    let end = body
        .char_indices()
        .rev()
        .find(|(_, character)| matches!(character, ']' | '}'))
        .map(|(index, _)| index)
        .ok_or("数据源返回格式错误")?;
    serde_json::from_str(&body[start..=end]).map_err(|error| error.to_string())
}

async fn get(
    client: &reqwest::Client,
    url: &str,
    query: &[(&str, String)],
) -> Result<Value, String> {
    let body = client
        .get(url)
        .header(
            "Referer",
            if url.contains("eastmoney.com") {
                "https://quote.eastmoney.com/"
            } else {
                "https://finance.sina.com.cn/"
            },
        )
        .query(query)
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .text()
        .await
        .map_err(|error| error.to_string())?;
    json_body(&body)
}

pub fn parse_minutes(value: &Value, source: &str) -> Result<Vec<Bar>, String> {
    let owned;
    let rows = if source == "eastmoney" {
        owned = value["data"]["klines"].as_array().ok_or("分钟行情为空")?.iter().map(|row| {
            let fields = row.as_str().unwrap_or("").split(',').collect::<Vec<_>>();
            json!({"day":fields.first(),"open":fields.get(1),"close":fields.get(2),"high":fields.get(3),"low":fields.get(4),"volume":fields.get(5),"amount":fields.get(6)})
        }).collect::<Vec<_>>();
        &owned
    } else {
        value.as_array().ok_or("分钟行情格式错误")?
    };
    let mut bars = Vec::new();
    for row in rows {
        let date = text(&row["day"]);
        let local = NaiveDateTime::parse_from_str(&date, "%Y-%m-%d %H:%M:%S")
            .or_else(|_| NaiveDateTime::parse_from_str(&format!("{date}:00"), "%Y-%m-%d %H:%M:%S"))
            .map_err(|_| "分钟时间格式错误")?;
        let required = |key: &str| numeric(&row[key]).ok_or_else(|| format!("分钟字段缺失：{key}"));
        let open = required("open")?;
        let high = required("high")?;
        let low = required("low")?;
        let close = required("close")?;
        let volume = required("volume")? * if source == "eastmoney" { 100.0 } else { 1.0 };
        if low <= 0.0
            || high < open.max(close)
            || low > open.min(close)
            || high < low
            || volume < 0.0
        {
            return Err("分钟价格或成交量无效".into());
        }
        let timestamp = chrono::FixedOffset::east_opt(8 * 3600)
            .unwrap()
            .from_local_datetime(&local)
            .single()
            .ok_or("分钟时间无效")?
            .timestamp();
        let mut bar: Bar = serde_json::from_value(json!({"date":timestamp.to_string(),"open":open,"high":high,"low":low,"close":close,"volume":volume,"amount":numeric(&row["amount"]).unwrap_or(0.0)})).map_err(|error| error.to_string())?;
        bar.date = timestamp.to_string();
        bars.push(bar);
    }
    bars.sort_by(|left, right| left.date.cmp(&right.date));
    bars.dedup_by(|left, right| left.date == right.date);
    if bars.is_empty() {
        return Err("分钟行情为空".into());
    }
    crate::indicators::apply(&mut bars);
    Ok(bars)
}

async fn minute_fetch(
    state: &AppState,
    symbol: &str,
    scale: &str,
) -> Result<(Value, String), String> {
    let code = symbol_code(symbol)?;
    let secid = format!(
        "{}.{}",
        if symbol.ends_with(".SH") { "1" } else { "0" },
        &symbol[..6]
    );
    let mut errors = Vec::new();
    for source in ["eastmoney", "sina"] {
        for _attempt in 0..2 {
            let result = if source == "eastmoney" {
                get(
                    &state.sources.client,
                    "https://push2his.eastmoney.com/api/qt/stock/kline/get",
                    &[
                        ("secid", secid.clone()),
                        ("klt", scale.into()),
                        ("fqt", "0".into()),
                        ("beg", "0".into()),
                        ("end", "20500101".into()),
                        ("lmt", "320".into()),
                        ("fields1", "f1,f2,f3,f4,f5,f6".into()),
                        ("fields2", "f51,f52,f53,f54,f55,f56,f57".into()),
                    ],
                )
                .await
            } else {
                get(&state.sources.client, "https://quotes.sina.cn/cn/api/jsonp_v2.php/var%20_data=/CN_MarketDataService.getKLineData", &[("symbol",code.clone()),("scale",scale.into()),("ma","no".into()),("datalen","320".into())]).await
            };
            match result.and_then(|value| parse_minutes(&value, source)) {
                Ok(bars) => return Ok((serde_json::to_value(bars).unwrap(), source.into())),
                Err(error) => errors.push(format!("{source}: {error}")),
            }
        }
    }
    Err(errors.join(" | "))
}

pub async fn cached_fetch(
    state: &AppState,
    symbol: &str,
    dataset: &str,
    ttl: i64,
) -> Result<Value, String> {
    symbol_code(symbol)?;
    let now = chrono::Utc::now().timestamp();
    let previous = {
        let conn = state.db.lock().map_err(|_| "数据库忙")?;
        let row: Option<(Option<String>,String,i64,i64,i64,String)> = conn.query_row("SELECT payload,source,fetched_at,attempted_at,failures,error FROM live_cache WHERE symbol=?1 AND dataset=?2",params![symbol,dataset],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?))).optional().map_err(|error|error.to_string())?;
        if let Some((payload, source, fetched, attempted, failures, error)) = &row {
            if now - fetched < ttl || *failures >= 5 || now - attempted < 60 {
                return payload.as_ref().and_then(|payload|serde_json::from_str::<Value>(payload).ok()).map(|data|json!({"data":data,"source":source,"updated_at":fetched,"stale":now-fetched>=ttl,"error":error,"retry_count":failures})).ok_or_else(||format!("{error}（已重试 {failures}/5 次）"));
            }
        }
        let claimed = conn.execute("INSERT INTO live_cache(symbol,dataset,attempted_at) VALUES(?1,?2,?3) ON CONFLICT(symbol,dataset) DO UPDATE SET attempted_at=excluded.attempted_at WHERE live_cache.attempted_at <= ?3-60",params![symbol,dataset,now]).map_err(|error|error.to_string())?;
        if claimed == 0 {
            return Err("数据正在更新，请稍后重试".into());
        }
        row
    };
    let result = if let Some(scale) = dataset.strip_prefix("minute_") {
        minute_fetch(state, symbol, scale).await
    } else {
        enrichment(state, symbol, dataset).await
    };
    let conn = state.db.lock().map_err(|_| "数据库忙")?;
    match result {
        Ok((data, source)) => {
            if dataset == "snapshot" {
                let snapshot: crate::sources::Snapshot =
                    serde_json::from_value(data.clone()).map_err(|error| error.to_string())?;
                let source_id = match source.as_str() {
                    "tencent" => crate::sources::SourceId::Tencent,
                    "sina" => crate::sources::SourceId::Sina,
                    _ => crate::sources::SourceId::Eastmoney,
                };
                crate::sources::store::publish_snapshot(
                    &conn,
                    &crate::sources::SourceBatch {
                        dataset: "snapshot".into(),
                        source: source_id,
                        attempts: 1,
                        data: snapshot.clone(),
                        adj_source: None,
                        adj_error: None,
                    },
                    "minute-refresh",
                )
                .map_err(|error| error.message)?;
                let bar = crate::sources::DailyBar {
                    symbol: symbol.into(),
                    date: snapshot.date,
                    open: snapshot.open,
                    high: snapshot.high,
                    low: snapshot.low,
                    close: snapshot.close,
                    volume: snapshot.volume,
                    amount: snapshot.amount,
                    amount_known: true,
                    adj_factor: None,
                };
                let date = bar.date.clone();
                let batch = crate::sources::SourceBatch {
                    dataset: "quote_daily".into(),
                    source: source_id,
                    attempts: 1,
                    data: vec![bar],
                    adj_source: None,
                    adj_error: None,
                };
                if matches!(
                    symbol,
                    "000001.SH" | "000300.SH" | "000905.SH" | "399001.SZ" | "399006.SZ"
                ) {
                    let _ = crate::sources::store::publish_index(
                        &conn,
                        &batch,
                        &[symbol.into()],
                        &date,
                        &date,
                        "minute-refresh",
                    );
                } else {
                    let _ = crate::sources::store::publish_daily(
                        &conn,
                        &batch,
                        &[symbol.into()],
                        &date,
                        &date,
                        "minute-refresh",
                    );
                }
            }
            conn.execute("UPDATE live_cache SET payload=?3,source=?4,fetched_at=?5,failures=0,error='' WHERE symbol=?1 AND dataset=?2",params![symbol,dataset,data.to_string(),source,now]).map_err(|error|error.to_string())?;
            Ok(
                json!({"data":data,"source":source,"updated_at":now,"stale":false,"error":"","retry_count":0}),
            )
        }
        Err(error) => {
            conn.execute(
                "UPDATE live_cache SET failures=failures+1,error=?3 WHERE symbol=?1 AND dataset=?2",
                params![symbol, dataset, error],
            )
            .map_err(|error| error.to_string())?;
            if let Some((Some(payload), source, fetched, _, failures, _)) = previous {
                Ok(
                    json!({"data":serde_json::from_str::<Value>(&payload).unwrap_or(Value::Null),"source":source,"updated_at":fetched,"stale":true,"error":error,"retry_count":failures+1}),
                )
            } else {
                Err(error)
            }
        }
    }
}

pub async fn minutes(
    state: &AppState,
    symbol: &str,
    name: &str,
    period: &str,
) -> Result<BarsResponse, String> {
    let scale = period.strip_suffix('m').ok_or("分钟周期错误")?;
    let result = cached_fetch(state, symbol, &format!("minute_{scale}"), 60).await?;
    let bars: Vec<Bar> =
        serde_json::from_value(result["data"].clone()).map_err(|error| error.to_string())?;
    Ok(BarsResponse {
        symbol: symbol.into(),
        name: name.into(),
        period: period.into(),
        adjust: "none".into(),
        adjust_available: false,
        notice: Some(format!(
            "{} · 每60秒更新 · 分钟线不复权{}",
            result["source"].as_str().unwrap_or(""),
            if result["stale"] == true {
                " · 更新失败，保留上次数据"
            } else {
                ""
            }
        )),
        as_of: bars.last().map(|bar| bar.date.clone()).unwrap_or_default(),
        bars,
    })
}

async fn enrichment(
    state: &AppState,
    symbol: &str,
    dataset: &str,
) -> Result<(Value, String), String> {
    if dataset == "sectors" {
        return crate::sources::sectors::fetch(&state.sources.client).await;
    }
    if dataset == "popularity" {
        return crate::sources::popularity::fetch(state).await;
    }
    if dataset == "news" {
        return crate::sources::news::fetch(state).await;
    }
    if dataset == "snapshot" {
        return state
            .sources
            .fetch_snapshot(symbol)
            .await
            .map(|batch| {
                (
                    serde_json::to_value(batch.data).unwrap(),
                    batch.source.as_str().into(),
                )
            })
            .map_err(|error| error.message());
    }
    if dataset == "metrics" {
        let mut errors = Vec::new();
        for source in ["eastmoney", "tencent"] {
            for _attempt in 0..2 {
                let result = if source == "eastmoney" {
                    let secid = format!(
                        "{}.{}",
                        if symbol.ends_with(".SH") { 1 } else { 0 },
                        &symbol[..6]
                    );
                    get(
                        &state.sources.client,
                        "https://push2.eastmoney.com/api/qt/stock/get",
                        &[
                            ("secid", secid),
                            ("fields", "f43,f116,f117,f162,f167,f168,f84,f85".into()),
                        ],
                    )
                    .await
                    .and_then(|value| parse_metrics(&value["data"]))
                } else {
                    tencent_metrics(&state.sources.client, symbol).await
                };
                match result {
                    Ok(value) => return Ok((value, source.into())),
                    Err(error) => errors.push(format!("{source}: {error}")),
                }
            }
        }
        return Err(errors.join(" | "));
    }
    let report = match dataset {
        "profile" => "RPT_F10_ORG_BASICINFO",
        "financials" => "RPT_DMSK_FN_INCOME",
        _ => return Err("未知补充数据集".into()),
    };
    let mut last_error = String::new();
    for host in [
        "https://datacenter.eastmoney.com/securities/api/data/v1/get",
        "https://datacenter-web.eastmoney.com/api/data/v1/get",
    ] {
        let result = get(
            &state.sources.client,
            host,
            &[
                ("reportName", report.into()),
                ("columns", "ALL".into()),
                ("filter", format!("(SECUCODE=\"{symbol}\")")),
                (
                    "pageSize",
                    if dataset == "profile" { "1" } else { "8" }.into(),
                ),
                (
                    "sortColumns",
                    if dataset == "profile" {
                        "SECURITY_CODE"
                    } else {
                        "REPORT_DATE"
                    }
                    .into(),
                ),
                ("sortTypes", "-1".into()),
            ],
        )
        .await;
        match result {
            Ok(value) => {
                if let Some(rows) = value["result"]["data"]
                    .as_array()
                    .filter(|rows| !rows.is_empty())
                {
                    let mut data = if dataset == "profile" {
                        rows[0].clone()
                    } else {
                        Value::Array(rows.clone())
                    };
                    if dataset == "financials" {
                        let (balance, cashflow, main_fields) = tokio::join!(
                            financial_table(state, symbol, "RPT_DMSK_FN_BALANCE"),
                            financial_table(state, symbol, "RPT_DMSK_FN_CASHFLOW"),
                            financial_table(state, symbol, "RPT_F10_FINANCE_MAINFINADATA")
                        );
                        for extra in [balance, cashflow, main_fields].into_iter().flatten() {
                            for row in data.as_array_mut().unwrap() {
                                if let Some(matching) = extra
                                    .iter()
                                    .find(|extra| extra["REPORT_DATE"] == row["REPORT_DATE"])
                                {
                                    for (key, value) in matching.as_object().unwrap() {
                                        if !value.is_null() {
                                            row[key] = value.clone();
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return Ok((data, "eastmoney".into()));
                } else {
                    last_error = "数据源暂无资料".into();
                }
            }
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

pub fn spawn_updates(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let _ = cached_fetch(&state, "000001.SH", "sectors", 60).await;
            let popularity_state = state.clone();
            tokio::spawn(async move {
                let _ = cached_fetch(&popularity_state, "000001.SH", "popularity", 60).await;
            });
            if let Ok(conn) = state.db.lock() {
                let _ = crate::market::refresh(&conn);
            }
            let symbols = {
                let Ok(conn) = state.db.lock() else { continue };
                let Ok(mut stmt)=conn.prepare("SELECT symbol FROM (SELECT DISTINCT symbol FROM quote_daily WHERE source <> 'demo' UNION SELECT symbol FROM watchlist_item UNION SELECT code FROM index_catalog)") else {continue};
                let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) else {
                    continue;
                };
                rows.filter_map(Result::ok).collect::<Vec<_>>()
            };
            for chunk in symbols.chunks(8) {
                let mut jobs = tokio::task::JoinSet::new();
                for symbol in chunk {
                    let state = state.clone();
                    let symbol = symbol.clone();
                    jobs.spawn(async move {
                        let _ = cached_fetch(&state, &symbol, "snapshot", 60).await;
                    });
                }
                while jobs.join_next().await.is_some() {}
            }
            if let Ok(conn) = state.db.lock() {
                let _ = crate::market::refresh(&conn);
            }
        }
    });
}

async fn financial_table(
    state: &AppState,
    symbol: &str,
    report: &str,
) -> Result<Vec<Value>, String> {
    let mut last_error = String::new();
    for host in [
        "https://datacenter.eastmoney.com/securities/api/data/v1/get",
        "https://datacenter-web.eastmoney.com/api/data/v1/get",
    ] {
        match get(
            &state.sources.client,
            host,
            &[
                ("reportName", report.into()),
                ("columns", "ALL".into()),
                ("filter", format!("(SECUCODE=\"{symbol}\")")),
                ("pageSize", "8".into()),
                ("sortColumns", "REPORT_DATE".into()),
                ("sortTypes", "-1".into()),
            ],
        )
        .await
        {
            Ok(value) => {
                if let Some(rows) = value["result"]["data"]
                    .as_array()
                    .filter(|rows| !rows.is_empty())
                {
                    return Ok(rows.clone());
                }
            }
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

pub fn parse_metrics(value: &Value) -> Result<Value, String> {
    let scaled = |key: &str, scale: f64| numeric(&value[key]).map(|number| number / scale);
    let result = json!({"total_mv":scaled("f116",1.0),"float_mv":scaled("f117",1.0),"pe_ttm":scaled("f162",100.0),"pb":scaled("f167",100.0),"turnover":scaled("f168",100.0),"total_shares":scaled("f84",1.0),"float_shares":scaled("f85",1.0)});
    if result["total_mv"].as_f64().unwrap_or(0.0) <= 0.0 {
        return Err("估值数据为空".into());
    }
    Ok(result)
}

async fn tencent_metrics(client: &reqwest::Client, symbol: &str) -> Result<Value, String> {
    let bytes = client
        .get(format!("https://qt.gtimg.cn/q={}", symbol_code(symbol)?))
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .bytes()
        .await
        .map_err(|error| error.to_string())?;
    let (body, _, _) = encoding_rs::GB18030.decode(&bytes);
    let payload = body
        .split_once("=\"")
        .and_then(|(_, rest)| rest.rsplit_once('"'))
        .map(|(payload, _)| payload)
        .ok_or("腾讯估值格式错误")?;
    let fields = payload.split('~').collect::<Vec<_>>();
    let number = |index: usize| {
        fields
            .get(index)
            .and_then(|field| field.parse::<f64>().ok())
            .filter(|number| number.is_finite())
    };
    let total = number(45)
        .filter(|number| *number > 0.0)
        .ok_or("腾讯市值为空")?
        * 1e8;
    Ok(
        json!({"total_mv":total,"float_mv":number(44).map(|number|number*1e8),"pe_ttm":number(52),"pb":number(46),"turnover":number(38)}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn minute_time_and_volume_are_normalized() {
        let bars=parse_minutes(&json!([{"day":"2026-10-22 09:35:00","open":"10","high":"12","low":"9","close":"11","volume":"100","amount":"1100"}]),"sina").unwrap();
        assert_eq!(bars[0].volume, 100.0);
        assert_eq!(bars[0].date, "1792632900");
        assert!(parse_minutes(&json!([{"day":"bad"}]), "sina").is_err());
    }
    #[test]
    fn missing_metrics_stay_null_and_units_are_correct() {
        let value = parse_metrics(&json!({"f116":100000000,"f162":1848,"f167":"-"})).unwrap();
        assert_eq!(value["pe_ttm"], 18.48);
        assert!(value["pb"].is_null());
        assert!(parse_metrics(&json!({})).is_err());
        assert!(symbol_code("600519.SH&x=1").is_err());
    }

    #[test]
    fn eastmoney_minute_fields_and_lot_units_are_normalized() {
        let bars = parse_minutes(
            &json!({"data":{"klines":["2026-10-22 09:35,10,11,12,9,100,110000"]}}),
            "eastmoney",
        )
        .unwrap();
        assert_eq!(bars[0].open, 10.0);
        assert_eq!(bars[0].close, 11.0);
        assert_eq!(bars[0].volume, 10000.0);
        assert!(parse_minutes(&json!([]), "sina").is_err());
    }

    #[tokio::test]
    async fn exhausted_cache_preserves_payload_without_network() {
        let state = crate::db::test_state();
        {
            let conn = state.db.lock().unwrap();
            conn.execute("INSERT INTO live_cache(symbol,dataset,payload,source,fetched_at,failures,error) VALUES('600519.SH','metrics','{\"pb\":6.55}','eastmoney',1,5,'upstream down')",[]).unwrap();
        }
        let result = cached_fetch(&state, "600519.SH", "metrics", 60)
            .await
            .unwrap();
        assert_eq!(result["data"]["pb"], 6.55);
        assert_eq!(result["stale"], true);
        assert_eq!(result["retry_count"], 5);
        let conn = state.db.lock().unwrap();
        let attempts: i64 = conn
            .query_row(
                "SELECT failures FROM live_cache WHERE symbol='600519.SH'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(attempts, 5);
    }
}
