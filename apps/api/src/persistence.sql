CREATE TABLE IF NOT EXISTS lifecycle_policy (
    dataset TEXT PRIMARY KEY, policy_version INTEGER NOT NULL,
    config TEXT NOT NULL, updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS input_manifest (
    hash TEXT PRIMARY KEY, relative_path TEXT NOT NULL, size_bytes INTEGER NOT NULL,
    source TEXT NOT NULL, available_at TEXT, created_at INTEGER NOT NULL,
    pinned INTEGER NOT NULL DEFAULT 0 CHECK(pinned IN (0,1))
);
CREATE TABLE IF NOT EXISTS data_reference (
    owner TEXT NOT NULL, input_hash TEXT NOT NULL REFERENCES input_manifest(hash),
    created_at INTEGER NOT NULL, PRIMARY KEY(owner,input_hash)
);
CREATE TABLE IF NOT EXISTS bar_cache (
    symbol TEXT NOT NULL, period TEXT NOT NULL, bucket_start INTEGER NOT NULL,
    source TEXT NOT NULL, payload TEXT NOT NULL, observation_hash TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1, observed_at INTEGER NOT NULL,
    last_accessed_at INTEGER NOT NULL,
    PRIMARY KEY(symbol,period,bucket_start,source)
);
CREATE INDEX IF NOT EXISTS idx_bar_idle ON bar_cache(last_accessed_at);
CREATE TABLE IF NOT EXISTS market_breadth_observation (
    id INTEGER PRIMARY KEY, trade_date TEXT NOT NULL, source TEXT NOT NULL,
    scope TEXT NOT NULL, provider_time TEXT, observed_at INTEGER NOT NULL,
    available_at INTEGER NOT NULL, phase TEXT NOT NULL,
    payload TEXT NOT NULL, observation_hash TEXT NOT NULL,
    UNIQUE(trade_date,source,scope,provider_time,observation_hash)
);
CREATE TABLE IF NOT EXISTS subscription_lease (
    client_id TEXT NOT NULL, symbol TEXT NOT NULL, dataset TEXT NOT NULL,
    expires_at INTEGER NOT NULL, PRIMARY KEY(client_id,symbol,dataset)
);
CREATE INDEX IF NOT EXISTS idx_lease_expiry ON subscription_lease(expires_at);
CREATE TABLE IF NOT EXISTS cleanup_plan (
    id TEXT PRIMARY KEY, policy_version INTEGER NOT NULL, created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL, candidates TEXT NOT NULL, status TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS cleanup_audit (
    id INTEGER PRIMARY KEY, plan_id TEXT NOT NULL REFERENCES cleanup_plan(id),
    table_name TEXT NOT NULL, record_key TEXT NOT NULL,
    deleted_rows INTEGER NOT NULL, created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS cache_tombstone (
    id TEXT PRIMARY KEY, period TEXT NOT NULL, hash TEXT NOT NULL,
    size_bytes INTEGER NOT NULL, deleted_at INTEGER NOT NULL,
    reason TEXT NOT NULL, actor TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS job_idempotency (
    key TEXT PRIMARY KEY, job_id TEXT NOT NULL,
    result TEXT NOT NULL, expires_at INTEGER NOT NULL
);
