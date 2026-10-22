#![allow(dead_code)]

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstrumentDto {
    pub symbol: String,
    pub name: String,
    #[serde(default)]
    pub pinyin: String,
    pub exchange: String,
    pub board: String,
    pub industry: String,
    pub list_date: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BarDto {
    pub symbol: String,
    pub trade_date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub amount: f64,
    pub adj_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FinancialDto {
    pub symbol: String,
    pub report_period: String,
    pub announce_date: String,
    pub statement_type: String,
    pub version: i64,
    pub revenue: f64,
    pub net_profit: f64,
    pub total_assets: f64,
    pub total_liabilities: f64,
    pub equity: f64,
    pub operating_cash_flow: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexCatalogDto {
    pub code: String,
    pub name: String,
    pub market: String,
    pub category: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexQuoteDto {
    pub index_code: String,
    pub trade_date: String,
    pub close: f64,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexMemberDto {
    pub index_code: String,
    pub symbol: String,
    pub in_date: String,
    pub out_date: Option<String>,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SectorDto {
    pub id: i64,
    pub name: String,
    pub sector_type: String,
    pub source: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SectorMemberDto {
    pub sector_id: i64,
    pub symbol: String,
    pub in_date: String,
    pub out_date: Option<String>,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DailyBasicDto {
    pub symbol: String,
    pub trade_date: String,
    pub total_shares: f64,
    pub float_shares: f64,
    pub total_mv: f64,
    pub float_mv: f64,
    pub pe_ttm: Option<f64>,
    pub pb: Option<f64>,
    pub turnover: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompanyProfileDto {
    pub symbol: String,
    pub full_name: String,
    pub industry: String,
    pub source: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReferenceBundle {
    pub index_catalog: Vec<IndexCatalogDto>,
    pub index_quote: Vec<IndexQuoteDto>,
    pub index_member: Vec<IndexMemberDto>,
    pub sector_catalog: Vec<SectorDto>,
    pub sector_member: Vec<SectorMemberDto>,
    pub daily_basic: Vec<DailyBasicDto>,
    pub company_profile: Vec<CompanyProfileDto>,
}

#[derive(Debug, Clone)]
pub struct AdapterError(pub String);

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for AdapterError {}

pub trait MarketDataAdapter {
    fn name(&self) -> &'static str;
    fn instruments(&self) -> Result<Vec<InstrumentDto>, AdapterError>;
    fn bars(&self, symbol: &str) -> Result<Vec<BarDto>, AdapterError>;
    fn financials(&self, symbol: &str) -> Result<Vec<FinancialDto>, AdapterError>;
    fn reference(&self) -> Result<ReferenceBundle, AdapterError>;
}

#[derive(Debug, Deserialize)]
struct Fixture {
    calendar: Vec<String>,
    instruments: Vec<InstrumentDto>,
    bars: Vec<BarDto>,
    financials: Vec<FinancialDto>,
}

pub struct MockAdapter {
    fixture: Fixture,
}

impl MockAdapter {
    pub fn from_fixture_json(json: &str) -> Result<Self, AdapterError> {
        let fixture = serde_json::from_str(json).map_err(|e| AdapterError(e.to_string()))?;
        Ok(Self { fixture })
    }

    pub fn embedded() -> Result<Self, AdapterError> {
        Self::from_fixture_json(include_str!("../fixtures/adapter-mock.json"))
    }
}

impl MarketDataAdapter for MockAdapter {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn instruments(&self) -> Result<Vec<InstrumentDto>, AdapterError> {
        Ok(self.fixture.instruments.clone())
    }

    fn bars(&self, symbol: &str) -> Result<Vec<BarDto>, AdapterError> {
        Ok(self
            .fixture
            .bars
            .iter()
            .filter(|bar| bar.symbol == symbol)
            .cloned()
            .collect())
    }

    fn financials(&self, symbol: &str) -> Result<Vec<FinancialDto>, AdapterError> {
        Ok(self
            .fixture
            .financials
            .iter()
            .filter(|report| report.symbol == symbol)
            .cloned()
            .collect())
    }

    fn reference(&self) -> Result<ReferenceBundle, AdapterError> {
        serde_json::from_str(reference_fixture_json()).map_err(|e| AdapterError(e.to_string()))
    }
}

impl MockAdapter {
    pub fn calendar(&self) -> Vec<String> {
        self.fixture.calendar.clone()
    }
}

pub fn normalize_symbol(input: &str) -> Option<String> {
    let raw = input.trim().to_uppercase().replace(' ', "");
    if raw.len() == 6 && raw.chars().all(|c| c.is_ascii_digit()) {
        let exchange = match &raw[..1] {
            "6" => "SH",
            "0" | "3" => "SZ",
            "4" | "8" => "BJ",
            _ => return None,
        };
        return Some(format!("{raw}.{exchange}"));
    }
    if let Some((left, right)) = raw.split_once('.') {
        if left.len() == 6
            && left.chars().all(|c| c.is_ascii_digit())
            && matches!(right, "SH" | "SZ" | "BJ")
        {
            return Some(format!("{left}.{right}"));
        }
        if right.len() == 6
            && right.chars().all(|c| c.is_ascii_digit())
            && matches!(left, "SH" | "SZ" | "BJ")
        {
            return Some(format!("{right}.{left}"));
        }
    }
    if raw.len() == 8 && raw[..2].chars().all(|c| c.is_ascii_alphabetic()) {
        let (prefix, digits) = raw.split_at(2);
        if matches!(prefix, "SH" | "SZ" | "BJ")
            && digits.len() == 6
            && digits.chars().all(|c| c.is_ascii_digit())
        {
            return Some(format!("{digits}.{prefix}"));
        }
    }
    None
}

pub fn adjust_bars(bars: &[BarDto], mode: &str) -> Vec<BarDto> {
    let latest = bars.last().map(|bar| bar.adj_factor).unwrap_or(1.0);
    bars.iter()
        .map(|bar| {
            let factor = match mode {
                "qfq" => bar.adj_factor / latest,
                "hfq" => bar.adj_factor,
                _ => 1.0,
            };
            let mut adjusted = bar.clone();
            adjusted.open *= factor;
            adjusted.high *= factor;
            adjusted.low *= factor;
            adjusted.close *= factor;
            adjusted
        })
        .collect()
}

pub fn reference_fixture_json() -> &'static str {
    include_str!("../fixtures/reference-mock.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_adapter_loads_offline_fixture() {
        let adapter = MockAdapter::embedded().unwrap();
        assert_eq!(adapter.name(), "mock");
        assert_eq!(adapter.instruments().unwrap().len(), 2);
        assert_eq!(adapter.bars("600519.SH").unwrap().len(), 3);
        assert_eq!(adapter.financials("600519.SH").unwrap()[0].version, 1);
    }

    #[test]
    fn reference_fixture_is_offline_and_complete() {
        let fixture: serde_json::Value =
            serde_json::from_str(reference_fixture_json()).expect("reference fixture json");
        for key in [
            "index_catalog",
            "index_quote",
            "index_member",
            "sector_catalog",
            "sector_member",
            "daily_basic",
            "company_profile",
        ] {
            assert!(
                fixture[key].as_array().is_some_and(|rows| !rows.is_empty()),
                "missing {key}"
            );
        }
    }

    #[test]
    fn symbol_normalization_covers_exchanges() {
        assert_eq!(normalize_symbol("600519").as_deref(), Some("600519.SH"));
        assert_eq!(normalize_symbol("000858.SZ").as_deref(), Some("000858.SZ"));
        assert_eq!(normalize_symbol("sz000333").as_deref(), Some("000333.SZ"));
        assert_eq!(normalize_symbol("830799").as_deref(), Some("830799.BJ"));
        assert_eq!(normalize_symbol("not-a-symbol"), None);
    }

    #[test]
    fn qfq_and_hfq_adjust_prices_but_keep_suspension() {
        let adapter = MockAdapter::embedded().unwrap();
        let bars = adapter.bars("600519.SH").unwrap();
        let qfq = adjust_bars(&bars, "qfq");
        let hfq = adjust_bars(&bars, "hfq");
        assert!((qfq.last().unwrap().close - 1505.0).abs() < 1e-9);
        assert!(qfq[0].close < bars[0].close);
        assert!(hfq.last().unwrap().close > bars.last().unwrap().close);
        assert_eq!(qfq[1].volume, 0.0);
    }
}
