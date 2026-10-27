use std::collections::HashMap;

use axum::extract::{FromRequest, FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::db::{now_iso, AppState};
use crate::error::{envelope, ApiResponse, AppError};
use crate::indicators;
use crate::models::*;

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OverviewQuery {
    pub refresh: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct BarsQuery {
    pub period: Option<String>,
    pub adjust: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FundamentalsQuery {
    pub as_of: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RunJobBody {
    pub dataset: String,
    #[serde(default)]
    pub request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SourceProbeQuery {
    pub symbol: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SourceSyncBody {
    pub dataset: String,
    pub symbols: Vec<String>,
    pub start: String,
    pub end: String,
    #[serde(default)]
    pub request_id: Option<String>,
}

type SnapshotRow = (String, f64, f64, f64, f64, f64, f64, f64, String, String);

pub struct ApiJson<T>(pub T);

impl<T, S> FromRequest<S> for ApiJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(|rejection| AppError::bad_request(rejection.body_text()))?;
        Ok(Self(value))
    }
}

pub struct ApiQuery<T>(pub T);

impl<T, S> FromRequestParts<S> for ApiQuery<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(|rejection| AppError::bad_request(rejection.body_text()))?;
        Ok(Self(value))
    }
}

pub struct ApiPath<T>(pub T);

impl<T, S> FromRequestParts<S> for ApiPath<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(value) = Path::<T>::from_request_parts(parts, state)
            .await
            .map_err(|rejection| AppError::bad_request(rejection.body_text()))?;
        Ok(Self(value))
    }
}

pub fn build_app(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin([
            "http://127.0.0.1:5173".parse().unwrap(),
            "http://localhost:5173".parse().unwrap(),
        ])
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route(
            "/api/v1/trading/paper-strategies/{id}/execution",
            get(crate::execution::list).post(crate::execution::execute),
        )
        .route(
            "/api/v1/trading/paper-strategies/{id}",
            axum::routing::patch(crate::paper_strategy::update),
        )
        .route(
            "/api/v1/trading/paper-strategies/{id}/events",
            get(crate::paper_strategy::events),
        )
        .route(
            "/api/v1/trading/paper-strategies",
            get(crate::paper_strategy::list).post(crate::paper_strategy::create),
        )
        .route(
            "/api/v1/trading/research",
            get(crate::trading_research::list).post(crate::trading_research::create),
        )
        .route(
            "/api/v1/trading/research/{id}/analyze",
            post(crate::trading_research::run),
        )
        .route("/api/v1/quant/strategies", get(crate::quant::list))
        .route("/api/v1/quant/seed", post(crate::quant::seed))
        .route("/api/v1/quant/strategies/{id}", put(crate::quant::toggle))
        .route("/api/v1/quant/run", post(crate::quant::run))
        .route(
            "/api/v1/quant/observations",
            get(crate::quant::observations),
        )
        .route("/api/v1/health", get(health))
        .route("/api/v1/market/overview", get(overview))
        .route("/api/v1/market/breadth", get(crate::market_breadth::get))
        .route("/api/v1/instruments/search", get(search))
        .route("/api/v1/instruments/{symbol}", get(stock_detail))
        .route("/api/v1/instruments/{symbol}/bars", get(bars))
        .route(
            "/api/v1/instruments/{symbol}/fundamentals",
            get(fundamentals),
        )
        .route(
            "/api/v1/instruments/{symbol}/profile",
            get(profile).put(correct_profile),
        )
        .route("/api/v1/instruments/{symbol}/daily-basic", get(daily_basic))
        .route("/api/v1/indices/{code}", get(index_detail))
        .route("/api/v1/indices/{code}/bars", get(index_bars))
        .route("/api/v1/market/breadth/history", get(breadth_history))
        .route("/api/v1/market/popularity", get(popularity))
        .route("/api/v1/market/news", get(market_news))
        .route(
            "/api/v1/instruments/{symbol}/supplement/{dataset}",
            get(stock_supplement),
        )
        .route(
            "/api/v1/instruments/{symbol}/supplement/retry",
            post(retry_supplements),
        )
        .route("/api/v1/indices/{code}/members", get(index_members))
        .route("/api/v1/sectors", get(sectors))
        .route("/api/v1/sectors/{id}", get(sector_detail))
        .route("/api/v1/sectors/{id}/members", get(sector_members))
        .route(
            "/api/v1/watchlists/workbench",
            get(crate::workbench::dashboard),
        )
        .route(
            "/api/v1/watchlists/alerts",
            get(crate::workbench::alerts).post(crate::workbench::create_alert),
        )
        .route(
            "/api/v1/watchlists/alerts/{id}",
            put(crate::workbench::update_alert).delete(crate::workbench::delete_alert),
        )
        .route("/api/v1/watchlists", get(watchlists).post(create_group))
        .route("/api/v1/realtime/quotes", get(realtime_quotes))
        .route("/api/v1/watchlists/refresh", post(refresh_watchlists))
        .route("/api/v1/watchlists/groups", post(create_group))
        .route(
            "/api/v1/watchlists/groups/{id}",
            put(rename_group).delete(delete_group),
        )
        .route(
            "/api/v1/watchlists/groups/{id}/items",
            axum::routing::delete(clear_group),
        )
        .route("/api/v1/watchlists/items", post(add_item))
        .route("/api/v1/watchlists/items/reorder", put(reorder_items))
        .route(
            "/api/v1/watchlists/items/{id}",
            put(update_item).delete(delete_item),
        )
        .route("/api/v1/settings", get(get_settings).put(update_settings))
        .route("/api/v1/admin/data-jobs", get(data_jobs).post(run_job))
        .route("/api/v1/admin/sources", get(source_status))
        .route("/api/v1/admin/sources/probe", get(source_probe))
        .route("/api/v1/admin/sources/sync", post(source_sync))
        .route("/api/v1/admin/backfill", post(run_backfill))
        .route("/api/v1/admin/data-sources", get(data_sources))
        .route("/api/v1/admin/quality", get(quality))
        .route(
            "/api/v1/data/subscriptions",
            post(crate::persistence::lease),
        )
        .route(
            "/api/v1/data/checkpoint",
            post(crate::persistence::checkpoint),
        )
        .route("/api/v1/data/inputs", post(crate::persistence::freeze))
        .route(
            "/api/v1/data/inputs/{hash}",
            get(crate::persistence::frozen),
        )
        .route("/api/v1/data/retention", get(crate::persistence::retention))
        .route(
            "/api/v1/data/cleanup/preview",
            post(crate::persistence::preview),
        )
        .route("/api/v1/data/cleanup", post(crate::persistence::cleanup))
        .route("/api/v1/data/cleanup/{id}", get(crate::persistence::result))
        .route("/api/v1/openapi.json", get(openapi))
        .fallback(fallback)
        .layer(TraceLayer::new_for_http().make_span_with(
            |request: &axum::http::Request<axum::body::Body>| {
                let uri = crate::security::redact(&request.uri().to_string());
                tracing::info_span!("http", method = %request.method(), uri = %uri)
            },
        ))
        .layer(cors)
        .with_state(state)
}

async fn openapi() -> impl IntoResponse {
    (
        [("content-type", "application/json; charset=utf-8")],
        include_str!("../../../docs/api/openapi.json"),
    )
}

pub(crate) async fn fallback() -> impl IntoResponse {
    AppError::new(
        StatusCode::NOT_FOUND,
        "NOT_OPEN",
        "接口未开放或不存在；MVP 不包含该能力",
    )
}

fn db(state: &AppState) -> Result<std::sync::MutexGuard<'_, Connection>, AppError> {
    let start = std::time::Instant::now();
    loop {
        match state.db.try_lock() {
            Ok(conn) => return Ok(conn),
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err(AppError::internal("database lock poisoned"))
            }
            Err(_) if start.elapsed() >= std::time::Duration::from_secs(1) => {
                return Err(AppError::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "DB_BUSY",
                    "数据库繁忙，请稍后重试",
                ))
            }
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(2)),
        }
    }
}

fn latest_date(conn: &Connection, symbol: Option<&str>) -> Result<Option<String>, AppError> {
    let date = match symbol {
        Some(s) => conn.query_row(
            "SELECT MAX(trade_date) FROM quote_daily WHERE symbol = ?1",
            params![s],
            |r| r.get::<_, Option<String>>(0),
        )?,
        None => conn.query_row("SELECT MAX(trade_date) FROM quote_daily", [], |r| {
            r.get::<_, Option<String>>(0)
        })?,
    };
    Ok(date)
}

type BasicRow = (Option<f64>, Option<f64>, Option<f64>, Option<f64>);
type ValuationRow = (String, Option<f64>, Option<f64>, Option<f64>);

struct QuoteRow {
    date: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    amount: f64,
    turnover: f64,
    source: String,
    amount_known: bool,
}

fn two_quotes(conn: &Connection, symbol: &str) -> Result<Vec<QuoteRow>, AppError> {
    let mut stmt = conn.prepare("SELECT trade_date, open, high, low, close, volume, amount, turnover, source, amount_known FROM quote_daily WHERE symbol = ?1 ORDER BY trade_date DESC LIMIT 2")?;
    let rows = stmt.query_map(params![symbol], |r| {
        Ok(QuoteRow {
            date: r.get(0)?,
            open: r.get(1)?,
            high: r.get(2)?,
            low: r.get(3)?,
            close: r.get(4)?,
            volume: r.get(5)?,
            amount: r.get(6)?,
            turnover: r.get(7)?,
            source: r.get(8)?,
            amount_known: r.get::<_, i64>(9)? != 0,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn pct(current: f64, prev: f64) -> f64 {
    if prev == 0.0 {
        0.0
    } else {
        (current / prev - 1.0) * 100.0
    }
}

fn get_instrument(conn: &Connection, symbol: &str) -> Result<Instrument, AppError> {
    crate::repository::instrument(conn, symbol)
}

async fn health(State(state): State<AppState>) -> Result<Json<ApiResponse<Health>>, AppError> {
    let conn = db(&state)?;
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM instruments", [], |r| r.get(0))?;
    let last_sync = latest_date(&conn, None)?;
    Ok(envelope(Health {
        status: "ok".into(),
        app: format!("alphak-api · {count} instruments"),
        database: "sqlite · connected".into(),
        last_sync,
        is_demo: true,
    }))
}

async fn overview(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<OverviewQuery>,
) -> Result<Json<ApiResponse<MarketOverview>>, AppError> {
    let force = query.refresh.unwrap_or(false);
    #[cfg(not(test))]
    refresh_market_sources(state.clone(), force);
    let cached = {
        let conn = db(&state)?;
        if force {
            None
        } else {
            crate::market::cached(&conn)?
        }
    };
    if let Some((snapshot, computed_at)) = cached {
        let age_secs = chrono::DateTime::parse_from_rfc3339(&computed_at)
            .map(|time| (chrono::Utc::now() - time.with_timezone(&chrono::Utc)).num_seconds())
            .unwrap_or(i64::MAX);
        if age_secs <= 60 {
            return Ok(envelope(snapshot));
        }
        // Stale-while-revalidate: return the existing snapshot immediately, refresh in background.
        let refresh_state = state.clone();
        tokio::task::spawn_blocking(move || {
            if let Ok(conn) = refresh_state.db.lock() {
                let _ = crate::market::refresh(&conn);
            }
        });
        return Ok(envelope(snapshot));
    }
    let conn = db(&state)?;
    Ok(envelope(crate::market::refresh(&conn)?))
}

async fn popularity(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    crate::live::cached_fetch(&state, "000001.SH", "popularity", 60)
        .await
        .map(envelope)
        .map_err(|error| AppError::new(StatusCode::BAD_GATEWAY, "POPULARITY_UNAVAILABLE", error))
}

async fn market_news(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    crate::live::cached_fetch(&state, "000001.SH", "news", 120)
        .await
        .map(envelope)
        .map_err(|error| AppError::new(StatusCode::BAD_GATEWAY, "NEWS_UNAVAILABLE", error))
}

#[cfg(not(test))]
pub(crate) fn spawn_market_updates(state: AppState) {
    tokio::spawn(async move {
        loop {
            refresh_market_sources(state.clone(), false);
            tokio::time::sleep(std::time::Duration::from_secs(900)).await;
        }
    });
}

#[cfg(test)]
pub(crate) fn spawn_market_updates(_state: AppState) {}

#[cfg(not(test))]
fn refresh_market_sources(state: AppState, force: bool) {
    use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
    static RUNNING: AtomicBool = AtomicBool::new(false);
    static LAST_STARTED: AtomicI64 = AtomicI64::new(0);
    let now = chrono::Utc::now().timestamp();
    let interval = if force { 30 } else { 900 };
    if now - LAST_STARTED.load(Ordering::Relaxed) < interval
        || RUNNING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
    {
        return;
    }
    LAST_STARTED.store(now, Ordering::Relaxed);
    tokio::spawn(async move {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                RUNNING.store(false, Ordering::Release);
            }
        }
        let _reset = Reset;
        let end = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
        let start = end - chrono::Duration::days(14);
        let symbols = {
            let Ok(conn) = state.db.lock() else { return };
            let Ok(mut stmt) =
                conn.prepare("SELECT symbol FROM (SELECT DISTINCT symbol FROM quote_daily WHERE source <> 'demo' UNION SELECT symbol FROM watchlist_item) ORDER BY symbol")
            else {
                return;
            };
            let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) else {
                return;
            };
            rows.filter_map(Result::ok).collect::<Vec<_>>()
        };
        for (dataset, symbols) in [
            (
                "index_quote",
                vec![
                    "000001.SH".into(),
                    "000300.SH".into(),
                    "000905.SH".into(),
                    "399001.SZ".into(),
                    "399006.SZ".into(),
                ],
            ),
            ("quote_daily", symbols),
        ] {
            if symbols.is_empty() {
                continue;
            }
            for chunk in symbols.chunks(20) {
                let eligible = {
                    let Ok(conn) = state.db.lock() else { continue };
                    chunk.iter().filter(|symbol| {
                    force || !conn.query_row(
                        "SELECT EXISTS(SELECT 1 FROM source_retry_task WHERE dataset = ?1 AND symbol = ?2 AND start_date = ?3 AND end_date = ?4)",
                        params![dataset, symbol, start.to_string(), end.to_string()],
                        |row| row.get::<_, bool>(0),
                    ).unwrap_or(true)
                }).cloned().collect::<Vec<_>>()
                };
                if eligible.is_empty() {
                    continue;
                }
                let result = source_sync(
                    State(state.clone()),
                    ApiJson(SourceSyncBody {
                        dataset: dataset.into(),
                        symbols: eligible,
                        start: start.to_string(),
                        end: end.to_string(),
                        request_id: None,
                    }),
                )
                .await;
                if let Err(error) = result {
                    tracing::warn!(dataset, error = %error.message, "market source sync failed");
                }
                if let Ok(conn) = state.db.lock() {
                    let _ = crate::market::refresh(&conn);
                }
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }
    });
}

async fn search(
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<SearchQuery>,
) -> Result<Json<ApiResponse<Vec<SearchResult>>>, AppError> {
    let needle = q.q.unwrap_or_default().trim().to_lowercase();
    if needle.is_empty() {
        return Ok(envelope(vec![]));
    }
    if needle.chars().count() > 80 {
        return Err(AppError::bad_request("搜索关键词过长"));
    }
    let has_local = {
        let conn = db(&state)?;
        conn.query_row("SELECT EXISTS(SELECT 1 FROM instruments WHERE lower(symbol) LIKE ?1 OR lower(name) LIKE ?1 OR lower(pinyin) LIKE ?1)", params![format!("%{}%", needle)], |r| r.get::<_, bool>(0))?
    };
    if !has_local {
        let matches = crate::sources::tencent::search(&state.sources.client, &needle)
            .await
            .map_err(|e| {
                AppError::new(
                    StatusCode::BAD_GATEWAY,
                    "SEARCH_UNAVAILABLE",
                    format!("证券搜索服务暂不可用，请稍后重试：{e}"),
                )
            })?;
        let conn = db(&state)?;
        for (symbol, name, pinyin, exchange) in matches {
            conn.execute("INSERT OR IGNORE INTO instruments (symbol,name,pinyin,exchange,board,industry,list_date,status) VALUES (?1,?2,?3,?4,'未知','未知','','active')", params![symbol,name,pinyin,exchange])?;
        }
    }
    let conn = db(&state)?;
    let pattern = format!("%{}%", needle);
    let mut stmt = conn.prepare("SELECT symbol, name, exchange, industry FROM instruments WHERE lower(symbol) LIKE ?1 OR lower(name) LIKE ?1 OR lower(pinyin) LIKE ?1 ORDER BY CASE WHEN symbol = ?2 THEN 0 ELSE 1 END, symbol LIMIT 20")?;
    let rows = stmt.query_map(params![pattern, needle], |r| {
        Ok(SearchResult {
            symbol: r.get(0)?,
            name: r.get(1)?,
            exchange: r.get(2)?,
            industry: r.get(3)?,
            close: None,
            change_pct: None,
        })
    })?;
    let mut results = Vec::new();
    for row in rows {
        let mut result = row?;
        let quotes = two_quotes(&conn, &result.symbol)?;
        if let Some(latest) = quotes.first() {
            result.close = Some(latest.close);
            result.change_pct = Some(if let Some(prev) = quotes.get(1) {
                pct(latest.close, prev.close)
            } else {
                0.0
            });
        }
        results.push(result);
    }
    Ok(envelope(results))
}

async fn stock_detail(
    State(state): State<AppState>,
    ApiPath(symbol): ApiPath<String>,
) -> Result<Json<ApiResponse<StockDetail>>, AppError> {
    let needs_quotes = {
        let conn = db(&state)?;
        get_instrument(&conn, &symbol)?;
        two_quotes(&conn, &symbol)?.is_empty()
    };
    if needs_quotes {
        let end = chrono::Local::now().date_naive();
        let start = end - chrono::Duration::days(365);
        sync_daily_symbol(
            &state,
            "quote_daily",
            &symbol,
            &start.to_string(),
            &end.to_string(),
            &format!("lookup-{}", uuid::Uuid::new_v4()),
            true,
        )
        .await
        .map_err(|e| {
            AppError::new(
                StatusCode::BAD_GATEWAY,
                "QUOTE_UNAVAILABLE",
                format!("首次加载行情失败，请重试：{e}"),
            )
        })?;
    }
    let conn = db(&state)?;
    let instrument = get_instrument(&conn, &symbol)?;
    let quotes = two_quotes(&conn, &symbol)?;
    let latest = quotes
        .first()
        .ok_or_else(|| AppError::not_found("暂无行情"))?;
    let prev_close = quotes.get(1).map(|q| q.close).unwrap_or(latest.close);
    let (high_52w, low_52w): (Option<f64>, Option<f64>) = conn.query_row(
        "SELECT MAX(high), MIN(low) FROM quote_daily
         WHERE symbol = ?1 AND trade_date >= date(?2, '-1 year') AND trade_date <= ?2",
        params![symbol, latest.date],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let high_52w = high_52w.unwrap_or(latest.high);
    let low_52w = low_52w.unwrap_or(latest.low);
    let basic: Option<BasicRow> = conn.query_row("SELECT total_mv, float_mv, pe_ttm, pb FROM daily_basic WHERE symbol = ?1 ORDER BY trade_date DESC LIMIT 1", params![symbol], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional()?;
    let (total_mv, float_mv, pe_ttm, pb) = basic.unwrap_or((None, None, None, None));
    let stale: bool = conn
        .query_row(
            "SELECT status FROM data_freshness WHERE dataset = 'quote_daily'",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .map(|status| status != "fresh")
        .unwrap_or(false);
    let mut detail = StockDetail {
        updated_at: None,
        symbol: instrument.symbol.clone(),
        name: instrument.name.clone(),
        pinyin: instrument.pinyin,
        exchange: instrument.exchange,
        board: instrument.board,
        industry: instrument.industry,
        list_date: instrument.list_date,
        status: instrument.status,
        close: latest.close,
        change_pct: pct(latest.close, prev_close),
        open: latest.open,
        high: latest.high,
        low: latest.low,
        volume: latest.volume,
        amount: latest.amount,
        turnover: latest.turnover,
        prev_close,
        high_52w,
        low_52w,
        total_mv,
        float_mv,
        pe_ttm,
        pb,
        as_of: latest.date.clone(),
        source: latest.source.clone(),
        stale,
        amount_known: latest.amount_known,
    };
    let snapshot: Option<SnapshotRow> = conn
        .query_row(
            "SELECT trade_date,open,high,low,close,prev_close,volume,amount,source,fetched_at FROM quote_snapshot WHERE symbol=?1 AND trade_date>=?2",
            params![symbol, latest.date],
            |row| {
                Ok((
                    row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?,
                    row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?,
                ))
            },
        )
        .optional()?;
    if let Some((date, open, high, low, close, previous, volume, amount, source, fetched)) =
        snapshot
    {
        detail.as_of = date;
        detail.updated_at = Some(fetched.clone());
        detail.open = open;
        detail.high = high;
        detail.low = low;
        detail.close = close;
        detail.prev_close = previous;
        detail.change_pct = pct(close, previous);
        detail.volume = volume;
        detail.amount = amount;
        detail.amount_known = true;
        detail.source = source;
        detail.stale = chrono::DateTime::parse_from_rfc3339(&fetched)
            .map(|time| chrono::Utc::now().timestamp() - time.timestamp() > 180)
            .unwrap_or(true);
    }
    Ok(envelope(detail))
}

fn aggregate_bars(rows: Vec<(String, f64, f64, f64, f64, f64, f64)>, period: &str) -> Vec<Bar> {
    let mut out: Vec<Bar> = Vec::new();
    for (date, open, high, low, close, volume, amount) in rows {
        let same_bucket = if let Some(last) = out.last() {
            match period {
                "weekly" => NaiveDate::parse_from_str(&last.date, "%Y-%m-%d")
                    .ok()
                    .map(|a| {
                        a.iso_week().year()
                            == NaiveDate::parse_from_str(&date, "%Y-%m-%d")
                                .map(|d| d.iso_week().year())
                                .unwrap_or(0)
                            && a.iso_week().week()
                                == NaiveDate::parse_from_str(&date, "%Y-%m-%d")
                                    .map(|d| d.iso_week().week())
                                    .unwrap_or(0)
                    })
                    .unwrap_or(false),
                "monthly" => last.date.get(0..7) == date.get(0..7),
                _ => false,
            }
        } else {
            false
        };
        if same_bucket {
            let last = out.last_mut().unwrap();
            last.high = last.high.max(high);
            last.low = last.low.min(low);
            last.close = close;
            last.volume += volume;
            last.amount += amount;
            last.date = date;
        } else {
            out.push(Bar {
                date,
                open,
                high,
                low,
                close,
                volume,
                amount,
                ma5: None,
                ma10: None,
                ma20: None,
                ma60: None,
                vol_ma5: None,
                vol_ma10: None,
                ema12: None,
                boll_upper: None,
                boll_mid: None,
                boll_lower: None,
                dif: None,
                dea: None,
                macd: None,
                k: None,
                d: None,
                j: None,
                rsi: None,
            });
        }
    }
    out
}

async fn bars(
    State(state): State<AppState>,
    ApiPath(symbol): ApiPath<String>,
    ApiQuery(q): ApiQuery<BarsQuery>,
) -> Result<Json<ApiResponse<BarsResponse>>, AppError> {
    let period = q.period.unwrap_or_else(|| "daily".into());
    if matches!(period.as_str(), "1m" | "5m" | "15m" | "30m" | "60m") {
        let name = {
            let conn = db(&state)?;
            get_instrument(&conn, &symbol)?.name
        };
        return crate::live::minutes(&state, &symbol, &name, &period)
            .await
            .map(envelope)
            .map_err(|error| AppError::new(StatusCode::BAD_GATEWAY, "MINUTE_UNAVAILABLE", error));
    }
    if matches!(period.as_str(), "1s" | "3s" | "5s" | "15s" | "30s" | "60s") {
        let name = {
            let conn = db(&state)?;
            get_instrument(&conn, &symbol)?.name
        };
        return crate::live::seconds(&state, &symbol, &name, &period)
            .await
            .map(envelope)
            .map_err(|error| AppError::new(StatusCode::BAD_GATEWAY, "SECOND_UNAVAILABLE", error));
    }
    let conn = db(&state)?;
    let instrument = get_instrument(&conn, &symbol)?;
    let adjust = q.adjust.unwrap_or_else(|| "qfq".into());
    if !matches!(period.as_str(), "daily" | "weekly" | "monthly") {
        return Err(AppError::bad_request(
            "period must be daily, weekly, or monthly",
        ));
    }
    if !matches!(adjust.as_str(), "none" | "qfq" | "hfq") {
        return Err(AppError::bad_request("adjust must be none, qfq, or hfq"));
    }
    let mut stmt = conn.prepare("SELECT trade_date, open, high, low, close, volume, amount, adj_factor, source, adj_source, adj_batch_id FROM quote_daily WHERE symbol = ?1 ORDER BY trade_date")?;
    let rows = stmt
        .query_map(params![symbol], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, f64>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, f64>(3)?,
                r.get::<_, f64>(4)?,
                r.get::<_, f64>(5)?,
                r.get::<_, f64>(6)?,
                r.get::<_, f64>(7)?,
                r.get::<_, String>(8)?,
                r.get::<_, String>(9)?,
                r.get::<_, String>(10)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let factor_available = adjust == "none"
        || rows
            .iter()
            .all(|row| row.8 == "demo" || (!row.9.is_empty() && !row.10.is_empty()));
    let applied_adjust = if factor_available {
        adjust.clone()
    } else {
        "none".to_string()
    };
    let notice = if factor_available {
        None
    } else {
        Some("复权因子尚未补齐，已降级为不复权；补齐后可重新生成复权图。".to_string())
    };
    let latest_factor = rows
        .last()
        .map(|row| row.7)
        .filter(|factor| *factor > 0.0)
        .unwrap_or(1.0);
    let adjusted = rows
        .into_iter()
        .map(
            |(
                date,
                open,
                high,
                low,
                close,
                volume,
                amount,
                factor,
                _source,
                _adj_source,
                _adj_batch,
            )| {
                let factor = match applied_adjust.as_str() {
                    "qfq" => factor / latest_factor,
                    "hfq" => factor,
                    _ => 1.0,
                };
                (
                    date,
                    open * factor,
                    high * factor,
                    low * factor,
                    close * factor,
                    volume,
                    amount,
                )
            },
        )
        .collect::<Vec<_>>();
    let mut bars = aggregate_bars(adjusted, &period);
    indicators::apply(&mut bars);
    let as_of = bars.last().map(|b| b.date.clone()).unwrap_or_default();
    Ok(envelope(BarsResponse {
        symbol,
        name: instrument.name,
        period,
        adjust: applied_adjust,
        adjust_available: factor_available,
        notice,
        as_of,
        bars,
    }))
}

async fn retry_supplements(
    State(state): State<AppState>,
    ApiPath(symbol): ApiPath<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let conn = db(&state)?;
    get_instrument(&conn, &symbol)?;
    conn.execute(
        "UPDATE live_cache SET failures=0,attempted_at=0,fetched_at=0 WHERE symbol=?1",
        params![symbol],
    )?;
    Ok(envelope(serde_json::json!({"reset":true})))
}

async fn stock_supplement(
    State(state): State<AppState>,
    ApiPath((symbol, dataset)): ApiPath<(String, String)>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    if !matches!(dataset.as_str(), "metrics" | "profile" | "financials") {
        return Err(AppError::bad_request("未知补充数据集"));
    }
    {
        let conn = db(&state)?;
        get_instrument(&conn, &symbol)?;
    }
    crate::live::cached_fetch(
        &state,
        &symbol,
        &dataset,
        if dataset == "metrics" { 60 } else { 86400 },
    )
    .await
    .map(envelope)
    .map_err(|error| AppError::new(StatusCode::BAD_GATEWAY, "SUPPLEMENT_UNAVAILABLE", error))
}

async fn fundamentals(
    State(state): State<AppState>,
    ApiPath(symbol): ApiPath<String>,
    ApiQuery(query): ApiQuery<FundamentalsQuery>,
) -> Result<Json<ApiResponse<Fundamentals>>, AppError> {
    let conn = db(&state)?;
    let instrument = get_instrument(&conn, &symbol)?;
    let reports_sql = if query.as_of.is_some() {
        "SELECT f.report_period, f.announce_date, f.statement_type, f.version, f.effective_at, f.supersedes, f.is_current, f.revenue, f.net_profit, f.total_assets, f.total_liabilities, f.equity, f.operating_cash_flow
         FROM financial_report f
         WHERE f.symbol = ?1 AND f.announce_date <= ?2
           AND f.version = (
             SELECT MAX(f2.version) FROM financial_report f2
             WHERE f2.symbol = f.symbol AND f2.report_period = f.report_period
               AND f2.statement_type = f.statement_type AND f2.announce_date <= ?2
           )
         ORDER BY f.report_period DESC"
    } else {
        "SELECT report_period, announce_date, statement_type, version, effective_at, supersedes, is_current, revenue, net_profit, total_assets, total_liabilities, equity, operating_cash_flow
         FROM financial_report WHERE symbol = ?1 AND is_current = 1 ORDER BY report_period DESC"
    };
    let mut stmt = conn.prepare(reports_sql)?;
    let map_report = |r: &rusqlite::Row<'_>| -> rusqlite::Result<FinancialReport> {
        Ok(FinancialReport {
            report_period: r.get(0)?,
            announce_date: r.get(1)?,
            statement_type: r.get(2)?,
            version: r.get(3)?,
            effective_at: r.get(4)?,
            supersedes: r.get(5)?,
            is_current: r.get::<_, i64>(6)? == 1,
            revenue: r.get(7)?,
            net_profit: r.get(8)?,
            total_assets: r.get(9)?,
            total_liabilities: r.get(10)?,
            equity: r.get(11)?,
            operating_cash_flow: r.get(12)?,
        })
    };
    let reports = if let Some(as_of) = &query.as_of {
        stmt.query_map(params![symbol, as_of], map_report)?
            .collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        stmt.query_map(params![symbol], map_report)?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut stored_indicators = HashMap::new();
    let mut stmt = conn.prepare("SELECT report_period, roe, gross_margin, net_margin, debt_ratio, eps, bps FROM financial_indicator WHERE symbol = ?1")?;
    let rows = stmt.query_map(params![symbol], |r| {
        Ok(FinancialIndicator {
            report_period: r.get(0)?,
            roe: r.get(1)?,
            gross_margin: r.get(2)?,
            net_margin: r.get(3)?,
            debt_ratio: r.get(4)?,
            eps: r.get(5)?,
            bps: r.get(6)?,
        })
    })?;
    for row in rows {
        let indicator = row?;
        stored_indicators.insert(indicator.report_period.clone(), indicator);
    }

    let valuation_sql = if query.as_of.is_some() {
        "SELECT trade_date, pe_ttm, pb, total_shares FROM daily_basic WHERE symbol = ?1 AND trade_date <= ?2 ORDER BY trade_date DESC LIMIT 1"
    } else {
        "SELECT trade_date, pe_ttm, pb, total_shares FROM daily_basic WHERE symbol = ?1 ORDER BY trade_date DESC LIMIT 1"
    };
    let latest: Option<ValuationRow> = if let Some(as_of) = &query.as_of {
        conn.query_row(valuation_sql, params![symbol, as_of], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .optional()?
    } else {
        conn.query_row(valuation_sql, params![symbol], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .optional()?
    };
    let (valuation_as_of, pe, pb, shares) = latest.unwrap_or((String::new(), None, None, None));

    let indicators = reports
        .iter()
        .map(|report| {
            let base = stored_indicators.get(&report.report_period);
            let net_margin = if report.revenue != 0.0 {
                report.net_profit / report.revenue * 100.0
            } else {
                base.map(|value| value.net_margin).unwrap_or(0.0)
            };
            let debt_ratio = if report.total_assets != 0.0 {
                report.total_liabilities / report.total_assets * 100.0
            } else {
                base.map(|value| value.debt_ratio).unwrap_or(0.0)
            };
            let roe = if report.equity != 0.0 {
                report.net_profit / report.equity * 100.0
            } else {
                base.map(|value| value.roe).unwrap_or(0.0)
            };
            let (eps, bps) = if let Some(shares) = shares.filter(|value| *value > 0.0) {
                (report.net_profit / shares, report.equity / shares)
            } else {
                (
                    base.map(|value| value.eps).unwrap_or(0.0),
                    base.map(|value| value.bps).unwrap_or(0.0),
                )
            };
            FinancialIndicator {
                report_period: report.report_period.clone(),
                roe,
                gross_margin: base.map(|value| value.gross_margin).unwrap_or(0.0),
                net_margin,
                debt_ratio,
                eps,
                bps,
            }
        })
        .collect::<Vec<_>>();

    let mut pes: Vec<f64> = Vec::new();
    let mut pbs: Vec<f64> = Vec::new();
    let samples_sql = if query.as_of.is_some() {
        "SELECT pe_ttm, pb FROM daily_basic WHERE symbol = ?1 AND trade_date <= ?2"
    } else {
        "SELECT pe_ttm, pb FROM daily_basic WHERE symbol = ?1"
    };
    let mut stmt = conn.prepare(samples_sql)?;
    let rows = if let Some(as_of) = &query.as_of {
        stmt.query_map(params![symbol, as_of], |r| {
            Ok((r.get::<_, Option<f64>>(0)?, r.get::<_, Option<f64>>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        stmt.query_map(params![symbol], |r| {
            Ok((r.get::<_, Option<f64>>(0)?, r.get::<_, Option<f64>>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (a, b) in rows {
        if let Some(v) = a {
            pes.push(v);
        }
        if let Some(v) = b {
            pbs.push(v);
        }
    }
    let percentile = |values: &mut Vec<f64>, current: Option<f64>| -> Option<f64> {
        if values.is_empty() {
            return None;
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let c = current?;
        let rank = values.iter().filter(|v| **v <= c).count() as f64 / values.len() as f64;
        Some(rank * 100.0)
    };
    Ok(envelope(Fundamentals {
        symbol,
        name: instrument.name,
        reports,
        indicators,
        valuation: Valuation {
            pe_ttm: pe,
            pb,
            pe_percentile: percentile(&mut pes, pe),
            pb_percentile: percentile(&mut pbs, pb),
            sample_days: pes.len() as i64,
            as_of: valuation_as_of,
        },
    }))
}

async fn profile(
    State(state): State<AppState>,
    ApiPath(symbol): ApiPath<String>,
) -> Result<Json<ApiResponse<Profile>>, AppError> {
    let conn = db(&state)?;
    let instrument = get_instrument(&conn, &symbol)?;
    let row = conn.query_row("SELECT full_name, business, industry, list_date, controller, website, region, intro, source, updated_at FROM company_profile WHERE symbol = ?1", params![symbol], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, String>(5)?, r.get::<_, String>(6)?, r.get::<_, String>(7)?, r.get::<_, String>(8)?, r.get::<_, String>(9)?))).optional()?;
    let (
        full_name,
        business,
        industry,
        list_date,
        controller,
        website,
        region,
        intro,
        source,
        updated_at,
    ) = row.ok_or_else(|| AppError::not_found("暂无公司资料"))?;
    Ok(envelope(Profile {
        symbol,
        name: instrument.name,
        full_name,
        business,
        industry,
        list_date,
        controller,
        website,
        region,
        intro,
        source,
        updated_at,
    }))
}

async fn correct_profile(
    State(state): State<AppState>,
    ApiPath(symbol): ApiPath<String>,
    ApiJson(body): ApiJson<CorrectProfileBody>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let conn = db(&state)?;
    if body.operator.trim().is_empty() {
        return Err(AppError::bad_request("operator is required"));
    }
    crate::repository::correct_company_profile(
        &conn,
        &symbol,
        &body.field,
        &body.value,
        body.operator.trim(),
    )?;
    Ok(envelope(serde_json::json!({"updated": true})))
}

async fn daily_basic(
    State(state): State<AppState>,
    ApiPath(symbol): ApiPath<String>,
) -> Result<Json<ApiResponse<DailyBasic>>, AppError> {
    let conn = db(&state)?;
    let _ = get_instrument(&conn, &symbol)?;
    conn.query_row("SELECT trade_date, total_shares, float_shares, total_mv, float_mv, pe_ttm, pb, turnover FROM daily_basic WHERE symbol = ?1 ORDER BY trade_date DESC LIMIT 1", params![symbol], |r| Ok(DailyBasic { symbol: symbol.clone(), trade_date: r.get(0)?, total_shares: r.get(1)?, float_shares: r.get(2)?, total_mv: r.get(3)?, float_mv: r.get(4)?, pe_ttm: r.get(5)?, pb: r.get(6)?, turnover: r.get(7)? })).map(envelope).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::not_found("暂无每日基础指标"),
        other => AppError::from(other),
    })
}

async fn index_bars(
    State(state): State<AppState>,
    ApiPath(code): ApiPath<String>,
    Query(query): Query<BarsQuery>,
) -> Result<Json<ApiResponse<BarsResponse>>, AppError> {
    let period = query.period.unwrap_or_else(|| "daily".into());
    if !matches!(period.as_str(), "daily" | "monthly") {
        return Err(AppError::bad_request("指数目前支持日K、月K"));
    }
    let symbol = code.clone();
    let (name, rows) = crate::persistence::run_db(state, move |conn| {
        let name: String = conn.query_row("SELECT name FROM index_catalog WHERE code=?1", [&code], |r| r.get(0))
            .optional()?.ok_or_else(|| AppError::not_found("指数不存在"))?;
        let mut stmt = conn.prepare("SELECT trade_date,open,high,low,close,volume,amount FROM (SELECT * FROM index_quote WHERE index_code=?1 AND source<>'demo' AND strftime('%w',trade_date) NOT IN ('0','6') AND NOT EXISTS(SELECT 1 FROM trading_calendar c WHERE c.trade_date=index_quote.trade_date AND c.is_open=0) ORDER BY trade_date DESC LIMIT 1500) ORDER BY trade_date")?;
        let rows = stmt.query_map([&code], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok((name, rows))
    }).await?;
    let mut bars = aggregate_bars(rows, &period);
    indicators::apply(&mut bars);
    Ok(envelope(BarsResponse {
        symbol,
        name,
        period,
        adjust: "none".into(),
        adjust_available: false,
        notice: None,
        as_of: bars.last().map(|b| b.date.clone()).unwrap_or_default(),
        bars,
    }))
}

#[derive(Debug, Default, Deserialize)]
struct BreadthQuery {
    period: Option<String>,
}

async fn breadth_history(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<BreadthQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let period = query.period.unwrap_or_else(|| "daily".into());
    if !["30min", "1h", "daily", "weekly", "monthly"].contains(&period.as_str()) {
        return Err(AppError::bad_request("不支持的涨跌分布周期"));
    }
    #[cfg(not(test))]
    crate::market_breadth::refresh_history(state.clone(), &period);
    crate::persistence::run_db(state, move |conn| {
        // Same-date previous close only: do not compare a suspended stock to an arbitrary old session.
        let mut stmt = conn.prepare("WITH dates AS (SELECT DISTINCT trade_date FROM quote_daily WHERE source<>'demo' AND strftime('%w',trade_date) NOT IN ('0','6') AND NOT EXISTS(SELECT 1 FROM trading_calendar c WHERE c.trade_date=quote_daily.trade_date AND c.is_open=0) ORDER BY trade_date DESC LIMIT 121), sessions AS (SELECT trade_date,LAG(trade_date) OVER(ORDER BY trade_date) AS previous FROM dates) SELECT q.trade_date,SUM(q.close>p.close),SUM(q.close<p.close),SUM(q.close=p.close) FROM sessions s JOIN quote_daily q ON q.trade_date=s.trade_date JOIN instruments i ON i.symbol=q.symbol JOIN quote_daily p ON p.symbol=q.symbol AND p.trade_date=s.previous WHERE q.source<>'demo' AND p.source<>'demo' AND p.close>0 AND q.volume>0 AND NOT EXISTS(SELECT 1 FROM trading_calendar c WHERE c.is_open=1 AND c.trade_date>s.previous AND c.trade_date<s.trade_date) GROUP BY q.trade_date ORDER BY q.trade_date")?;
        let sample = stmt.query_map([], |r| Ok(json!({"date":r.get::<_,String>(0)?,"advance":r.get::<_,i64>(1)?,"decline":r.get::<_,i64>(2)?,"flat":r.get::<_,i64>(3)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
        // Intraday observations use provider time, never collection time or synthetic daily interpolation.
        let intraday = period == "30min" || period == "1h";
        let step = if period == "30min" { 1800 } else { 3600 };
        let sample = if intraday {
            let mut stmt = conn.prepare("WITH ranked AS (SELECT b.symbol,(b.bucket_start/?1)*?1 AS bucket,CAST(json_extract(b.payload,'$.close') AS REAL) AS close,ROW_NUMBER() OVER(PARTITION BY b.symbol,b.bucket_start/?1 ORDER BY b.bucket_start DESC,b.observed_at DESC) AS rank, date(b.bucket_start,'unixepoch','+8 hours') AS day FROM bar_cache b JOIN instruments i ON i.symbol=b.symbol WHERE (b.period LIKE 'minute_%' OR b.period LIKE 'second_%') AND b.source<>'demo' AND b.bucket_start>strftime('%s','now','-30 days')), prices AS (SELECT r.*, (SELECT q.close FROM quote_daily q WHERE q.symbol=r.symbol AND q.trade_date<r.day AND q.source<>'demo' ORDER BY q.trade_date DESC LIMIT 1) AS previous FROM ranked r WHERE rank=1) SELECT bucket,SUM(close>previous),SUM(close<previous),SUM(close=previous),COUNT(*) FROM prices WHERE previous>0 GROUP BY bucket ORDER BY bucket DESC LIMIT 480")?;
            let mut points = stmt.query_map([step], |r| Ok(json!({"date":r.get::<_,i64>(0)?.to_string(),"advance":r.get::<_,i64>(1)?,"decline":r.get::<_,i64>(2)?,"flat":r.get::<_,i64>(3)?,"coverage":r.get::<_,i64>(4)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
            points.reverse(); points
        } else { sample };
        let mut stmt = conn.prepare("SELECT trade_date,payload,phase,provider_time FROM market_breadth_observation ORDER BY trade_date,provider_time,id")?;
        let rows = stmt.query_map([],|r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let mut market_by_time = std::collections::BTreeMap::new();
        for (date,payload,phase,provider_time) in rows {
            let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&payload) else { continue };
            let tdx = value["source"].as_str().unwrap_or("").starts_with("AxData · 通达信");
            if tdx && value["source"].as_str()!=Some(format!("AxData · 通达信 · {}",if intraday {if step==1800 {"30m"}else{"60m"}}else{"day"}).as_str()) {continue}
            let key = if intraday {
                let Some(time) = provider_time.and_then(|v| chrono::NaiveDateTime::parse_from_str(&v,"%Y-%m-%d %H:%M:%S").ok()) else { continue };
                let timestamp = time.and_utc().timestamp()-8*3600;
                ((timestamp/step)*step).to_string()
            } else { date };
            value["date"]=json!(key); value["phase"]=json!(phase);
            if !tdx && market_by_time.get(&key).is_some_and(|p:&serde_json::Value|p["source"].as_str().unwrap_or("").starts_with("AxData · 通达信")) {continue}
            market_by_time.insert(key,value);
        }
        let market: Vec<_> = market_by_time.into_values().collect();
        let aggregate = |points: Vec<serde_json::Value>| {
            let mut buckets = std::collections::BTreeMap::new();
            for mut point in points {
                let Some(date) = point["date"].as_str().and_then(|v| NaiveDate::parse_from_str(v,"%Y-%m-%d").ok()) else { continue };
                let bucket = if period == "weekly" { date-chrono::Duration::days(date.weekday().num_days_from_monday().into()) } else { date.with_day(1).unwrap() };
                point["observed_date"] = point["date"].clone();
                point["date"] = json!(bucket.to_string());
                buckets.insert(bucket,point);
            }
            buckets.into_values().collect::<Vec<_>>()
        };
        let (sample,market) = if period == "weekly" || period == "monthly" { (aggregate(sample),aggregate(market)) } else { (sample,market) };
        let mut stmt = conn.prepare("WITH dates AS (SELECT DISTINCT trade_date FROM quote_daily WHERE source<>'demo' AND strftime('%w',trade_date) NOT IN ('0','6') AND NOT EXISTS(SELECT 1 FROM trading_calendar c WHERE c.trade_date=quote_daily.trade_date AND c.is_open=0) ORDER BY trade_date DESC LIMIT 2) SELECT i.symbol,i.name,i.industry FROM instruments i JOIN quote_daily q ON q.symbol=i.symbol JOIN quote_daily p ON p.symbol=i.symbol AND p.trade_date=(SELECT MIN(trade_date) FROM dates) WHERE q.trade_date=(SELECT MAX(trade_date) FROM dates) AND p.trade_date<q.trade_date AND q.source<>'demo' AND p.source<>'demo' AND p.close>0 AND q.volume>0 AND NOT EXISTS(SELECT 1 FROM trading_calendar c WHERE c.is_open=1 AND c.trade_date>p.trade_date AND c.trade_date<q.trade_date) ORDER BY i.symbol")?;
        let members = stmt.query_map([],|r| Ok(json!({"symbol":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"industry":r.get::<_,String>(2)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(envelope(json!({"sample":sample,"market":market,"members":members,"period":period})))
    }).await
}

async fn index_detail(
    State(state): State<AppState>,
    ApiPath(code): ApiPath<String>,
) -> Result<Json<ApiResponse<IndexDetail>>, AppError> {
    let conn = db(&state)?;
    let (name,): (String,) = conn
        .query_row(
            "SELECT name FROM index_catalog WHERE code = ?1",
            params![code],
            |r| Ok((r.get(0)?,)),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("指数不存在"))?;
    let (date, close, amount, amount_known): (String, f64, f64, i64) = conn.query_row("SELECT trade_date, close, amount, amount_known FROM index_quote WHERE index_code = ?1 AND strftime('%w',trade_date) NOT IN ('0','6') AND NOT EXISTS(SELECT 1 FROM trading_calendar c WHERE c.trade_date=index_quote.trade_date AND c.is_open=0) ORDER BY trade_date DESC LIMIT 1", params![code], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    let prev: Option<f64> = conn.query_row("SELECT close FROM index_quote WHERE index_code = ?1 AND trade_date < ?2 AND strftime('%w',trade_date) NOT IN ('0','6') AND NOT EXISTS(SELECT 1 FROM trading_calendar c WHERE c.trade_date=index_quote.trade_date AND c.is_open=0) ORDER BY trade_date DESC LIMIT 1", params![code, date], |r| r.get(0)).optional()?;
    let members = index_member_rows(&conn, &code, &date)?;
    Ok(envelope(IndexDetail {
        change_points: prev.map(|previous| close - previous),
        code,
        name,
        close,
        change_pct: pct(close, prev.unwrap_or(close)),
        amount,
        amount_known: amount_known != 0,
        as_of: date,
        members,
    }))
}

fn index_member_rows(
    conn: &Connection,
    code: &str,
    date: &str,
) -> Result<Vec<MemberRow>, AppError> {
    let mut stmt = conn.prepare("SELECT m.symbol, i.name, m.weight FROM index_member m JOIN instruments i ON i.symbol = m.symbol WHERE m.index_code = ?1 AND m.in_date <= ?2 AND (m.out_date IS NULL OR m.out_date > ?2) ORDER BY m.weight DESC")?;
    let rows = stmt
        .query_map(params![code, date], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, f64>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut out = Vec::new();
    for (symbol, name, weight) in rows {
        let quotes = two_quotes(conn, &symbol)?;
        if let Some(latest) = quotes.first() {
            out.push(MemberRow {
                symbol,
                name,
                weight,
                close: latest.close,
                change_pct: if let Some(prev) = quotes.get(1) {
                    pct(latest.close, prev.close)
                } else {
                    0.0
                },
            });
        }
    }
    Ok(out)
}

async fn index_members(
    State(state): State<AppState>,
    ApiPath(code): ApiPath<String>,
) -> Result<Json<ApiResponse<Vec<MemberRow>>>, AppError> {
    let conn = db(&state)?;
    let date = latest_date(&conn, None)?.unwrap_or_default();
    Ok(envelope(index_member_rows(&conn, &code, &date)?))
}

async fn sectors(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<SectorRow>>>, AppError> {
    let conn = db(&state)?;
    let as_of = latest_date(&conn, None)?.unwrap_or_default();
    let mut out = Vec::new();
    let mut stmt = conn.prepare("SELECT id, name, sector_type FROM sector_catalog ORDER BY id")?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (id, name, sector_type) in rows {
        let members = sector_member_rows(&conn, id, &as_of)?;
        let avg = if members.is_empty() {
            0.0
        } else {
            members.iter().map(|m| m.change_pct).sum::<f64>() / members.len() as f64
        };
        let leading = members.iter().max_by(|a, b| {
            a.change_pct
                .partial_cmp(&b.change_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out.push(SectorRow {
            id,
            name,
            sector_type,
            change_pct: avg,
            leading_symbol: leading.map(|m| m.symbol.clone()).unwrap_or_default(),
            leading_name: leading.map(|m| m.name.clone()).unwrap_or_default(),
            turnover: 0.0,
        });
    }
    out.sort_by(|a, b| {
        b.change_pct
            .partial_cmp(&a.change_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let _ = as_of;
    Ok(envelope(out))
}

fn sector_member_rows(
    conn: &Connection,
    sector_id: i64,
    date: &str,
) -> Result<Vec<MemberRow>, AppError> {
    let mut stmt = conn.prepare("SELECT m.symbol, i.name, m.weight FROM sector_member m JOIN instruments i ON i.symbol = m.symbol WHERE m.sector_id = ?1 AND m.in_date <= ?2 AND (m.out_date IS NULL OR m.out_date > ?2)")?;
    let rows = stmt
        .query_map(params![sector_id, date], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, f64>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut out = Vec::new();
    for (symbol, name, weight) in rows {
        let quotes = two_quotes(conn, &symbol)?;
        if let Some(latest) = quotes.first() {
            out.push(MemberRow {
                symbol,
                name,
                weight,
                close: latest.close,
                change_pct: if let Some(prev) = quotes.get(1) {
                    pct(latest.close, prev.close)
                } else {
                    0.0
                },
            });
        }
    }
    Ok(out)
}

async fn sector_detail(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> Result<Json<ApiResponse<SectorDetail>>, AppError> {
    let conn = db(&state)?;
    let (name, sector_type): (String, String) = conn
        .query_row(
            "SELECT name, sector_type FROM sector_catalog WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("板块不存在"))?;
    let as_of = latest_date(&conn, None)?.unwrap_or_default();
    let members = sector_member_rows(&conn, id, &as_of)?;
    let avg = if members.is_empty() {
        0.0
    } else {
        members.iter().map(|m| m.change_pct).sum::<f64>() / members.len() as f64
    };
    Ok(envelope(SectorDetail {
        id,
        name,
        sector_type,
        change_pct: avg,
        members,
    }))
}

async fn sector_members(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> Result<Json<ApiResponse<Vec<MemberRow>>>, AppError> {
    let conn = db(&state)?;
    let as_of = latest_date(&conn, None)?.unwrap_or_default();
    Ok(envelope(sector_member_rows(&conn, id, &as_of)?))
}

fn settings_from(conn: &Connection) -> Result<Settings, AppError> {
    let get = |key: &str| -> Result<String, AppError> {
        Ok(conn
            .query_row(
                "SELECT value FROM app_setting WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or_default())
    };
    Ok(Settings {
        theme: get("theme")?,
        up_color: get("up_color")?,
        default_home: get("default_home")?,
        timezone: get("timezone")?,
        density: get("density")?,
        poll_interval: get("poll_interval")?.parse().unwrap_or(30),
    })
}

async fn get_settings(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Settings>>, AppError> {
    let conn = db(&state)?;
    Ok(envelope(settings_from(&conn)?))
}

async fn update_settings(
    State(state): State<AppState>,
    ApiJson(body): ApiJson<UpdateSettingsBody>,
) -> Result<Json<ApiResponse<Settings>>, AppError> {
    let conn = db(&state)?;
    let current = settings_from(&conn)?;
    let next = Settings {
        theme: body.theme.unwrap_or(current.theme),
        up_color: body.up_color.unwrap_or(current.up_color),
        default_home: body.default_home.unwrap_or(current.default_home),
        timezone: body.timezone.unwrap_or(current.timezone),
        density: body.density.unwrap_or(current.density),
        poll_interval: body.poll_interval.unwrap_or(current.poll_interval),
    };
    if !matches!(next.theme.as_str(), "light" | "dark" | "system") {
        return Err(AppError::bad_request(
            "theme must be light, dark, or system",
        ));
    }
    if !matches!(next.up_color.as_str(), "red" | "green") {
        return Err(AppError::bad_request("up_color must be red or green"));
    }
    if !matches!(next.default_home.as_str(), "/" | "/watchlist" | "/data") {
        return Err(AppError::bad_request("invalid default_home"));
    }
    if !matches!(next.timezone.as_str(), "Asia/Shanghai" | "UTC") {
        return Err(AppError::bad_request("invalid timezone"));
    }
    if !matches!(next.density.as_str(), "comfortable" | "compact") {
        return Err(AppError::bad_request("invalid density"));
    }
    if !matches!(next.poll_interval, 0 | 15 | 30 | 60) {
        return Err(AppError::bad_request(
            "poll_interval must be 0, 15, 30, or 60",
        ));
    }
    let tx = conn.unchecked_transaction()?;
    for (k, v) in [
        ("theme", next.theme.clone()),
        ("up_color", next.up_color.clone()),
        ("default_home", next.default_home.clone()),
        ("timezone", next.timezone.clone()),
        ("density", next.density.clone()),
        ("poll_interval", next.poll_interval.to_string()),
    ] {
        tx.execute(
            "INSERT OR REPLACE INTO app_setting (key, value) VALUES (?1, ?2)",
            params![k, v],
        )?;
    }
    tx.commit()?;
    Ok(envelope(settings_from(&conn)?))
}

fn valid_group_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

fn watchlist_groups(conn: &Connection) -> Result<Vec<WatchlistGroup>, AppError> {
    let mut stmt =
        conn.prepare("SELECT id, name, sort, color FROM watchlist_group ORDER BY CASE WHEN id = 'watchlist-all' THEN 0 ELSE 1 END, sort, name")?;
    let groups = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut out = Vec::new();
    for (id, name, sort, color) in groups {
        let mut stmt = conn.prepare("SELECT w.id, w.symbol, i.name, w.note, w.sort FROM watchlist_item w JOIN instruments i ON i.symbol = w.symbol WHERE w.group_id = ?1 ORDER BY w.sort, w.created_at")?;
        let items = stmt
            .query_map(params![id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut item_rows = Vec::new();
        for (item_id, symbol, stock_name, note, sort) in items {
            let quotes = two_quotes(conn, &symbol)?;
            let (close, change_pct, as_of) = if let Some(latest) = quotes.first() {
                (
                    latest.close,
                    if let Some(prev) = quotes.get(1) {
                        pct(latest.close, prev.close)
                    } else {
                        0.0
                    },
                    latest.date.clone(),
                )
            } else {
                (0.0, 0.0, String::new())
            };
            item_rows.push(WatchlistItem {
                source: quotes
                    .first()
                    .map(|quote| quote.source.clone())
                    .unwrap_or_else(|| "pending".into()),
                id: item_id,
                symbol,
                name: stock_name,
                note,
                sort,
                close,
                change_pct,
                as_of,
            });
        }
        out.push(WatchlistGroup {
            id,
            name,
            color,
            sort,
            items: item_rows,
        });
    }
    Ok(out)
}

async fn watchlists(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<WatchlistGroup>>>, AppError> {
    let conn = db(&state)?;
    Ok(envelope(watchlist_groups(&conn)?))
}

async fn create_group(
    State(state): State<AppState>,
    ApiJson(body): ApiJson<CreateGroupBody>,
) -> Result<Json<ApiResponse<Vec<WatchlistGroup>>>, AppError> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::bad_request("分组名称不能为空"));
    }
    let color = body.color.as_deref().unwrap_or("#8a8175");
    if !valid_group_color(color) {
        return Err(AppError::bad_request("分组颜色必须是 #RRGGBB"));
    }
    let conn = db(&state)?;
    let id = format!("grp-{}", uuid::Uuid::new_v4());
    conn.execute("INSERT INTO watchlist_group (id, name, sort, color) VALUES (?1, ?2, (SELECT COALESCE(MAX(sort),0)+1 FROM watchlist_group), ?3)", params![id, name, color])?;
    Ok(envelope(watchlist_groups(&conn)?))
}

#[derive(Debug, serde::Deserialize)]
struct RealtimeQuery {
    symbols: Option<String>,
}

async fn realtime_quotes(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<RealtimeQuery>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, AppError> {
    if !crate::sources::axdata::enabled() {
        return Err(AppError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "AXDATA_UNAVAILABLE",
            "AxData 未配置",
        ));
    }
    let mut symbols: Vec<String> = query
        .symbols
        .unwrap_or_default()
        .split(',')
        .map(|symbol| symbol.trim().to_uppercase())
        .filter(|symbol| !symbol.is_empty())
        .collect();
    symbols.sort();
    symbols.dedup();
    if symbols.is_empty() {
        return Ok(envelope(vec![]));
    }
    if symbols.len() > 20 {
        return Err(AppError::bad_request("实时行情单次最多 20 只"));
    }
    let snapshots = crate::sources::axdata::snapshots(&state.sources.client, &symbols)
        .await
        .map_err(|error| {
            AppError::new(StatusCode::BAD_GATEWAY, "AXDATA_QUOTE_UNAVAILABLE", error)
        })?;
    if snapshots.len() != symbols.len() || snapshots.iter().any(|s| !symbols.contains(&s.symbol)) {
        return Err(AppError::invalid_data("上游报价未完整覆盖请求标的"));
    }
    let checked_at = now_iso();
    let out = snapshots
        .iter()
        .map(|s| {
            serde_json::json!({
                "symbol": s.symbol, "close": s.close, "pre_close": s.prev_close,
                "change_pct": s.change_pct, "volume": s.volume, "amount": s.amount,
                "updated_at": null, "provider_time": null, "time_status": "unknown",
                "last_checked_at": checked_at, "source": "axdata",
            })
        })
        .collect::<Vec<_>>();
    let prepared = crate::persistence::prepare_realtime(&snapshots)?;
    crate::persistence::run_db(state, move |conn| {
        crate::persistence::publish_prepared(conn, prepared, &checked_at)
    })
    .await?;
    let mut out = out;
    out.sort_by(|left, right| {
        left["symbol"]
            .as_str()
            .unwrap_or("")
            .cmp(right["symbol"].as_str().unwrap_or(""))
    });
    if out.is_empty() {
        return Err(AppError::new(
            StatusCode::BAD_GATEWAY,
            "AXDATA_QUOTE_UNAVAILABLE",
            "AxData 秒级行情暂不可用",
        ));
    }
    Ok(envelope(out))
}

async fn refresh_watchlists(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<WatchlistGroup>>>, AppError> {
    #[cfg(not(test))]
    refresh_market_sources(state.clone(), true);
    let conn = db(&state)?;
    Ok(envelope(watchlist_groups(&conn)?))
}

async fn rename_group(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<String>,
    ApiJson(body): ApiJson<RenameGroupBody>,
) -> Result<Json<ApiResponse<Vec<WatchlistGroup>>>, AppError> {
    if id == crate::db::WATCHLIST_ALL {
        return Err(AppError::bad_request(
            "全部自选是固定入口，不能重命名或修改颜色",
        ));
    }
    if body.name.is_none() && body.color.is_none() {
        return Err(AppError::bad_request("至少提供名称或颜色"));
    }
    let name = body.name.as_deref().map(str::trim);
    if name.is_some_and(str::is_empty) {
        return Err(AppError::bad_request("分组名称不能为空"));
    }
    if let Some(color) = body.color.as_deref() {
        if !valid_group_color(color) {
            return Err(AppError::bad_request("分组颜色必须是 #RRGGBB"));
        }
    }
    let conn = db(&state)?;
    let changed = conn.execute(
        "UPDATE watchlist_group SET name = COALESCE(?1, name), color = COALESCE(?2, color) WHERE id = ?3",
        params![name, body.color.as_deref(), id],
    )?;
    if changed == 0 {
        return Err(AppError::not_found("分组不存在"));
    }
    Ok(envelope(watchlist_groups(&conn)?))
}

async fn reorder_items(
    State(state): State<AppState>,
    ApiJson(body): ApiJson<ReorderItemsBody>,
) -> Result<Json<ApiResponse<Vec<WatchlistGroup>>>, AppError> {
    let conn = db(&state)?;
    let group_exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM watchlist_group WHERE id = ?1",
            params![body.group_id],
            |r| r.get(0),
        )
        .optional()?;
    if group_exists.is_none() {
        return Err(AppError::not_found("分组不存在"));
    }
    for (index, id) in body.ids.iter().enumerate() {
        conn.execute(
            "UPDATE watchlist_item SET sort = ?1 WHERE id = ?2 AND group_id = ?3",
            params![index as i64, id, body.group_id],
        )?;
    }
    Ok(envelope(watchlist_groups(&conn)?))
}

async fn delete_group(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> Result<Json<ApiResponse<Vec<WatchlistGroup>>>, AppError> {
    let conn = db(&state)?;
    if id == crate::db::WATCHLIST_ALL {
        return Err(AppError::bad_request("不能删除全部自选入口"));
    }
    let changed = conn.execute("DELETE FROM watchlist_group WHERE id = ?1", params![id])?;
    if changed == 0 {
        return Err(AppError::not_found("分组不存在"));
    }
    Ok(envelope(watchlist_groups(&conn)?))
}

async fn clear_group(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> Result<Json<ApiResponse<Vec<WatchlistGroup>>>, AppError> {
    let conn = db(&state)?;
    let exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM watchlist_group WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        return Err(AppError::not_found("分组不存在"));
    }
    if id == crate::db::WATCHLIST_ALL {
        conn.execute("DELETE FROM watchlist_item", [])?;
    } else {
        conn.execute(
            "DELETE FROM watchlist_item WHERE group_id = ?1",
            params![id],
        )?;
    }
    Ok(envelope(watchlist_groups(&conn)?))
}

async fn add_item(
    State(state): State<AppState>,
    ApiJson(body): ApiJson<AddItemBody>,
) -> Result<Json<ApiResponse<Vec<WatchlistGroup>>>, AppError> {
    let conn = db(&state)?;
    get_instrument(&conn, &body.symbol)?;
    let tx = conn.unchecked_transaction()?;
    let group_exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM watchlist_group WHERE id=?1)",
        [&body.group_id],
        |r| r.get(0),
    )?;
    if !group_exists {
        return Err(AppError::not_found("分组不存在"));
    }
    let note = body.note.unwrap_or_default();
    // Join the collection first. Assigning to a group never replaces its note.
    for group_id in [crate::db::WATCHLIST_ALL, body.group_id.as_str()] {
        let id = format!("item-{}", uuid::Uuid::new_v4());
        tx.execute("INSERT OR IGNORE INTO watchlist_item (id, group_id, symbol, note, sort, created_at) VALUES (?1, ?2, ?3, ?4, (SELECT COALESCE(MAX(sort),0)+1 FROM watchlist_item WHERE group_id = ?2), ?5)", params![id, group_id, body.symbol, note, now_iso()])?;
    }
    tx.commit()?;
    Ok(envelope(watchlist_groups(&conn)?))
}

async fn update_item(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<String>,
    ApiJson(body): ApiJson<UpdateItemBody>,
) -> Result<Json<ApiResponse<Vec<WatchlistGroup>>>, AppError> {
    let conn = db(&state)?;
    if let Some(note) = body.note {
        conn.execute(
            "UPDATE watchlist_item SET note = ?1 WHERE id = ?2",
            params![note, id],
        )?;
    }
    if let Some(sort) = body.sort {
        conn.execute(
            "UPDATE watchlist_item SET sort = ?1 WHERE id = ?2",
            params![sort, id],
        )?;
    }
    Ok(envelope(watchlist_groups(&conn)?))
}

async fn delete_item(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<String>,
) -> Result<Json<ApiResponse<Vec<WatchlistGroup>>>, AppError> {
    let conn = db(&state)?;
    let tx = conn.unchecked_transaction()?;
    let item: Option<(String, String)> = tx
        .query_row(
            "SELECT group_id,symbol FROM watchlist_item WHERE id=?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((group_id, symbol)) = item {
        if group_id == crate::db::WATCHLIST_ALL {
            tx.execute("DELETE FROM watchlist_item WHERE symbol=?1", [symbol])?;
        } else {
            tx.execute("DELETE FROM watchlist_item WHERE id=?1", [id])?;
        }
    }
    tx.commit()?;
    Ok(envelope(watchlist_groups(&conn)?))
}

async fn data_jobs(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<DataJob>>>, AppError> {
    let conn = db(&state)?;
    Ok(envelope(crate::sync::list_jobs(&conn)?))
}

async fn run_job(
    State(state): State<AppState>,
    ApiJson(body): ApiJson<RunJobBody>,
) -> Result<Json<ApiResponse<Vec<DataJob>>>, AppError> {
    let conn = db(&state)?;
    Ok(envelope(crate::sync::run_request(
        &conn,
        &body.dataset,
        body.request_id.as_deref(),
    )?))
}

fn mask_secret(value: &str) -> String {
    if value.is_empty() {
        "未配置".into()
    } else if value.len() <= 4 {
        "***".into()
    } else {
        format!("***{}", &value[value.len() - 4..])
    }
}

async fn run_backfill(
    State(state): State<AppState>,
    ApiJson(body): ApiJson<BackfillBody>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let conn = db(&state)?;
    let changed = crate::sync::backfill(
        &conn,
        &body.dataset,
        &body.start,
        &body.end,
        body.operator.trim(),
    )?;
    Ok(envelope(serde_json::json!({
        "dataset": body.dataset,
        "start": body.start,
        "end": body.end,
        "changed": changed
    })))
}

async fn source_status(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let (freshness, retries) = {
        let conn = db(&state)?;
        (
            crate::sources::store::freshness(&conn)?,
            crate::sources::store::pending_retries(&conn, None)?,
        )
    };
    let policies: Vec<_> = ["quote_daily", "snapshot", "index_quote"]
        .into_iter()
        .map(|dataset| {
            serde_json::json!({
                "dataset": dataset,
                "priority": crate::sources::SourceHub::priority(dataset)
                    .iter()
                    .map(|source| serde_json::json!({"id": source.as_str(), "label": source.label()}))
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(envelope(serde_json::json!({
        "policies": policies,
        "health": state.sources.health(),
        "freshness": freshness,
        "retry_queue": retries
            .iter()
            .map(|task| serde_json::json!({
                "dataset": task.dataset,
                "symbol": task.symbol,
                "start": task.start,
                "end": task.end,
                "attempts": task.attempts,
                "last_error": task.last_error,
            }))
            .collect::<Vec<_>>(),
        "mock_fallback": false,
    })))
}

async fn source_probe(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<SourceProbeQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let symbol = query
        .symbol
        .as_deref()
        .unwrap_or("600519.SH")
        .trim()
        .to_string();
    let end = query.end.clone().unwrap_or_else(|| {
        let conn = state.db.lock().unwrap();
        latest_date(&conn, None)
            .ok()
            .flatten()
            .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string())
    });
    let start = query.start.clone().unwrap_or_else(|| {
        chrono::NaiveDate::parse_from_str(&end, "%Y-%m-%d")
            .map(|date| {
                (date - chrono::Duration::days(7))
                    .format("%Y-%m-%d")
                    .to_string()
            })
            .unwrap_or_else(|_| end.clone())
    });
    if chrono::NaiveDate::parse_from_str(&start, "%Y-%m-%d").is_err()
        || chrono::NaiveDate::parse_from_str(&end, "%Y-%m-%d").is_err()
        || start > end
    {
        return Err(AppError::bad_request("invalid date range"));
    }
    let daily = state.sources.probe_daily(&symbol, &start, &end).await;
    let snapshot = state.sources.probe_snapshot(&symbol).await;
    let snapshot: Vec<_> = snapshot
        .into_iter()
        .map(|(source, result)| match result {
            Ok(data) => serde_json::json!({"source": source, "ok": true, "data": data}),
            Err(error) => serde_json::json!({"source": source, "ok": false, "error": error}),
        })
        .collect();
    Ok(envelope(serde_json::json!({
        "symbol": symbol,
        "start": start,
        "end": end,
        "daily": daily,
        "snapshot": snapshot,
    })))
}

fn expected_dates(conn: &Connection, start: &str, end: &str) -> Result<Vec<String>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT trade_date FROM trading_calendar WHERE is_open = 1 AND trade_date BETWEEN ?1 AND ?2 ORDER BY trade_date",
    )?;
    let rows = stmt.query_map(params![start, end], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn missing_dates(expected: &[String], bars: &[crate::sources::DailyBar]) -> Vec<String> {
    expected
        .iter()
        .filter(|date| !bars.iter().any(|bar| &bar.date == *date))
        .cloned()
        .collect()
}

fn merge_bars(target: &mut Vec<crate::sources::DailyBar>, extra: Vec<crate::sources::DailyBar>) {
    for bar in extra {
        if let Some(existing) = target.iter_mut().find(|item| item.date == bar.date) {
            *existing = bar;
        } else {
            target.push(bar);
        }
    }
    target.sort_by(|a, b| a.date.cmp(&b.date));
}

pub(crate) async fn sync_daily_symbol(
    state: &AppState,
    dataset: &str,
    symbol: &str,
    start: &str,
    end: &str,
    batch_id: &str,
    with_adj: bool,
) -> Result<serde_json::Value, String> {
    let first = state
        .sources
        .fetch_daily(dataset, symbol, start, end, with_adj)
        .await
        .map_err(|failure| failure.message())?;
    let mut all_bars = first.data.clone();
    let mut sources = vec![first.source.as_str().to_string()];
    let mut adj_pending: usize;
    {
        let conn = db(state).map_err(|error| error.message)?;
        let result = if dataset == "index_quote" {
            crate::sources::store::publish_index(
                &conn,
                &first,
                std::slice::from_ref(&symbol.to_string()),
                start,
                end,
                batch_id,
            )
        } else {
            crate::sources::store::publish_daily(
                &conn,
                &first,
                std::slice::from_ref(&symbol.to_string()),
                start,
                end,
                batch_id,
            )
        }
        .map_err(|error| error.message)?;
        adj_pending = result.adj_pending;
    }
    let expected = {
        let conn = db(state).map_err(|error| error.message)?;
        expected_dates(&conn, start, end).map_err(|error| error.message)?
    };
    let mut gaps = missing_dates(&expected, &all_bars);
    for source in crate::sources::SourceHub::priority(dataset) {
        if gaps.is_empty() {
            break;
        }
        if *source == first.source {
            continue;
        }
        let gap_start = gaps.first().unwrap().clone();
        let gap_end = gaps.last().unwrap().clone();
        if let Ok(extra) = state
            .sources
            .fetch_daily_from(*source, dataset, symbol, &gap_start, &gap_end, with_adj)
            .await
        {
            let extra_id = format!("{batch_id}-{}", source.as_str());
            let result = {
                let conn = db(state).map_err(|error| error.message)?;
                if dataset == "index_quote" {
                    crate::sources::store::publish_index(
                        &conn,
                        &extra,
                        std::slice::from_ref(&symbol.to_string()),
                        &gap_start,
                        &gap_end,
                        &extra_id,
                    )
                } else {
                    crate::sources::store::publish_daily(
                        &conn,
                        &extra,
                        std::slice::from_ref(&symbol.to_string()),
                        &gap_start,
                        &gap_end,
                        &extra_id,
                    )
                }
                .map_err(|error| error.message)?
            };
            adj_pending += result.adj_pending;
            sources.push(source.as_str().to_string());
            merge_bars(&mut all_bars, extra.data);
            gaps = missing_dates(&expected, &all_bars);
        }
    }
    let gap_count = gaps.len();
    if gap_count > 0 {
        let conn = db(state).map_err(|error| error.message)?;
        crate::sources::store::queue_retry(
            &conn,
            dataset,
            symbol,
            start,
            end,
            &format!("{gap_count} trading days missing"),
        )
        .map_err(|error| error.message)?;
    }
    Ok(serde_json::json!({
        "symbol": symbol,
        "ok": true,
        "source": sources.join("+"),
        "rows": all_bars.len(),
        "batch_id": batch_id,
        "adj_pending": adj_pending,
        "gap_count": gap_count,
    }))
}

async fn source_sync(
    State(state): State<AppState>,
    ApiJson(body): ApiJson<SourceSyncBody>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    if !matches!(
        body.dataset.as_str(),
        "quote_daily" | "index_quote" | "snapshot"
    ) {
        return Err(AppError::bad_request(
            "dataset must be quote_daily, index_quote, or snapshot",
        ));
    }
    if body.symbols.is_empty() || body.symbols.len() > 20 {
        return Err(AppError::bad_request("symbols must contain 1..20 entries"));
    }
    if chrono::NaiveDate::parse_from_str(&body.start, "%Y-%m-%d").is_err()
        || chrono::NaiveDate::parse_from_str(&body.end, "%Y-%m-%d").is_err()
        || body.start > body.end
    {
        return Err(AppError::bad_request("invalid date range"));
    }
    let symbols: Vec<String> = body
        .symbols
        .iter()
        .map(|symbol| symbol.trim().to_string())
        .collect();
    let batch_id = format!(
        "src-{}-{}",
        body.dataset,
        body.request_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
    );
    let mut results = Vec::new();
    let mut success = 0usize;
    let mut errors = Vec::new();
    let mut sources = std::collections::HashSet::new();
    let mut gaps_total = 0usize;
    for symbol in &symbols {
        let outcome: Result<serde_json::Value, String> = async {
            match body.dataset.as_str() {
                "quote_daily" | "index_quote" => {
                    let dataset = body.dataset.as_str();
                    let with_adj = dataset == "quote_daily";
                    match sync_daily_symbol(
                        &state,
                        dataset,
                        symbol,
                        &body.start,
                        &body.end,
                        &batch_id,
                        with_adj,
                    )
                    .await
                    {
                        Ok(value) => {
                            let conn = db(&state).map_err(|error| error.message)?;
                            if value["gap_count"].as_u64().unwrap_or(0) == 0 {
                                crate::sources::store::clear_retry(
                                    &conn,
                                    dataset,
                                    symbol,
                                    &body.start,
                                    &body.end,
                                )
                                .map_err(|error| error.message)?;
                            }
                            Ok(value)
                        }
                        Err(error) => {
                            let conn = db(&state).map_err(|db_error| db_error.message)?;
                            crate::sources::store::queue_retry(
                                &conn,
                                dataset,
                                symbol,
                                &body.start,
                                &body.end,
                                &error,
                            )
                            .map_err(|queue_error| queue_error.message)?;
                            Err(error)
                        }
                    }
                }
                "snapshot" => {
                    let batch = state
                        .sources
                        .fetch_snapshot(symbol)
                        .await
                        .map_err(|failure| failure.message())?;
                    let source = batch.source.as_str().to_string();
                    {
                        let conn = db(&state).map_err(|error| error.message)?;
                        crate::sources::store::publish_snapshot(&conn, &batch, &batch_id)
                            .map_err(|error| error.message)?;
                        crate::sources::store::clear_retry(&conn, "snapshot", symbol, "", "")
                            .map_err(|error| error.message)?;
                    }
                    Ok(serde_json::json!({
                        "symbol": symbol, "ok": true, "source": source,
                        "trade_date": batch.data.date, "batch_id": batch_id,
                    }))
                }
                _ => unreachable!(),
            }
        }
        .await;
        match outcome {
            Ok(value) => {
                if let Some(source) = value["source"].as_str() {
                    sources.insert(source.to_string());
                }
                gaps_total += value["gap_count"].as_u64().unwrap_or(0) as usize;
                success += 1;
                results.push(value);
            }
            Err(error) => {
                if body.dataset == "snapshot" {
                    if let Ok(conn) = db(&state) {
                        let _ = crate::sources::store::queue_retry(
                            &conn, "snapshot", symbol, "", "", &error,
                        );
                    }
                }
                errors.push(format!("{symbol}: {error}"));
                results.push(serde_json::json!({"symbol": symbol, "ok": false, "error": error}));
            }
        }
    }
    let pending = {
        let conn = db(&state)?;
        crate::sources::store::has_pending(&conn, &body.dataset).unwrap_or(true)
    };
    let status = if success == symbols.len() && gaps_total == 0 && !pending {
        "fresh"
    } else if success > 0 {
        "partial"
    } else {
        "stale"
    };
    {
        let conn = db(&state)?;
        if status == "fresh" {
            let source = if sources.len() == 1 {
                sources.iter().next().cloned().unwrap_or_default()
            } else {
                "mixed".into()
            };
            crate::sources::store::mark_fresh(&conn, &body.dataset, &batch_id, &source)?;
        } else if status == "partial" {
            crate::sources::store::mark_partial(&conn, &body.dataset, &errors.join(" | "))?;
        } else {
            crate::sources::store::mark_stale(&conn, &body.dataset, &errors.join(" | "))?;
        }
    }
    Ok(envelope(serde_json::json!({
        "dataset": body.dataset,
        "batch_id": batch_id,
        "status": status,
        "success": success,
        "total": symbols.len(),
        "results": results,
        "errors": errors,
        "gap_count": gaps_total,
    })))
}

async fn data_sources(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<DataSourceConfig>>>, AppError> {
    let conn = db(&state)?;
    let mut stmt = conn.prepare(
        "SELECT id, provider, enabled, base_url, secret_env, updated_at FROM data_source_config ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        let secret_env: String = r.get(4)?;
        let secret = std::env::var(&secret_env).unwrap_or_default();
        Ok(DataSourceConfig {
            id: r.get(0)?,
            provider: r.get(1)?,
            enabled: r.get::<_, i64>(2)? == 1,
            base_url: r.get(3)?,
            secret_env,
            secret_configured: !secret.is_empty(),
            secret_masked: mask_secret(&secret),
            updated_at: r.get(5)?,
        })
    })?;
    Ok(envelope(rows.collect::<rusqlite::Result<Vec<_>>>()?))
}

async fn quality(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<QualityReport>>>, AppError> {
    let conn = db(&state)?;
    Ok(envelope(crate::sync::recompute_quality(&conn)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn index_chart_uses_real_ohlcv_and_aggregates_months() {
        let state = crate::db::test_state();
        {
            let conn = state.db.lock().unwrap();
            conn.execute("DELETE FROM index_quote WHERE index_code='000001.SH'", [])
                .unwrap();
            for (date, source, close) in [
                ("2026-10-21", "demo", 9.0),
                ("2026-10-22", "fixture", 11.0),
                ("2026-10-23", "fixture", 12.0),
                ("2026-10-25", "fixture", 99.0),
            ] {
                conn.execute("INSERT INTO index_quote(index_code,trade_date,open,high,low,close,volume,amount,source) VALUES('000001.SH',?1,10,100,9,?2,100,1000,?3)",params![date,close,source]).unwrap();
            }
        }
        let result = index_bars(
            State(state.clone()),
            ApiPath("000001.SH".into()),
            Query(BarsQuery {
                period: Some("daily".into()),
                adjust: None,
            }),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert_eq!(result.symbol, "000001.SH");
        assert_eq!(result.bars.len(), 2);
        assert_eq!(result.bars[1].date, "2026-10-23");
        let result = index_bars(
            State(state.clone()),
            ApiPath("000001.SH".into()),
            Query(BarsQuery {
                period: Some("monthly".into()),
                adjust: None,
            }),
        )
        .await
        .unwrap()
        .0
        .data
        .unwrap();
        assert_eq!(result.bars.len(), 1);
        assert_eq!(result.bars[0].volume, 200.0);
        assert_eq!(result.bars[0].close, 12.0);
        assert!(index_bars(
            State(state),
            ApiPath("000001.SH".into()),
            Query(BarsQuery {
                period: Some("3s".into()),
                adjust: None
            })
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn breadth_comparison_uses_real_previous_session_and_latest_observation() {
        let state = crate::db::test_state();
        {
            let conn = state.db.lock().unwrap();
            for (date, close) in [
                ("2026-10-22", 10.0),
                ("2026-10-23", 11.0),
                ("2026-10-25", 12.0),
            ] {
                conn.execute("INSERT OR REPLACE INTO quote_daily(symbol,trade_date,open,high,low,close,volume,amount,source) VALUES('600519.SH',?1,10,12,9,?2,100,1000,'fixture')",params![date,close]).unwrap();
            }
            for (time, advance) in [("2026-10-23 10:00:00", 10), ("2026-10-23 15:00:00", 20)] {
                let payload =
                    serde_json::json!({"advance":advance,"decline":5,"flat":1}).to_string();
                conn.execute("INSERT INTO market_breadth_observation(trade_date,source,scope,provider_time,observed_at,available_at,phase,payload,observation_hash) VALUES('2026-10-23','fixture','market',?1,1,1,'close',?2,?1)",params![time,payload]).unwrap();
            }
        }
        let value = breadth_history(State(state), ApiQuery(BreadthQuery::default()))
            .await
            .unwrap()
            .0
            .data
            .unwrap();
        let sample = value["sample"].as_array().unwrap();
        assert_eq!(sample.len(), 1);
        assert_eq!(sample[0]["date"], "2026-10-23");
        assert_eq!(sample[0]["advance"], 1);
        assert_eq!(value["market"].as_array().unwrap().len(), 1);
        assert_eq!(value["market"][0]["advance"], 20);
    }

    #[tokio::test]
    async fn breadth_periods_preserve_observation_time_and_last_value() {
        let state = crate::db::test_state();
        {
            let conn = state.db.lock().unwrap();
            for (time, advance) in [
                ("2026-10-22 10:00:00", 10),
                ("2026-10-23 10:05:00", 20),
                ("2026-10-23 10:25:00", 30),
                ("2026-10-23 10:35:00", 40),
            ] {
                let payload = json!({"advance":advance,"decline":5,"flat":1}).to_string();
                conn.execute("INSERT INTO market_breadth_observation(trade_date,source,scope,provider_time,observed_at,available_at,phase,payload,observation_hash) VALUES(?1,'fixture','market',?2,1,1,'intraday',?3,?2)",params![&time[..10],time,payload]).unwrap();
            }
        }
        for (period, length) in [
            ("30min", 3),
            ("1h", 2),
            ("daily", 2),
            ("weekly", 1),
            ("monthly", 1),
        ] {
            let result = breadth_history(
                State(state.clone()),
                ApiQuery(BreadthQuery {
                    period: Some(period.into()),
                }),
            )
            .await
            .unwrap()
            .0
            .data
            .unwrap();
            let market = result["market"].as_array().unwrap();
            assert_eq!(market.len(), length, "{period}");
            assert_eq!(
                market.last().unwrap()["advance"],
                40,
                "last observation, never sum"
            );
            if period == "30min" {
                assert_eq!(market[1]["advance"], 30);
            }
            if period == "weekly" {
                assert_eq!(market[0]["observed_date"], "2026-10-23");
            }
            assert!(
                result["sample"].as_array().unwrap().is_empty(),
                "missing samples must not be fabricated"
            );
        }
        assert!(breadth_history(
            State(state),
            ApiQuery(BreadthQuery {
                period: Some("3s".into())
            })
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn last_custom_group_can_be_deleted_but_collection_is_retained() {
        let state = crate::db::test_state();
        {
            let conn = state.db.lock().unwrap();
            conn.execute(
                "INSERT INTO watchlist_group (id, name, sort) VALUES ('grp-test', 'test', 99)",
                [],
            )
            .unwrap();
        }
        let _ = delete_group(State(state.clone()), ApiPath("grp-default".into()))
            .await
            .unwrap();
        {
            let conn = state.db.lock().unwrap();
            let orphaned: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM watchlist_item WHERE group_id = 'grp-default'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(orphaned, 0);
            conn.execute(
                "DELETE FROM watchlist_group WHERE id NOT IN ('grp-test','watchlist-all')",
                [],
            )
            .unwrap();
        }
        delete_group(State(state.clone()), ApiPath("grp-test".into()))
            .await
            .unwrap();
        assert!(
            delete_group(State(state), ApiPath(crate::db::WATCHLIST_ALL.into()))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn health_and_search_work() {
        let app = build_app(crate::db::test_state());
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/instruments/search?q=gzmt")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("600519.SH"));
    }

    #[tokio::test]
    async fn financial_versions_are_point_in_time() {
        let app = build_app(crate::db::test_state());
        let historical = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/instruments/600519.SH/fundamentals?as_of=2026-08-01")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(historical.status(), StatusCode::OK);
        let body = axum::body::to_bytes(historical.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("\"version\":1"), "{text}");
        assert!(
            !text.contains("\"version\":2"),
            "future correction leaked: {text}"
        );

        let current = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/instruments/600519.SH/fundamentals")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(current.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(
            text.contains("\"version\":2"),
            "current correction missing: {text}"
        );
    }

    #[tokio::test]
    async fn sync_job_is_idempotent_and_quality_is_fresh() {
        let app = build_app(crate::db::test_state());
        let body = r#"{"dataset":"quote_daily","request_id":"test-run-1"}"#;
        for _ in 0..2 {
            let res = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/v1/admin/data-jobs")
                        .header("content-type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK);
        }
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/admin/data-jobs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let jobs: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let quote_jobs = jobs["data"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|job| job["dataset"] == "quote_daily")
            .filter(|job| job["id"].as_str().unwrap_or("").starts_with("job-"))
            .filter(|job| !job["id"].as_str().unwrap_or("").starts_with("job-seed"))
            .count();
        assert_eq!(quote_jobs, 1, "job must be idempotent: {jobs}");

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/admin/quality")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let quality: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(quality["data"].as_array().unwrap().len() >= 6);

        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/admin/data-jobs")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"dataset":"nope"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn overview_snapshot_is_cached() {
        let app = build_app(crate::db::test_state());
        let mut stamps = Vec::new();
        for uri in ["/api/v1/market/overview", "/api/v1/market/overview"] {
            let res = app
                .clone()
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            let body = axum::body::to_bytes(res.into_body(), usize::MAX)
                .await
                .unwrap();
            let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
            stamps.push(value["data"]["updated_at"].as_str().unwrap().to_string());
        }
        assert_eq!(stamps[0], stamps[1], "overview should read the snapshot");

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/market/overview?refresh=true")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn error_contract_covers_404_validation_and_unimplemented() {
        let app = build_app(crate::db::test_state());
        for (uri, expected) in [
            ("/api/v1/nope", StatusCode::NOT_FOUND),
            ("/api/v1/instruments/999999.SH", StatusCode::NOT_FOUND),
            ("/api/v1/strategies", StatusCode::NOT_FOUND),
        ] {
            let res = app
                .clone()
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(res.status(), expected, "uri {uri}");
        }
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/admin/data-jobs")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"dataset":"bad"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn watchlist_supports_rename_reorder_and_refresh() {
        let app = build_app(crate::db::test_state());
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/watchlists")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let groups: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let default = groups["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["id"] != crate::db::WATCHLIST_ALL)
            .unwrap();
        let id = default["id"].as_str().unwrap();
        let ids: Vec<String> = default["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_str().unwrap().to_string())
            .collect();
        let mut reversed = ids.clone();
        reversed.reverse();
        let payload = serde_json::json!({"group_id": id, "ids": reversed});
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/v1/watchlists/items/reorder")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let after: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let first = after["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["id"] == id)
            .unwrap()["items"][0]["id"]
            .as_str()
            .unwrap();
        assert_eq!(first, reversed[0]);

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/v1/watchlists/groups/{id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"重命名测试"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/watchlists/refresh")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn watchlist_group_color_and_clear_items() {
        let app = build_app(crate::db::test_state());
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/watchlists")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let groups: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let id = groups["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["id"] != crate::db::WATCHLIST_ALL)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/v1/watchlists/groups/{id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r##"{"color":"#2f6f5e"}"##))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/v1/watchlists/groups/{id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r##"{"color":"bad"}"##))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        let res = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v1/watchlists/groups/{id}/items"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let group = value["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|group| group["id"] == id)
            .unwrap();
        assert_eq!(group["color"], "#2f6f5e");
        assert_eq!(group["items"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn watchlist_collection_survives_without_custom_groups() {
        async fn call(
            app: &Router,
            method: &str,
            uri: &str,
            body: serde_json::Value,
            status: StatusCode,
        ) -> serde_json::Value {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(uri)
                        .header("content-type", "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), status, "{method} {uri}");
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            serde_json::from_slice(&bytes).unwrap()
        }
        let state = crate::db::test_state();
        let app = build_app(state.clone());
        let before = watchlist_groups(&state.db.lock().unwrap()).unwrap()[0]
            .items
            .len();
        for id in ["grp-default", "grp-banks"] {
            call(
                &app,
                "DELETE",
                &format!("/api/v1/watchlists/groups/{id}"),
                serde_json::Value::Null,
                StatusCode::OK,
            )
            .await;
        }
        let data = call(
            &app,
            "GET",
            "/api/v1/watchlists",
            serde_json::Value::Null,
            StatusCode::OK,
        )
        .await;
        assert_eq!(data["data"].as_array().unwrap().len(), 1);
        assert_eq!(data["data"][0]["id"], crate::db::WATCHLIST_ALL);
        assert_eq!(data["data"][0]["items"].as_array().unwrap().len(), before);
        for (method, body) in [
            ("DELETE", serde_json::Value::Null),
            ("PUT", serde_json::json!({"name":"rename"})),
        ] {
            call(
                &app,
                method,
                "/api/v1/watchlists/groups/watchlist-all",
                body,
                StatusCode::BAD_REQUEST,
            )
            .await;
        }
        call(
            &app,
            "DELETE",
            "/api/v1/watchlists/groups/watchlist-all/items",
            serde_json::Value::Null,
            StatusCode::OK,
        )
        .await;
        let added=call(&app,"POST","/api/v1/watchlists/items",serde_json::json!({"group_id":"watchlist-all","symbol":"600519.SH","note":"collection note"}),StatusCode::OK).await;
        let item_id = added["data"][0]["items"][0]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let created = call(
            &app,
            "POST",
            "/api/v1/watchlists/groups",
            serde_json::json!({"name":"test"}),
            StatusCode::OK,
        )
        .await;
        let group_id = created["data"][1]["id"].as_str().unwrap().to_string();
        for _ in 0..2 {
            let result = call(
                &app,
                "POST",
                "/api/v1/watchlists/items",
                serde_json::json!({"group_id":group_id,"symbol":"600519.SH","note":"group note"}),
                StatusCode::OK,
            )
            .await;
            assert_eq!(result["data"][0]["items"].as_array().unwrap().len(), 1);
            assert_eq!(result["data"][0]["items"][0]["note"], "collection note");
            assert_eq!(result["data"][1]["items"].as_array().unwrap().len(), 1);
        }
        call(
            &app,
            "DELETE",
            &format!("/api/v1/watchlists/groups/{group_id}/items"),
            serde_json::Value::Null,
            StatusCode::OK,
        )
        .await;
        assert_eq!(
            watchlist_groups(&state.db.lock().unwrap()).unwrap()[0]
                .items
                .len(),
            1
        );
        call(
            &app,
            "POST",
            "/api/v1/watchlists/items",
            serde_json::json!({"group_id":group_id,"symbol":"600519.SH"}),
            StatusCode::OK,
        )
        .await;
        let removed = call(
            &app,
            "DELETE",
            &format!("/api/v1/watchlists/items/{item_id}"),
            serde_json::Value::Null,
            StatusCode::OK,
        )
        .await;
        assert!(removed["data"]
            .as_array()
            .unwrap()
            .iter()
            .all(|g| g["items"].as_array().unwrap().is_empty()));
        // An invalid group must not leave a partially added collection entry.
        call(
            &app,
            "POST",
            "/api/v1/watchlists/items",
            serde_json::json!({"group_id":"missing","symbol":"600519.SH"}),
            StatusCode::NOT_FOUND,
        )
        .await;
        assert!(watchlist_groups(&state.db.lock().unwrap()).unwrap()[0]
            .items
            .is_empty());
    }

    #[tokio::test]
    async fn watchlist_survives_database_reopen() {
        let path =
            std::env::temp_dir().join(format!("alphak-watchlist-{}.db", uuid::Uuid::new_v4()));
        let path_str = path.to_str().unwrap().to_string();
        {
            let conn = crate::db::init_db(&path_str).unwrap();
            let state = crate::db::AppState {
                db: std::sync::Arc::new(std::sync::Mutex::new(conn)),
                sources: std::sync::Arc::new(crate::sources::SourceHub::new()),
            };
            let app = build_app(state);
            let res = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/v1/watchlists/items")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            r#"{"group_id":"grp-default","symbol":"000333.SZ","note":"重开测试"}"#,
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK);
            let res = app
                .oneshot(
                    Request::builder()
                        .method("PUT")
                        .uri("/api/v1/watchlists/groups/grp-default")
                        .header("content-type", "application/json")
                        .body(Body::from(r#"{"name":"重开分组"}"#))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK);
        }
        {
            let conn = crate::db::init_db(&path_str).unwrap();
            let state = crate::db::AppState {
                db: std::sync::Arc::new(std::sync::Mutex::new(conn)),
                sources: std::sync::Arc::new(crate::sources::SourceHub::new()),
            };
            let app = build_app(state);
            let res = app
                .oneshot(
                    Request::builder()
                        .uri("/api/v1/watchlists")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let body = axum::body::to_bytes(res.into_body(), usize::MAX)
                .await
                .unwrap();
            let text = String::from_utf8(body.to_vec()).unwrap();
            assert!(text.contains("000333.SZ"));
            assert!(text.contains("重开测试"));
            assert!(text.contains("重开分组"));
        }
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(format!("{path_str}-wal"));
        let _ = std::fs::remove_file(format!("{path_str}-shm"));
    }

    #[tokio::test]
    async fn openapi_document_is_reachable() {
        let app = build_app(crate::db::test_state());
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/openapi.json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("\"openapi\": \"3.1.0\""));
        assert!(text.contains("/api/v1/instruments/{symbol}/fundamentals"));
    }

    #[tokio::test]
    async fn bars_adjust_uses_adj_factor_through_route() {
        let state = crate::db::test_state();
        {
            let conn = state.db.lock().unwrap();
            conn.execute(
                "UPDATE quote_daily SET adj_factor = 2.0 WHERE symbol = '600519.SH' AND trade_date = (SELECT MAX(trade_date) FROM quote_daily WHERE symbol = '600519.SH')",
                [],
            )
            .unwrap();
        }
        let app = build_app(state);
        let mut closes: Vec<(String, f64, f64)> = Vec::new();
        for mode in ["none", "qfq", "hfq"] {
            let uri = format!("/api/v1/instruments/600519.SH/bars?period=daily&adjust={mode}");
            let res = app
                .clone()
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK);
            let body = axum::body::to_bytes(res.into_body(), usize::MAX)
                .await
                .unwrap();
            let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
            let bars = value["data"]["bars"].as_array().unwrap();
            let first = bars.first().unwrap()["close"].as_f64().unwrap();
            let last = bars.last().unwrap()["close"].as_f64().unwrap();
            closes.push((mode.to_string(), first, last));
        }
        let none = &closes[0];
        let qfq = &closes[1];
        let hfq = &closes[2];
        assert!((qfq.2 - none.2).abs() < 1e-6);
        assert!((qfq.1 - none.1 / 2.0).abs() < 1e-6);
        assert!((hfq.1 - none.1).abs() < 1e-6);
        assert!((hfq.2 - none.2 * 2.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn fundamentals_as_of_does_not_leak_future_indicators() {
        let state = crate::db::test_state();
        let latest = {
            let conn = state.db.lock().unwrap();
            conn.query_row("SELECT MAX(trade_date) FROM quote_daily", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap()
        };
        let app = build_app(state);
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/instruments/600519.SH/fundamentals?as_of=2020-01-01")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["data"]["reports"].as_array().unwrap().len(), 0);
        assert_eq!(value["data"]["indicators"].as_array().unwrap().len(), 0);
        assert_eq!(value["data"]["valuation"]["as_of"], "");

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/instruments/600519.SH/fundamentals")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let report = value["data"]["reports"]
            .as_array()
            .unwrap()
            .iter()
            .find(|report| report["report_period"] == "2026-06-30")
            .unwrap();
        let indicator = value["data"]["indicators"]
            .as_array()
            .unwrap()
            .iter()
            .find(|indicator| indicator["report_period"] == "2026-06-30")
            .unwrap();
        let expected =
            report["net_profit"].as_f64().unwrap() / report["revenue"].as_f64().unwrap() * 100.0;
        assert!((indicator["net_margin"].as_f64().unwrap() - expected).abs() < 1e-9);
        assert_eq!(value["data"]["valuation"]["as_of"], latest);
    }

    #[tokio::test]
    async fn fifty_two_week_range_ignores_ancient_extremes() {
        let state = crate::db::test_state();
        {
            let conn = state.db.lock().unwrap();
            conn.execute(
                "INSERT OR REPLACE INTO quote_daily (symbol, trade_date, open, high, low, close, volume, amount, adj_factor, turnover) VALUES ('600519.SH', '2020-01-02', 100, 99999, 90, 100, 1, 1, 1.0, 0)",
                [],
            )
            .unwrap();
        }
        let app = build_app(state);
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/instruments/600519.SH")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(value["data"]["high_52w"].as_f64().unwrap() < 90000.0);
    }

    #[tokio::test]
    async fn profile_correction_writes_audit_trail() {
        let state = crate::db::test_state();
        let audit_state = state.clone();
        let app = build_app(state);
        let res = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/v1/instruments/600519.SH/profile")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"field":"controller","value":"测试控制人","operator":"tester"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let conn = audit_state.db.lock().unwrap();
        let (old, new, operator): (String, String, String) = conn
            .query_row(
                "SELECT old_value, new_value, operator FROM reference_correction ORDER BY id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(old, "贵州省国资委");
        assert_eq!(new, "测试控制人");
        assert_eq!(operator, "tester");
    }

    #[tokio::test]
    async fn data_source_secret_is_masked() {
        std::env::set_var("ALPHAK_AKSHARE_TOKEN", "supersecret1234");
        let app = build_app(crate::db::test_state());
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/admin/data-sources")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(!text.contains("supersecret1234"));
        assert!(text.contains("***1234"));
        assert!(text.contains("\"secret_configured\":true"));
        std::env::remove_var("ALPHAK_AKSHARE_TOKEN");
    }

    #[tokio::test]
    async fn stale_snapshot_is_served_while_refreshing() {
        let state = crate::db::test_state();
        let app = build_app(state.clone());
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/market/overview")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        {
            let conn = state.db.lock().unwrap();
            conn.execute(
                "UPDATE market_snapshot SET computed_at = '2000-01-01T00:00:00+00:00'",
                [],
            )
            .unwrap();
        }
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/market/overview")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        let conn = state.db.lock().unwrap();
        let computed: String = conn
            .query_row("SELECT computed_at FROM market_snapshot LIMIT 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_ne!(computed, "2000-01-01T00:00:00+00:00");
    }

    #[tokio::test]
    async fn future_members_are_not_returned() {
        let state = crate::db::test_state();
        {
            let conn = state.db.lock().unwrap();
            conn.execute(
                "UPDATE index_member SET in_date = '2099-01-01' WHERE index_code = '000001.SH'",
                [],
            )
            .unwrap();
            conn.execute(
                "UPDATE sector_member SET in_date = '2099-01-01' WHERE sector_id = 1",
                [],
            )
            .unwrap();
        }
        let app = build_app(state);
        for uri in ["/api/v1/indices/000001.SH", "/api/v1/sectors/1"] {
            let res = app
                .clone()
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK);
            let body = axum::body::to_bytes(res.into_body(), usize::MAX)
                .await
                .unwrap();
            let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(
                value["data"]["members"].as_array().unwrap().len(),
                0,
                "uri {uri}"
            );
        }
    }

    #[tokio::test]
    async fn extractor_failures_use_json_envelope() {
        let app = build_app(crate::db::test_state());
        let cases = [
            (
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/watchlists/groups")
                    .header("content-type", "application/json")
                    .body(Body::from("{"))
                    .unwrap(),
                StatusCode::BAD_REQUEST,
            ),
            (
                Request::builder()
                    .uri("/api/v1/sectors/not-a-number")
                    .body(Body::empty())
                    .unwrap(),
                StatusCode::BAD_REQUEST,
            ),
            (
                Request::builder()
                    .uri("/api/v1/market/overview?refresh=wrong")
                    .body(Body::empty())
                    .unwrap(),
                StatusCode::BAD_REQUEST,
            ),
        ];
        for (request, expected) in cases {
            let res = app.clone().oneshot(request).await.unwrap();
            assert_eq!(res.status(), expected);
            println!(
                "extractor content-type={:?}",
                res.headers().get("content-type")
            );
            let is_json = res
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .contains("application/json");
            assert!(is_json);
            let body = axum::body::to_bytes(res.into_body(), usize::MAX)
                .await
                .unwrap();
            let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(value["code"], 1);
            assert!(value["request_id"]
                .as_str()
                .is_some_and(|id| !id.is_empty()));
        }
    }

    #[tokio::test]
    async fn invalid_settings_are_rejected_without_partial_update() {
        let app = build_app(crate::db::test_state());
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/v1/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"default_home":"/nope","poll_interval":-1,"up_color":"invalid"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["data"]["default_home"], "/");
        assert_eq!(value["data"]["up_color"], "red");
        assert_eq!(value["data"]["poll_interval"], 30);
    }

    #[tokio::test]
    async fn a_new_manual_request_can_repair_the_same_day() {
        let state = crate::db::test_state();
        let app = build_app(state.clone());
        for request_id in ["repair-1", "repair-2"] {
            let res = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/v1/admin/data-jobs")
                        .header("content-type", "application/json")
                        .body(Body::from(format!(
                            r#"{{"dataset":"quote_daily","request_id":"{request_id}"}}"#
                        )))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK);
            if request_id == "repair-1" {
                let conn = state.db.lock().unwrap();
                conn.execute(
                    "DELETE FROM quote_daily WHERE symbol = '600519.SH' AND trade_date = '2026-10-21'",
                    [],
                )
                .unwrap();
            }
        }
        let conn = state.db.lock().unwrap();
        let restored: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM quote_daily WHERE symbol = '600519.SH' AND trade_date = '2026-10-21'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(restored, 1);
    }

    #[test]
    fn gap_detection_uses_trading_dates() {
        let bar = crate::sources::DailyBar {
            symbol: "600519.SH".into(),
            date: "2026-10-21".into(),
            open: 1300.0,
            high: 1320.0,
            low: 1290.0,
            close: 1309.69,
            volume: 33514.0,
            amount: 4403823070.0,
            amount_known: true,
            adj_factor: Some(1.0),
        };
        let expected = vec!["2026-10-20".to_string(), "2026-10-21".to_string()];
        assert_eq!(
            missing_dates(&expected, &[bar]),
            vec!["2026-10-20".to_string()]
        );
    }

    #[tokio::test]
    async fn source_status_exposes_policies_and_no_mock_fallback() {
        let app = build_app(crate::db::test_state());
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/admin/sources")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["data"]["mock_fallback"], false);
        assert_eq!(value["data"]["policies"].as_array().unwrap().len(), 3);

        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/admin/sources/sync")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"dataset":"unknown","symbols":["600519.SH"],"start":"2026-10-01","end":"2026-10-02"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn adjusted_bars_degrade_when_factor_is_missing() {
        let state = crate::db::test_state();
        {
            let conn = state.db.lock().unwrap();
            conn.execute(
                "UPDATE quote_daily SET source = 'tencent', adj_source = '', adj_batch_id = '' WHERE symbol = '600519.SH'",
                [],
            )
            .unwrap();
        }
        let app = build_app(state);
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/instruments/600519.SH/bars?period=daily&adjust=qfq")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["data"]["adjust"], "none");
        assert_eq!(value["data"]["adjust_available"], false);
        assert!(value["data"]["notice"].as_str().is_some());
    }

    #[tokio::test]
    async fn overview_stock_and_bars_work() {
        let app = build_app(crate::db::test_state());
        for uri in [
            "/api/v1/market/overview",
            "/api/v1/instruments/600519.SH",
            "/api/v1/instruments/600519.SH/bars?period=daily&adjust=qfq",
        ] {
            let res = app
                .clone()
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK, "uri {uri}");
            let body = axum::body::to_bytes(res.into_body(), usize::MAX)
                .await
                .unwrap();
            let text = String::from_utf8(body.to_vec()).unwrap();
            assert!(text.contains("\"code\":0"), "uri {uri}");
        }
    }
}
