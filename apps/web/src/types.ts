export type IndexCard = { code: string; name: string; close: number; change_points: number | null; change_pct: number; amount: number; amount_known: boolean };
export type NewsItem = { id: string; title: string; url: string; source: string; published_at: number; summary: string };
export type NewsFeed = { data: NewsItem[]; source: string; updated_at: number; stale: boolean };
export type Breadth = { advance: number; decline: number; flat: number; limit_up: number; limit_down: number; total_amount: number; amount_known: boolean };
export type SectorRow = { id: number; name: string; sector_type: string; change_pct: number; leading_symbol: string; leading_name: string; turnover: number };
export type MarketOverview = {
  sector_source?: string;
  sector_updated_at?: number | null;
  sector_stale?: boolean;
  as_of: string;
  is_trading_day: boolean;
  indices: IndexCard[];
  breadth: Breadth;
  sectors: SectorRow[];
  source: string;
  updated_at: string;
  is_demo: boolean;
};

export type SearchResult = { symbol: string; name: string; exchange: string; industry: string; close: number | null; change_pct: number | null };

export type StockDetail = {
  updated_at?: string | null;
  symbol: string; name: string; pinyin: string; exchange: string; board: string; industry: string; list_date: string; status: string;
  close: number; change_pct: number; open: number; high: number; low: number; volume: number; amount: number; turnover: number;
  prev_close: number; high_52w: number; low_52w: number; total_mv: number | null; float_mv: number | null; pe_ttm: number | null; pb: number | null; as_of: string;
  source: string; stale: boolean; amount_known: boolean;
};

export type Bar = {
  date: string; open: number; high: number; low: number; close: number; volume: number; amount: number;
  ma5: number | null; ma10: number | null; ma20: number | null; ma60: number | null; vol_ma5: number | null; vol_ma10: number | null; ema12: number | null; boll_upper: number | null; boll_mid: number | null; boll_lower: number | null;
  dif: number | null; dea: number | null; macd: number | null; k: number | null; d: number | null; j: number | null; rsi: number | null;
};
export type BarsResponse = { symbol: string; name: string; period: string; adjust: string; adjust_available: boolean; notice: string | null; as_of: string; bars: Bar[] };

export type FinancialReport = { report_period: string; announce_date: string; statement_type: string; version: number; effective_at: string; supersedes: number | null; is_current: boolean; revenue: number | null; net_profit: number | null; total_assets: number | null; total_liabilities: number | null; equity: number | null; operating_cash_flow: number | null };
export type FinancialIndicator = { report_period: string; roe: number | null; gross_margin: number | null; net_margin: number | null; debt_ratio: number | null; eps: number | null; bps: number | null };
export type Valuation = { pe_ttm: number | null; pb: number | null; pe_percentile: number | null; pb_percentile: number | null; sample_days: number; as_of: string };
export type Fundamentals = { symbol: string; name: string; reports: FinancialReport[]; indicators: FinancialIndicator[]; valuation: Valuation };
export type Profile = { symbol: string; name: string; full_name: string; business: string; industry: string; list_date: string; controller: string; website: string; region: string; intro: string; source: string; updated_at: string };
export type DailyBasic = { symbol: string; trade_date: string; total_shares: number; float_shares: number; total_mv: number; float_mv: number; pe_ttm: number | null; pb: number | null; turnover: number };

export type MemberRow = { symbol: string; name: string; weight: number; close: number; change_pct: number };
export type IndexDetail = { code: string; name: string; close: number; change_points: number | null; change_pct: number; amount: number; amount_known: boolean; as_of: string; members: MemberRow[] };
export type SectorDetail = { id: number; name: string; sector_type: string; change_pct: number; members: MemberRow[] };

export type WatchlistItem = { id: string; symbol: string; name: string; note: string; sort: number; close: number; change_pct: number; as_of: string; source?: string };
export type WatchlistGroup = { id: string; name: string; color: string; sort: number; items: WatchlistItem[] };
export type Settings = { theme: string; up_color: string; default_home: string; timezone: string; density: string; poll_interval: number };
export type DataJob = { id: string; dataset: string; status: string; progress: number; message: string; started_at: string; finished_at: string | null };
export type QualityReport = { dataset: string; coverage: number; missing: number; duplicates: number; anomalies: number; updated_at: string };
export type Health = { status: string; app: string; database: string; last_sync: string | null; is_demo: boolean };

export type SourceHealth = { source: "eastmoney" | "tencent" | "sina"; available: boolean; failures: number; cooldown_seconds: number; last_success_at: string | null; last_error: string | null };
export type SourceFreshness = { dataset: string; status: "fresh" | "partial" | "stale"; last_success_at: string | null; last_source: string | null; last_batch_id: string | null; last_error: string | null; updated_at: string };
export type SourcePolicy = { dataset: string; priority: { id: string; label: string }[] };
export type SourceRetry = { dataset: string; symbol: string; start: string; end: string; attempts: number; last_error: string };
export type SourceStatus = { policies: SourcePolicy[]; health: SourceHealth[]; freshness: SourceFreshness[]; retry_queue: SourceRetry[]; mock_fallback: boolean };

export type WatchSignal = { kind: string; title: string; evidence: string; invalidation: string; risk: string };
export type WatchAnalysis = {
  symbol: string; name: string; industry: string; as_of: string; source: string;
  status: "ready" | "stale" | "missing" | "demo"; notice: string;
  close: number | null; change: number | null; change_pct: number | null;
  volume: number | null; amount: number | null; score: number | null; trend: string;
  volatility: number | null; volume_ratio: number | null; support: number | null;
  resistance: number | null; ma20: number | null; ma60: number | null;
  sparkline: number[]; signals: WatchSignal[]; bars_count: number; technical_bars_count: number; history_status: string; history_start: string;
};
export type WatchWorkbench = { generated_at: string; expected_session: string; analyses: WatchAnalysis[] };
export type WatchAlertInput = { symbol: string; kind: string; threshold: number; cooldown_minutes: number; enabled: boolean };
export type WatchAlertRule = WatchAlertInput & { id: string; last_triggered_at: string | null };
export type WatchAlertEvent = { id: number; rule_id: string; symbol: string; message: string; as_of: string; created_at: string };
export type WatchAlerts = { rules: WatchAlertRule[]; events: WatchAlertEvent[] };
