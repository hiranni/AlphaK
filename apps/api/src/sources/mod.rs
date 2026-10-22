pub mod eastmoney;
pub mod news;
pub mod popularity;
pub mod retry;
pub mod sectors;
pub mod sina;
pub mod store;
pub mod tencent;

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;

const RETRIES: usize = 2;
const COOLDOWN_BASE_SECS: u64 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceId {
    Eastmoney,
    Tencent,
    Sina,
}

impl SourceId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Eastmoney => "eastmoney",
            Self::Tencent => "tencent",
            Self::Sina => "sina",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Eastmoney => "东方财富",
            Self::Tencent => "腾讯",
            Self::Sina => "新浪财经",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyBar {
    pub symbol: String,
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub amount: f64,
    pub amount_known: bool,
    pub adj_factor: Option<f64>,
}

impl DailyBar {
    pub fn validate(&self) -> Result<(), String> {
        if self.date.len() != 10
            || chrono::NaiveDate::parse_from_str(&self.date, "%Y-%m-%d").is_err()
        {
            return Err(format!("invalid trade date: {}", self.date));
        }
        if self.open <= 0.0 || self.close <= 0.0 || self.low <= 0.0 {
            return Err(format!("non-positive price: {} {}", self.symbol, self.date));
        }
        if self.high + 1e-6 < self.low
            || self.high + 1e-6 < self.open
            || self.high + 1e-6 < self.close
        {
            return Err(format!("OHLC inconsistent: {} {}", self.symbol, self.date));
        }
        if self.volume < 0.0 || self.amount < 0.0 {
            return Err(format!(
                "negative volume/amount: {} {}",
                self.symbol, self.date
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub symbol: String,
    pub name: String,
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub prev_close: f64,
    pub volume: f64,
    pub amount: f64,
    pub change_pct: f64,
}

impl Snapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.date.len() != 10
            || chrono::NaiveDate::parse_from_str(&self.date, "%Y-%m-%d").is_err()
        {
            return Err(format!("invalid snapshot date: {}", self.date));
        }
        if self.close <= 0.0 || self.open <= 0.0 || self.low <= 0.0 || self.prev_close <= 0.0 {
            return Err(format!("non-positive snapshot price: {}", self.symbol));
        }
        if self.high + 1e-6 < self.low {
            return Err(format!("snapshot high < low: {}", self.symbol));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceBatch<T> {
    pub dataset: String,
    pub source: SourceId,
    pub attempts: usize,
    pub data: T,
    pub adj_source: Option<SourceId>,
    pub adj_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProbeResult {
    pub source: SourceId,
    pub ok: bool,
    pub rows: usize,
    pub elapsed_ms: u128,
    pub error: Option<String>,
    pub data: Option<Vec<DailyBar>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthView {
    pub source: SourceId,
    pub available: bool,
    pub failures: u32,
    pub cooldown_seconds: i64,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SourceFailure {
    pub dataset: String,
    pub errors: Vec<String>,
}

impl SourceFailure {
    pub fn message(&self) -> String {
        format!(
            "{} sources failed: {}",
            self.dataset,
            self.errors.join(" | ")
        )
    }
}

struct Health {
    failures: u32,
    cooldown_until: Option<Instant>,
    last_success_at: Option<String>,
    last_error: String,
}

pub struct SourceHub {
    pub client: reqwest::Client,
    health: Mutex<HashMap<SourceId, Health>>,
}

impl SourceHub {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("AlphaK/0.1 personal-research (+https://127.0.0.1)")
            .timeout(Duration::from_secs(12))
            .build()
            .expect("build http client");
        Self {
            client,
            health: Mutex::new(HashMap::new()),
        }
    }

    pub fn priority(dataset: &str) -> &'static [SourceId] {
        match dataset {
            "quote_daily" | "snapshot" | "index_quote" => {
                &[SourceId::Tencent, SourceId::Eastmoney, SourceId::Sina]
            }
            _ => &[SourceId::Eastmoney, SourceId::Tencent, SourceId::Sina],
        }
    }

    pub fn health(&self) -> Vec<HealthView> {
        let health = self.health.lock().unwrap();
        [SourceId::Eastmoney, SourceId::Tencent, SourceId::Sina]
            .into_iter()
            .map(|source| {
                let entry = health.get(&source);
                HealthView {
                    source,
                    available: entry
                        .and_then(|value| value.cooldown_until)
                        .map(|until| until <= Instant::now())
                        .unwrap_or(true),
                    failures: entry.map(|value| value.failures).unwrap_or(0),
                    cooldown_seconds: entry
                        .and_then(|value| value.cooldown_until)
                        .map(|until| {
                            until.saturating_duration_since(Instant::now()).as_secs() as i64
                        })
                        .unwrap_or(0),
                    last_success_at: entry.and_then(|value| value.last_success_at.clone()),
                    last_error: entry
                        .filter(|value| !value.last_error.is_empty())
                        .map(|value| value.last_error.clone()),
                }
            })
            .collect()
    }

    fn available(&self, source: SourceId) -> bool {
        let health = self.health.lock().unwrap();
        health
            .get(&source)
            .and_then(|value| value.cooldown_until)
            .map(|until| until <= Instant::now())
            .unwrap_or(true)
    }

    fn record_success(&self, source: SourceId) {
        let mut health = self.health.lock().unwrap();
        let entry = health.entry(source).or_insert(Health {
            failures: 0,
            cooldown_until: None,
            last_success_at: None,
            last_error: String::new(),
        });
        entry.failures = 0;
        entry.cooldown_until = None;
        entry.last_success_at = Some(chrono::Utc::now().to_rfc3339());
        entry.last_error.clear();
    }

    fn record_failure(&self, source: SourceId, error: &str) {
        let mut health = self.health.lock().unwrap();
        let entry = health.entry(source).or_insert(Health {
            failures: 0,
            cooldown_until: None,
            last_success_at: None,
            last_error: String::new(),
        });
        entry.failures += 1;
        let seconds = COOLDOWN_BASE_SECS
            .saturating_mul(entry.failures as u64)
            .min(120);
        entry.cooldown_until = Some(Instant::now() + Duration::from_secs(seconds));
        entry.last_error = error.to_string();
    }

    pub async fn fetch_daily(
        &self,
        dataset: &str,
        symbol: &str,
        start: &str,
        end: &str,
        with_adj: bool,
    ) -> Result<SourceBatch<Vec<DailyBar>>, SourceFailure> {
        let mut errors = Vec::new();
        for source in Self::priority(dataset) {
            if !self.available(*source) {
                errors.push(format!("{}: cooling down", source.as_str()));
                continue;
            }
            for attempt in 1..=RETRIES {
                match self.daily_from(*source, symbol, start, end, with_adj).await {
                    Ok((mut bars, adj_source, adj_error)) => {
                        bars.sort_by(|a, b| a.date.cmp(&b.date));
                        self.record_success(*source);
                        return Ok(SourceBatch {
                            dataset: dataset.to_string(),
                            source: *source,
                            attempts: attempt,
                            data: bars,
                            adj_source,
                            adj_error,
                        });
                    }
                    Err(err) => {
                        errors.push(format!("{} attempt {attempt}: {err}", source.as_str()));
                        if attempt < RETRIES {
                            tokio::time::sleep(Duration::from_millis(250)).await;
                        }
                    }
                }
            }
            self.record_failure(*source, &errors.join(" | "));
        }
        Err(SourceFailure {
            dataset: dataset.to_string(),
            errors,
        })
    }

    pub async fn fetch_daily_from(
        &self,
        source: SourceId,
        dataset: &str,
        symbol: &str,
        start: &str,
        end: &str,
        with_adj: bool,
    ) -> Result<SourceBatch<Vec<DailyBar>>, SourceFailure> {
        let mut errors = Vec::new();
        for attempt in 1..=RETRIES {
            match self.daily_from(source, symbol, start, end, with_adj).await {
                Ok((mut bars, adj_source, adj_error)) => {
                    bars.sort_by(|a, b| a.date.cmp(&b.date));
                    self.record_success(source);
                    return Ok(SourceBatch {
                        dataset: dataset.to_string(),
                        source,
                        attempts: attempt,
                        data: bars,
                        adj_source,
                        adj_error,
                    });
                }
                Err(err) => {
                    errors.push(format!("{} attempt {attempt}: {err}", source.as_str()));
                    if attempt < RETRIES {
                        tokio::time::sleep(Duration::from_millis(250)).await;
                    }
                }
            }
        }
        self.record_failure(source, &errors.join(" | "));
        Err(SourceFailure {
            dataset: dataset.to_string(),
            errors,
        })
    }

    pub async fn fetch_snapshot(
        &self,
        symbol: &str,
    ) -> Result<SourceBatch<Snapshot>, SourceFailure> {
        let mut errors = Vec::new();
        for source in Self::priority("snapshot") {
            if !self.available(*source) {
                errors.push(format!("{}: cooling down", source.as_str()));
                continue;
            }
            for attempt in 1..=RETRIES {
                let result = match source {
                    SourceId::Eastmoney => eastmoney::snapshot(&self.client, symbol).await,
                    SourceId::Tencent => tencent::snapshot(&self.client, symbol).await,
                    SourceId::Sina => sina::snapshot(&self.client, symbol).await,
                };
                match result {
                    Ok(snapshot) => {
                        self.record_success(*source);
                        return Ok(SourceBatch {
                            dataset: "snapshot".into(),
                            source: *source,
                            attempts: attempt,
                            data: snapshot,
                            adj_source: None,
                            adj_error: None,
                        });
                    }
                    Err(err) => {
                        errors.push(format!("{} attempt {attempt}: {err}", source.as_str()));
                        if attempt < RETRIES {
                            tokio::time::sleep(Duration::from_millis(250)).await;
                        }
                    }
                }
            }
            self.record_failure(*source, &errors.join(" | "));
        }
        Err(SourceFailure {
            dataset: "snapshot".into(),
            errors,
        })
    }

    async fn daily_from(
        &self,
        source: SourceId,
        symbol: &str,
        start: &str,
        end: &str,
        with_adj: bool,
    ) -> Result<(Vec<DailyBar>, Option<SourceId>, Option<String>), String> {
        let mut bars = match source {
            SourceId::Eastmoney => eastmoney::daily(&self.client, symbol, start, end, "").await?,
            SourceId::Tencent => tencent::daily(&self.client, symbol, start, end, "").await?,
            SourceId::Sina => sina::daily(&self.client, symbol, start, end).await?,
        };
        if bars.is_empty() {
            return Err("empty daily response".into());
        }
        if !with_adj {
            return Ok((bars, None, None));
        }
        let adjusted = match source {
            SourceId::Eastmoney => eastmoney::daily(&self.client, symbol, start, end, "qfq").await,
            SourceId::Tencent => tencent::daily(&self.client, symbol, start, end, "qfq").await,
            SourceId::Sina => Err("新浪日线不提供复权因子".into()),
        };
        match adjusted {
            Ok(adjusted) => {
                let factors: HashMap<String, f64> = adjusted
                    .into_iter()
                    .filter(|bar| bar.close > 0.0)
                    .map(|bar| (bar.date, bar.close))
                    .collect();
                for bar in &mut bars {
                    if let Some(qfq_close) = factors.get(&bar.date) {
                        if bar.close > 0.0 {
                            bar.adj_factor = Some(qfq_close / bar.close);
                        }
                    }
                }
                Ok((bars, Some(source), None))
            }
            Err(err) => Ok((bars, None, Some(err))),
        }
    }

    pub async fn probe_daily(&self, symbol: &str, start: &str, end: &str) -> Vec<ProbeResult> {
        let mut results = Vec::new();
        for source in [SourceId::Eastmoney, SourceId::Tencent, SourceId::Sina] {
            let started = Instant::now();
            let result = self.daily_from(source, symbol, start, end, true).await;
            let elapsed_ms = started.elapsed().as_millis();
            match result {
                Ok((data, _, error)) => results.push(ProbeResult {
                    source,
                    ok: true,
                    rows: data.len(),
                    elapsed_ms,
                    error,
                    data: Some(data),
                }),
                Err(error) => results.push(ProbeResult {
                    source,
                    ok: false,
                    rows: 0,
                    elapsed_ms,
                    error: Some(error),
                    data: None,
                }),
            }
        }
        results
    }

    pub async fn probe_snapshot(&self, symbol: &str) -> Vec<(SourceId, Result<Snapshot, String>)> {
        let mut results = Vec::new();
        for source in [SourceId::Eastmoney, SourceId::Tencent, SourceId::Sina] {
            let result = match source {
                SourceId::Eastmoney => eastmoney::snapshot(&self.client, symbol).await,
                SourceId::Tencent => tencent::snapshot(&self.client, symbol).await,
                SourceId::Sina => sina::snapshot(&self.client, symbol).await,
            };
            results.push((source, result));
        }
        results
    }
}

impl Default for SourceHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_ohlc() {
        let bar = DailyBar {
            symbol: "600519.SH".into(),
            date: "2026-10-08".into(),
            open: 10.0,
            high: 9.0,
            low: 8.0,
            close: 10.0,
            volume: 1.0,
            amount: 1.0,
            amount_known: true,
            adj_factor: None,
        };
        assert!(bar.validate().is_err());
    }

    #[test]
    fn failed_primary_enters_cooldown_but_secondary_is_available() {
        let hub = SourceHub::new();
        hub.record_failure(SourceId::Tencent, "timeout");
        assert!(!hub.available(SourceId::Tencent));
        assert!(hub.available(SourceId::Eastmoney));
        hub.record_success(SourceId::Tencent);
        assert!(hub.available(SourceId::Tencent));
    }

    #[test]
    fn priority_is_dataset_specific() {
        assert_eq!(SourceHub::priority("quote_daily")[0], SourceId::Tencent);
        assert_eq!(SourceHub::priority("index_quote")[1], SourceId::Eastmoney);
    }
}
