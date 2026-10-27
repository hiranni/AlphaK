//! Aggregate market statistics only; never downloads or scans the stock universe.
use crate::{
    db::AppState,
    error::{envelope, ApiResponse, AppError},
};
use axum::{extract::State, Json};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct MarketBreadth {
    advance: u32,
    decline: u32,
    flat: u32,
    suspended: u32,
    limit_up: u32,
    limit_down: u32,
    as_of: String,
    source: String,
    scope: String,
}
#[derive(Serialize)]
pub struct BreadthFeed {
    data: Option<MarketBreadth>,
    stale: bool,
    error: String,
}

fn parse(html: &str) -> Result<MarketBreadth, String> {
    let time = html
        .split("class=\"market-activity-time\"")
        .nth(1)
        .and_then(|s| s.split_once('>'))
        .and_then(|(_, s)| s.split('<').next())
        .map(str::trim)
        .ok_or("缺少统计时间")?;
    chrono::NaiveDateTime::parse_from_str(time, "%Y-%m-%d %H:%M:%S")
        .map_err(|_| "统计时间格式无效")?;
    let table = html
        .split("market-activity-chart-right-side-table")
        .nth(1)
        .and_then(|s| s.split("</table>").next())
        .ok_or("缺少涨跌统计表")?;
    let cells: Vec<&str> = table
        .split("<td")
        .skip(1)
        .filter_map(|s| {
            s.split_once('>')
                .and_then(|(_, value)| value.split('<').next())
                .map(str::trim)
        })
        .collect();
    let count = |label: &str| -> Result<u32, String> {
        cells
            .windows(2)
            .find(|pair| pair[0] == label)
            .and_then(|pair| pair[1].parse::<u32>().ok())
            .ok_or_else(|| format!("缺少或无效的{label}统计"))
    };
    let data = MarketBreadth {
        advance: count("上涨")?,
        decline: count("下跌")?,
        flat: count("平盘")?,
        suspended: count("停牌")?,
        limit_up: count("涨停")?,
        limit_down: count("跌停")?,
        as_of: time.into(),
        source: "乐咕乐股".into(),
        scope: "沪深市场聚合统计，停牌不计入涨跌占比".into(),
    };
    let total = u64::from(data.advance) + u64::from(data.decline) + u64::from(data.flat);
    if !(1000..=20000).contains(&total) {
        return Err("市场统计数量异常".into());
    }
    Ok(data)
}

async fn fetch() -> Result<MarketBreadth, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .user_agent("Mozilla/5.0 AlphaK/0.1")
        .build()
        .map_err(|e| e.to_string())?;
    // Same upstream, alternative hostname for transport failures; not an independent data source.
    let mut last = String::new();
    if crate::sources::axdata::enabled() {
        match crate::sources::axdata::request(
            &client,
            "cls_market_emotion",
            serde_json::json!({}),
            &[],
        )
        .await
        {
            Ok(rows) => {
                if let Some(row) = rows.first() {
                    let count = |key: &str| row[key].as_u64().and_then(|v| u32::try_from(v).ok());
                    if let (Some(advance), Some(decline), Some(flat)) =
                        (count("rise_num"), count("fall_num"), count("flat_num"))
                    {
                        if (1000..=20000)
                            .contains(&(u64::from(advance) + u64::from(decline) + u64::from(flat)))
                        {
                            return Ok(MarketBreadth {
                                advance,
                                decline,
                                flat,
                                suspended: row["raw_up_down_dis"]["suspend_num"]
                                    .as_u64()
                                    .unwrap_or(0) as u32,
                                limit_up: count("up_num").unwrap_or(0),
                                limit_down: count("down_num").unwrap_or(0),
                                as_of: String::new(),
                                source: "AxData · 财联社".into(),
                                scope: "A股聚合快照；源未提供行情时间，时间未知".into(),
                            });
                        }
                    }
                }
            }
            Err(error) => last = error,
        }
    }
    for host in ["legulegu.com", "www.legulegu.com"] {
        let result = async {
            let html = client
                .get(format!("https://{host}/stockdata/market-activity"))
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?
                .text()
                .await
                .map_err(|e| e.to_string())?;
            parse(&html)
        }
        .await;
        match result {
            Ok(data) => return Ok(data),
            Err(err) => last = err,
        }
    }
    Err(last)
}

pub async fn get(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<BreadthFeed>>, AppError> {
    static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = LOCK.lock().await;
    let now = chrono::Utc::now().timestamp();
    let cached: Option<(Option<String>, i64, i64, String)> = {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::internal("数据库锁不可用"))?;
        conn.query_row("SELECT payload,fetched_at,attempted_at,error FROM live_cache WHERE symbol='MARKET' AND dataset='breadth'", [],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?
    };
    let (payload, fetched, attempted, error) = cached.unwrap_or((None, 0, 0, String::new()));
    let old = payload.and_then(|p| serde_json::from_str::<MarketBreadth>(&p).ok());
    if (old.is_some() && now - fetched < 120 && error.is_empty()) || now - attempted < 30 {
        return Ok(envelope(BreadthFeed {
            data: old,
            stale: !error.is_empty() || now - fetched >= 120,
            error,
        }));
    }
    match fetch().await {
        Ok(data) => {
            let payload =
                serde_json::to_string(&data).map_err(|e| AppError::internal(e.to_string()))?;
            {
                let conn = state
                    .db
                    .lock()
                    .map_err(|_| AppError::internal("数据库锁不可用"))?;
                let tx = conn.unchecked_transaction()?;
                if !data.as_of.is_empty() {
                    crate::persistence::observe_breadth(
                        &tx,
                        &serde_json::to_value(&data)
                            .map_err(|e| AppError::internal(e.to_string()))?,
                        now,
                    )?;
                }
                tx.execute(
                "INSERT INTO live_cache(symbol,dataset,payload,source,fetched_at,attempted_at) VALUES('MARKET','breadth',?1,?3,?2,?2) ON CONFLICT(symbol,dataset) DO UPDATE SET payload=excluded.payload,source=excluded.source,fetched_at=excluded.fetched_at,attempted_at=excluded.attempted_at,failures=0,error=''", params![payload,now,data.source])?;
                tx.commit()?;
            }
            Ok(envelope(BreadthFeed {
                data: Some(data),
                stale: false,
                error: String::new(),
            }))
        }
        Err(error) => {
            state.db.lock().map_err(|_| AppError::internal("数据库锁不可用"))?.execute(
                "INSERT INTO live_cache(symbol,dataset,attempted_at,failures,error) VALUES('MARKET','breadth',?1,1,?2) ON CONFLICT(symbol,dataset) DO UPDATE SET attempted_at=excluded.attempted_at,failures=failures+1,error=excluded.error",params![now,error])?;
            Ok(envelope(BreadthFeed {
                data: old,
                stale: true,
                error: "大盘统计更新失败，稍后自动重试".into(),
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_aggregate_and_rejects_broken_or_partial_pages() {
        let html = r#"<span class="market-activity-time">2026-10-22 15:00:00</span><div class="market-activity-chart-right-side-table"><table><td>上涨</td><td class="red">447</td><td>下跌</td><td>4739</td><td>平盘</td><td>32</td><td>涨停</td><td>33</td><td>跌停</td><td>62</td><td>停牌</td><td>7</td></table>"#;
        let data = parse(html).unwrap();
        assert_eq!(
            (data.advance, data.decline, data.flat, data.suspended),
            (447, 4739, 32, 7)
        );
        assert!(parse(&html.replace("4739", "0")).is_err());
        assert!(parse(&html.replace("<td>平盘</td><td>32</td>", "")).is_err());
        assert!(parse(&html.replace("2026-10-22", "invalid")).is_err());
    }
}

/// Fetch only two aggregate index series, never constituent quotes or a market scan.
pub fn refresh_history(state: AppState, period: &str) {
    static LAST: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, i64>>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if !crate::sources::axdata::enabled() {
        return;
    }
    let dataset = match period {
        "30min" => "30m",
        "1h" => "60m",
        _ => "day",
    }
    .to_string();
    let now = chrono::Utc::now().timestamp();
    {
        let Ok(mut last) = LAST.lock() else { return };
        if now - last.get(&dataset).copied().unwrap_or(0) < 120 {
            return;
        }
        last.insert(dataset.clone(), now);
    }
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let fetch = |code: &str| {
            crate::sources::axdata::request(
                &client,
                "index_kline_tdx",
                serde_json::json!({"code":code,"period":dataset,"count":240}),
                &["trade_time", "up_count", "down_count"],
            )
        };
        let (Ok(sh), Ok(sz)) = tokio::join!(fetch("000001.SH"), fetch("399107.SZ")) else {
            return;
        };
        let points = combine_history(&sh, &sz, &dataset);
        if let Ok(conn) = state.db.lock() {
            if let Ok(tx) = conn.unchecked_transaction() {
                for point in points {
                    if let Err(error) = crate::persistence::observe_breadth(&tx, &point, now) {
                        tracing::warn!(error=?error,"breadth observation rejected");
                    }
                }
                let _ = tx.commit();
            }
        }
    });
}
fn combine_history(
    sh: &[serde_json::Value],
    sz: &[serde_json::Value],
    period: &str,
) -> Vec<serde_json::Value> {
    sh.iter().filter_map(|a| {
        let raw=a["trade_time"].as_str()?;
        let b=sz.iter().find(|b| b["trade_time"].as_str()==Some(raw))?;
        let advance=a["up_count"].as_u64()?+b["up_count"].as_u64()?;
        let decline=a["down_count"].as_u64()?+b["down_count"].as_u64()?;
        if !(1000..=20000).contains(&(advance+decline)) {return None}
        let time=chrono::DateTime::parse_from_rfc3339(raw).ok()?.format("%Y-%m-%d %H:%M:%S").to_string();
        Some(serde_json::json!({"advance":advance,"decline":decline,"flat":0,"flat_known":false,"as_of":time,"source":format!("AxData · 通达信 · {period}"),"scope":"沪深指数携带的涨跌家数合计；平盘未知，不含北交所"}))
    }).collect()
}

#[cfg(test)]
mod history_tests {
    use super::*;
    #[test]
    fn aggregate_requires_matching_time_and_valid_counts() {
        let sh = vec![
            serde_json::json!({"trade_time":"2026-10-23T15:00:00+08:00","up_count":1800,"down_count":400}),
        ];
        let sz = vec![
            serde_json::json!({"trade_time":"2026-10-23T15:00:00+08:00","up_count":2400,"down_count":500}),
        ];
        let result = combine_history(&sh, &sz, "day");
        assert_eq!(result[0]["advance"], 4200);
        assert_eq!(result[0]["decline"], 900);
        assert_eq!(result[0]["flat_known"], false);
        assert_eq!(result[0]["as_of"], "2026-10-23 15:00:00");
        assert!(combine_history(&sh, &[], "day").is_empty());
    }
}
