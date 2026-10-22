use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instrument {
    pub symbol: String,
    pub name: String,
    pub pinyin: String,
    pub exchange: String,
    pub board: String,
    pub industry: String,
    pub list_date: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub symbol: String,
    pub name: String,
    pub exchange: String,
    pub industry: String,
    pub close: Option<f64>,
    pub change_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockDetail {
    pub updated_at: Option<String>,
    pub symbol: String,
    pub name: String,
    pub pinyin: String,
    pub exchange: String,
    pub board: String,
    pub industry: String,
    pub list_date: String,
    pub status: String,
    pub close: f64,
    pub change_pct: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub volume: f64,
    pub amount: f64,
    pub turnover: f64,
    pub prev_close: f64,
    pub high_52w: f64,
    pub low_52w: f64,
    pub total_mv: Option<f64>,
    pub float_mv: Option<f64>,
    pub pe_ttm: Option<f64>,
    pub pb: Option<f64>,
    pub as_of: String,
    pub source: String,
    pub stale: bool,
    pub amount_known: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexCard {
    #[serde(default)]
    pub change_points: Option<f64>,
    pub code: String,
    pub name: String,
    pub close: f64,
    pub change_pct: f64,
    pub amount: f64,
    pub amount_known: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Breadth {
    pub advance: i64,
    pub decline: i64,
    pub flat: i64,
    pub limit_up: i64,
    pub limit_down: i64,
    pub total_amount: f64,
    pub amount_known: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorRow {
    pub id: i64,
    pub name: String,
    pub sector_type: String,
    pub change_pct: f64,
    pub leading_symbol: String,
    pub leading_name: String,
    pub turnover: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOverview {
    #[serde(default)]
    pub sector_source: String,
    #[serde(default)]
    pub sector_updated_at: Option<i64>,
    #[serde(default)]
    pub sector_stale: bool,
    pub as_of: String,
    pub is_trading_day: bool,
    pub indices: Vec<IndexCard>,
    pub breadth: Breadth,
    pub sectors: Vec<SectorRow>,
    pub source: String,
    pub updated_at: String,
    pub is_demo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bar {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub amount: f64,
    pub ma5: Option<f64>,
    pub ma10: Option<f64>,
    pub ma20: Option<f64>,
    pub ma60: Option<f64>,
    pub vol_ma5: Option<f64>,
    pub vol_ma10: Option<f64>,
    pub ema12: Option<f64>,
    pub boll_upper: Option<f64>,
    pub boll_mid: Option<f64>,
    pub boll_lower: Option<f64>,
    pub dif: Option<f64>,
    pub dea: Option<f64>,
    pub macd: Option<f64>,
    pub k: Option<f64>,
    pub d: Option<f64>,
    pub j: Option<f64>,
    pub rsi: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarsResponse {
    pub symbol: String,
    pub name: String,
    pub period: String,
    pub adjust: String,
    pub adjust_available: bool,
    pub notice: Option<String>,
    pub as_of: String,
    pub bars: Vec<Bar>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialReport {
    pub report_period: String,
    pub announce_date: String,
    pub statement_type: String,
    pub version: i64,
    pub effective_at: String,
    pub supersedes: Option<i64>,
    pub is_current: bool,
    pub revenue: f64,
    pub net_profit: f64,
    pub total_assets: f64,
    pub total_liabilities: f64,
    pub equity: f64,
    pub operating_cash_flow: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialIndicator {
    pub report_period: String,
    pub roe: f64,
    pub gross_margin: f64,
    pub net_margin: f64,
    pub debt_ratio: f64,
    pub eps: f64,
    pub bps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Valuation {
    pub pe_ttm: Option<f64>,
    pub pb: Option<f64>,
    pub pe_percentile: Option<f64>,
    pub pb_percentile: Option<f64>,
    pub sample_days: i64,
    pub as_of: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fundamentals {
    pub symbol: String,
    pub name: String,
    pub reports: Vec<FinancialReport>,
    pub indicators: Vec<FinancialIndicator>,
    pub valuation: Valuation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub symbol: String,
    pub name: String,
    pub full_name: String,
    pub business: String,
    pub industry: String,
    pub list_date: String,
    pub controller: String,
    pub website: String,
    pub region: String,
    pub intro: String,
    pub source: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyBasic {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberRow {
    pub symbol: String,
    pub name: String,
    pub weight: f64,
    pub close: f64,
    pub change_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDetail {
    pub change_points: Option<f64>,
    pub code: String,
    pub name: String,
    pub close: f64,
    pub change_pct: f64,
    pub amount: f64,
    pub amount_known: bool,
    pub as_of: String,
    pub members: Vec<MemberRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorDetail {
    pub id: i64,
    pub name: String,
    pub sector_type: String,
    pub change_pct: f64,
    pub members: Vec<MemberRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchlistItem {
    pub source: String,
    pub id: String,
    pub symbol: String,
    pub name: String,
    pub note: String,
    pub sort: i64,
    pub close: f64,
    pub change_pct: f64,
    pub as_of: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchlistGroup {
    pub id: String,
    pub name: String,
    pub sort: i64,
    pub items: Vec<WatchlistItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub theme: String,
    pub up_color: String,
    pub default_home: String,
    pub timezone: String,
    pub density: String,
    pub poll_interval: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataJob {
    pub id: String,
    pub dataset: String,
    pub status: String,
    pub progress: i64,
    pub message: String,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceConfig {
    pub id: String,
    pub provider: String,
    pub enabled: bool,
    pub base_url: String,
    pub secret_env: String,
    pub secret_configured: bool,
    pub secret_masked: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityReport {
    pub dataset: String,
    pub coverage: f64,
    pub missing: i64,
    pub duplicates: i64,
    pub anomalies: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    pub status: String,
    pub app: String,
    pub database: String,
    pub last_sync: Option<String>,
    pub is_demo: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateGroupBody {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct AddItemBody {
    pub group_id: String,
    pub symbol: String,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RenameGroupBody {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ReorderItemsBody {
    pub group_id: String,
    pub ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateItemBody {
    pub note: Option<String>,
    pub sort: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct BackfillBody {
    pub dataset: String,
    pub start: String,
    pub end: String,
    pub operator: String,
}

#[derive(Debug, Deserialize)]
pub struct CorrectProfileBody {
    pub field: String,
    pub value: String,
    pub operator: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsBody {
    pub theme: Option<String>,
    pub up_color: Option<String>,
    pub default_home: Option<String>,
    pub timezone: Option<String>,
    pub density: Option<String>,
    pub poll_interval: Option<i64>,
}
