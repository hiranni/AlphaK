//! Offline, file-backed storage workload. Never touches the user's DB or upstream.
use crate::{
    db::AppState,
    persistence as p,
    sources::{Snapshot, SourceBatch, SourceId},
};
use serde_json::{json, Value};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

fn percentile(v: &[u64], q: f64) -> u64 {
    if v.is_empty() {
        return 0;
    }
    let mut s = v.to_vec();
    s.sort_unstable();
    s[((s.len() - 1) as f64 * q).ceil() as usize]
}
fn stats(v: &[u64]) -> Value {
    json!({"count":v.len(),"p50_us":percentile(v,0.5),"p95_us":percentile(v,0.95),"p99_us":percentile(v,0.99)})
}
fn size(path: &std::path::Path, suffix: &str) -> u64 {
    std::fs::metadata(format!("{}{suffix}", path.display()))
        .map(|m| m.len())
        .unwrap_or(0)
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let warmup: u64 = std::env::var("ALPHAK_BENCH_WARMUP")
        .unwrap_or_else(|_| "120".into())
        .parse()?;
    let sample: u64 = std::env::var("ALPHAK_BENCH_SAMPLE")
        .unwrap_or_else(|_| "900".into())
        .parse()?;
    let mut reports = Vec::new();
    for count in [37, 120] {
        for optimized in [false, true] {
            let root = std::env::temp_dir().join(format!("alphak-bench-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&root)?;
            let path = root.join("workload.db");
            let conn = crate::db::init_db(path.to_str().unwrap())?;
            // Same source/seed/hardware; only storage path differs.
            let state = AppState {
                db: Arc::new(Mutex::new(conn)),
                sources: Arc::new(crate::sources::SourceHub::new()),
            };
            let reader_state = state.clone();
            let read_samples = Arc::new(Mutex::new(Vec::<(Instant, u64)>::new()));
            let read_out = read_samples.clone();
            let started = Instant::now();
            let measurement = started + Duration::from_secs(warmup);
            let read_job = tokio::spawn(async move {
                loop {
                    let timer = Instant::now();
                    if timer.duration_since(started) >= Duration::from_secs(warmup + sample) {
                        break;
                    }
                    let result=p::run_db(reader_state.clone(),|c|{let _:i64=c.query_row("SELECT COUNT(*) FROM quote_daily WHERE trade_date=(SELECT MAX(trade_date) FROM quote_daily)",[],|r|r.get(0))?;Ok(())}).await;
                    if result.is_ok() {
                        read_out
                            .lock()
                            .unwrap()
                            .push((timer, timer.elapsed().as_micros() as u64));
                    }
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            });
            let mut commits = Vec::new();
            let mut lock_wait = Vec::new();
            let mut busy = 0;
            let mut cycle = 0;
            let mut before = Value::Null;
            while started.elapsed() < Duration::from_secs(warmup + sample) {
                let cycle_start = Instant::now();
                if before.is_null() && cycle_start >= measurement {
                    before = json!({"db_bytes":size(&path,""),"wal_bytes":size(&path,"-wal")});
                }
                let snapshots: Vec<_> = (0..count)
                    .map(|n| Snapshot {
                        symbol: format!("{n:06}.SZ"),
                        name: "fixture".into(),
                        date: "2026-10-23".into(),
                        open: 10.0,
                        high: 12.0,
                        low: 9.0,
                        close: 11.0 + (cycle % 2) as f64 * 0.01,
                        prev_close: 10.0,
                        volume: 100.0,
                        amount: 1100.0,
                        change_pct: 10.0,
                    })
                    .collect();
                for chunk in snapshots.chunks(20) {
                    let values = chunk.to_vec();
                    let measured = cycle_start >= measurement;
                    let result = p::run_db(state.clone(), move |conn| {
                        let mut times = Vec::new();
                        if optimized {
                            let t = Instant::now();
                            p::publish_realtime(conn, &values, &crate::db::now_iso())?;
                            times.push(t.elapsed().as_micros() as u64)
                        } else {
                            for snapshot in values {
                                let t = Instant::now();
                                crate::sources::store::publish_snapshot(
                                    conn,
                                    &SourceBatch {
                                        dataset: "snapshot".into(),
                                        source: SourceId::AxData,
                                        attempts: 1,
                                        data: snapshot,
                                        adj_source: None,
                                        adj_error: None,
                                    },
                                    "baseline",
                                )?;
                                times.push(t.elapsed().as_micros() as u64)
                            }
                        }
                        Ok(times)
                    })
                    .await;
                    match result {
                        Ok(times) => {
                            if measured {
                                commits.extend(times)
                            }
                        }
                        Err(e) => {
                            busy += 1;
                            tracing::warn!(error=%e.message,"benchmark database error")
                        }
                    }
                }
                // Active 360-bar chart: identical unchanged upstream payload each round.
                let bars:Vec<crate::models::Bar>=(0..360).map(|n|serde_json::from_value(json!({"date":(1_700_000_000+n*3).to_string(),"open":10.0,"high":12.0,"low":9.0,"close":11.0,"volume":100.0,"amount":1100.0})).unwrap()).collect();
                let waited = Instant::now();
                let history = {
                    let conn = state.db.lock().unwrap();
                    if cycle_start >= measurement {
                        lock_wait.push(waited.elapsed().as_micros() as u64)
                    }
                    if optimized {
                        p::store_bars(
                            &conn,
                            "000000.SZ",
                            "second_3",
                            "fixture",
                            &bars,
                            chrono::Utc::now().timestamp(),
                        )
                        .map_err(|e| std::io::Error::other(e.message))?;
                    } else {
                        conn.execute("INSERT INTO live_cache(symbol,dataset,payload) VALUES('000000.SZ','second_3',?1) ON CONFLICT(symbol,dataset) DO UPDATE SET payload=excluded.payload",[serde_json::to_string(&bars)?])?;
                    }
                    // Historical loading is finished while locked; computation below holds no connection.
                    let history: Vec<f64> = if cycle % 20 == 0 {
                        conn.prepare("SELECT close FROM quote_daily ORDER BY trade_date")?
                            .query_map([], |r| r.get(0))?
                            .collect::<rusqlite::Result<_>>()?
                    } else {
                        Vec::new()
                    };
                    history
                };
                let _sum: f64 = history.iter().sum();
                cycle += 1;
                tokio::time::sleep(Duration::from_secs(3).saturating_sub(cycle_start.elapsed()))
                    .await;
            }
            read_job.await?;
            let reads: Vec<_> = read_samples
                .lock()
                .unwrap()
                .iter()
                .filter(|(t, _)| *t >= measurement)
                .map(|(_, v)| *v)
                .collect();
            let conn = state.db.lock().unwrap();
            let diag = p::diagnostics(&conn).map_err(|e| std::io::Error::other(e.message))?;
            let report = json!({"symbols":count,"mode":if optimized{"optimized"}else{"baseline"},"warmup_seconds":warmup,"sample_seconds":sample,"cycles":cycle,"commit":stats(&commits),"read":stats(&reads),"lock_wait":stats(&lock_wait),"db_busy":busy,"before":before,"after":{"db_bytes":size(&path,""),"wal_bytes":size(&path,"-wal")},"pragmas":diag["pragmas"],"sqlite_version":rusqlite::version(),"acceptance_eligible":warmup>=120&&sample>=900,"latency_targets_met":percentile(&reads,0.95)<=200_000&&percentile(&commits,0.95)<=50_000&&percentile(&lock_wait,0.95)<=50_000&&busy==0});
            println!("{}", report);
            reports.push(report);
            drop(conn);
            drop(state);
            std::fs::remove_dir_all(root)?;
        }
    }
    let out =
        std::env::var("ALPHAK_BENCH_REPORT").unwrap_or_else(|_| "storage-benchmark.json".into());
    let hardware = std::process::Command::new("uname").arg("-a").output()?;
    std::fs::write(
        out,
        serde_json::to_string_pretty(
            &json!({"hardware":String::from_utf8_lossy(&hardware.stdout).trim(),"logical_cpus":std::thread::available_parallelism()?.get(),"started_utc":chrono::Utc::now().to_rfc3339(),"upstream":"offline deterministic fixture; no network latency included","limitations":["smoke runs below120/900 seconds are not D7 acceptance","process cumulative I/O and CPU/RSS require external profiler","lock samples cover chart entry; full lock histograms require instrumentation"],"runs":reports}),
        )?,
    )?;
    Ok(())
}
