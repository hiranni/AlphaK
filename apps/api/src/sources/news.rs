use crate::db::AppState;
use serde_json::{json, Value};
use std::collections::HashSet;

const ENDPOINT: &str = "https://feed.mix.sina.com.cn/api/roll/get";
const LIMIT: usize = 12;

pub async fn fetch(state: &AppState) -> Result<(Value, String), String> {
    let mut error = String::new();
    for _attempt in 0..2 {
        let result = state
            .sources
            .client
            .get(ENDPOINT)
            .header("Referer", "https://finance.sina.com.cn/")
            .query(&[
                ("pageid", "153"),
                ("lid", "2517"),
                ("k", ""),
                ("num", "20"),
                ("page", "1"),
            ])
            .send()
            .await;
        match result {
            Ok(response) => match response.error_for_status() {
                Ok(response) => match response.json::<Value>().await {
                    Ok(value) => match parse(&value) {
                        Ok(rows) => return Ok((Value::Array(rows), "sina".into())),
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
    let rows = value["result"]["data"]
        .as_array()
        .ok_or("新闻列表暂无数据")?;
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    for row in rows {
        let title = row["title"].as_str().unwrap_or("").trim();
        let url = row["url"].as_str().unwrap_or("").trim();
        if title.is_empty() || !(url.starts_with("https://") || url.starts_with("http://")) {
            continue;
        }
        let published_at = row["ctime"]
            .as_str()
            .and_then(|value| value.parse::<i64>().ok())
            .or_else(|| row["ctime"].as_i64())
            .unwrap_or(0);
        if published_at <= 0 {
            continue;
        }
        let id = row["docid"].as_str().unwrap_or(url).to_string();
        if !seen.insert(id.clone()) {
            continue;
        }
        items.push(json!({
            "id": id,
            "title": title,
            "url": url,
            "source": row["media_name"].as_str().unwrap_or("新浪财经"),
            "published_at": published_at,
            "summary": row["intro"].as_str().unwrap_or("").trim(),
        }));
        if items.len() >= LIMIT {
            break;
        }
    }
    if items.is_empty() {
        return Err("新闻列表没有有效条目".into());
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"{"result":{"data":[
        {"docid":"a","title":"第一条 热点新闻","url":"https://finance.sina.com.cn/a.shtml","ctime":"1792660970","media_name":"证券时报","intro":"摘要"},
        {"docid":"a","title":"重复条目","url":"https://finance.sina.com.cn/a.shtml","ctime":"1792660971"},
        {"docid":"b","title":"  ","url":"https://finance.sina.com.cn/b.shtml","ctime":"1792660972"},
        {"docid":"c","title":"缺少链接","url":"","ctime":"1792660973"},
        {"docid":"d","title":"缺少时间","url":"https://finance.sina.com.cn/d.shtml","ctime":""},
        {"docid":"e","title":"第二条 热点新闻","url":"http://finance.sina.com.cn/e.shtml","ctime":1792660974}
    ]}}"#;

    #[test]
    fn news_keeps_valid_unique_items_in_feed_order() {
        let rows = parse(&serde_json::from_str::<Value>(FEED).unwrap()).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["title"], "第一条 热点新闻");
        assert_eq!(rows[0]["source"], "证券时报");
        assert_eq!(rows[0]["published_at"], 1792660970_i64);
        assert_eq!(rows[1]["id"], "e");
    }

    #[test]
    fn news_rejects_empty_feed() {
        assert!(parse(&json!({"result":{"data":[]}})).is_err());
        assert!(parse(&json!({})).is_err());
    }
}
