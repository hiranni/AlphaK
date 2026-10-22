use crate::db::AppState;
use serde_json::{json, Value};

pub async fn fetch(state: &AppState) -> Result<(Value, String), String> {
    let mut error = String::new();
    for _attempt in 0..2 {
        let result=state.sources.client.post("https://emappdata.eastmoney.com/stockrank/getAllCurrentList").json(&json!({"appId":"appId01","globalId":"786e4c21-70dc-435a-93bb-38","marketType":"","pageNo":1,"pageSize":12})).send().await;
        match result {
            Ok(response) => match response.error_for_status() {
                Ok(response) => match response.json::<Value>().await {
                    Ok(value) => match parse(&value) {
                        Ok(mut rows) => {
                            for row in &mut rows {
                                let symbol = row["symbol"].as_str().unwrap().to_string();
                                if let Ok(batch) = state.sources.fetch_snapshot(&symbol).await {
                                    row["name"] = json!(batch.data.name);
                                    row["close"] = json!(batch.data.close);
                                    row["change_pct"] = json!(batch.data.change_pct);
                                    let conn = state.db.lock().map_err(|_| "数据库忙")?;
                                    let exchange = symbol.split_once('.').unwrap().1;
                                    conn.execute("INSERT OR IGNORE INTO instruments(symbol,name,pinyin,exchange,board,industry,list_date,status) VALUES(?1,?2,'',?3,'未知','未知','','active')",rusqlite::params![symbol,row["name"].as_str().unwrap(),exchange]).map_err(|error|error.to_string())?;
                                } else if let Ok(conn) = state.db.lock() {
                                    let name: Option<String> = conn
                                        .query_row(
                                            "SELECT name FROM instruments WHERE symbol=?1",
                                            [&symbol],
                                            |row| row.get(0),
                                        )
                                        .ok();
                                    if let Some(name) = name {
                                        row["name"] = json!(name);
                                    }
                                }
                            }
                            return Ok((Value::Array(rows), "eastmoney".into()));
                        }
                        Err(message) => error = message,
                    },
                    Err(message) => error = message.to_string(),
                },
                Err(message) => error = message.to_string(),
            },
            Err(message) => error = message.to_string(),
        }
    }
    Err(error)
}

pub fn parse(value: &Value) -> Result<Vec<Value>, String> {
    let rows = value["data"].as_array().ok_or("人气榜暂无数据")?;
    let mut result = Vec::new();
    for row in rows.iter().take(12) {
        let code = row["sc"].as_str().unwrap_or("");
        if !code.is_ascii()
            || code.len() != 8
            || !matches!(&code[..2], "SH" | "SZ" | "BJ")
            || !code[2..].bytes().all(|byte| byte.is_ascii_digit())
        {
            continue;
        }
        let Some(rank) = row["rk"].as_u64().filter(|rank| *rank > 0) else {
            continue;
        };
        let symbol = format!("{}.{}", &code[2..], &code[..2]);
        result.push(json!({"symbol":symbol,"name":symbol,"rank":rank,"rank_change":row["hisRc"],"close":null,"change_pct":null}));
    }
    result.sort_by_key(|row| row["rank"].as_u64().unwrap());
    if result.is_empty() {
        return Err("人气榜没有有效排名".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn popularity_uses_provider_rank_and_filters_invalid_symbols() {
        let rows=parse(&json!({"data":[{"sc":"SZ300750","rk":2},{"sc":"SH600519","rk":1},{"sc":"invalid","rk":3}]})).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["symbol"], "600519.SH");
        assert_eq!(rows[1]["rank"], 2);
        assert!(rows[0]["close"].is_null());
    }
}
