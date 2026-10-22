use crate::models::SectorRow;
use serde_json::Value;

fn sector_id(code: &str) -> i64 {
    -1 - (code.bytes().fold(2166136261u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(16777619)
    }) as i64)
}

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|number| number.is_finite())
}

fn symbol(code: &str) -> String {
    if code.len() == 8
        && matches!(&code[..2], "sh" | "sz" | "bj")
        && code[2..].bytes().all(|byte| byte.is_ascii_digit())
    {
        format!("{}.{}", &code[2..], code[..2].to_uppercase())
    } else if code.len() == 6 && code.bytes().all(|byte| byte.is_ascii_digit()) {
        format!(
            "{code}.{}",
            if code.starts_with('6') {
                "SH"
            } else if code.starts_with('8') || code.starts_with('4') {
                "BJ"
            } else {
                "SZ"
            }
        )
    } else {
        String::new()
    }
}

pub fn parse_sina(body: &str) -> Result<Vec<SectorRow>, String> {
    let start = body.find('{').ok_or("新浪行业汇总为空")?;
    let end = body.rfind('}').ok_or("新浪行业汇总格式错误")?;
    let value: Value =
        serde_json::from_str(&body[start..=end]).map_err(|error| error.to_string())?;
    let mut sectors = Vec::new();
    for (code, value) in value.as_object().ok_or("新浪行业汇总格式错误")? {
        let fields = value.as_str().unwrap_or("").split(',').collect::<Vec<_>>();
        if fields.len() < 13 {
            continue;
        }
        let Some(change) = fields[5]
            .parse::<f64>()
            .ok()
            .filter(|change| change.is_finite())
        else {
            continue;
        };
        let Some(amount) = fields[7]
            .parse::<f64>()
            .ok()
            .filter(|amount| amount.is_finite() && *amount >= 0.0)
        else {
            continue;
        };
        if fields[1].is_empty() {
            continue;
        }
        sectors.push(SectorRow {
            id: sector_id(code),
            name: fields[1].into(),
            sector_type: "industry".into(),
            change_pct: change,
            leading_symbol: symbol(fields[8]),
            leading_name: fields[12].into(),
            turnover: amount,
        });
    }
    if sectors.is_empty() {
        return Err("新浪行业汇总无有效数据".into());
    }
    Ok(sectors)
}

pub fn parse_eastmoney(value: &Value) -> Result<Vec<SectorRow>, String> {
    let rows = value["data"]["diff"]
        .as_array()
        .ok_or("东方财富行业汇总为空")?;
    let mut sectors = Vec::new();
    for row in rows {
        let Some(change) = number(&row["f3"]) else {
            continue;
        };
        let Some(amount) = number(&row["f6"]).filter(|amount| *amount >= 0.0) else {
            continue;
        };
        let Some(name) = row["f14"].as_str().filter(|name| !name.is_empty()) else {
            continue;
        };
        let Some(code) = row["f12"].as_str() else {
            continue;
        };
        sectors.push(SectorRow {
            id: sector_id(code),
            name: name.into(),
            sector_type: "industry".into(),
            change_pct: change,
            leading_symbol: symbol(row["f140"].as_str().unwrap_or("")),
            leading_name: row["f128"].as_str().unwrap_or("").into(),
            turnover: amount,
        });
    }
    if sectors.is_empty() {
        return Err("东方财富行业汇总无有效数据".into());
    }
    Ok(sectors)
}

pub async fn fetch(client: &reqwest::Client) -> Result<(Value, String), String> {
    let mut errors = Vec::new();
    for source in ["eastmoney", "sina"] {
        for _attempt in 0..2 {
            let request = if source == "eastmoney" {
                client
                    .get("https://push2.eastmoney.com/api/qt/clist/get")
                    .header("Referer", "https://quote.eastmoney.com/")
                    .query(&[
                        ("pn", "1"),
                        ("pz", "100"),
                        ("po", "1"),
                        ("np", "1"),
                        ("fltt", "2"),
                        ("invt", "2"),
                        ("fid", "f3"),
                        ("fs", "m:90+t:2"),
                        ("fields", "f12,f14,f3,f6,f128,f140"),
                    ])
            } else {
                client
                    .get("https://vip.stock.finance.sina.com.cn/q/view/newSinaHy.php")
                    .header("Referer", "https://finance.sina.com.cn/")
            };
            let result = async {
                let bytes = request
                    .send()
                    .await
                    .map_err(|error| error.to_string())?
                    .error_for_status()
                    .map_err(|error| error.to_string())?
                    .bytes()
                    .await
                    .map_err(|error| error.to_string())?;
                if source == "sina" {
                    let (body, _, _) = encoding_rs::GB18030.decode(&bytes);
                    parse_sina(&body)
                } else {
                    parse_eastmoney(
                        &serde_json::from_slice(&bytes).map_err(|error| error.to_string())?,
                    )
                }
            }
            .await;
            match result {
                Ok(mut sectors) => {
                    sectors.sort_by(|left, right| right.change_pct.total_cmp(&left.change_pct));
                    return Ok((
                        serde_json::to_value(sectors).map_err(|error| error.to_string())?,
                        source.into(),
                    ));
                }
                Err(error) => errors.push(format!("{source}: {error}")),
            }
        }
    }
    Err(errors.join(" | "))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sector_percentage_and_amount_are_taken_from_summary() {
        let rows=parse_sina(r#"var data={"new_blhy":"new_blhy,玻璃行业,19,14.62,-0.53,-3.51,405889728,7788005569,sh600293,10.105,5.23,0.48,三峡新材"};"#).unwrap();
        assert_eq!(rows[0].change_pct, -3.51);
        assert_eq!(rows[0].turnover, 7788005569.0);
        assert_eq!(rows[0].leading_symbol, "600293.SH");
        assert!(rows[0].id < 0);
        assert!(parse_sina("var data={};").is_err());
        assert!(parse_eastmoney(
            &serde_json::json!({"data":{"diff":[{"f12":"BK1","f14":"行业","f3":"-","f6":100}]}})
        )
        .is_err());
    }
}
