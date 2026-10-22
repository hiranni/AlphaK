use std::time::Duration;

use crate::db::AppState;

use super::store::{self, RetryTask};

pub const MAX_RETRIES: i64 = 5;

fn claim_retry(conn: &rusqlite::Connection, task: &RetryTask) -> bool {
    conn.execute(
        "UPDATE source_retry_task SET attempts = attempts + 1 WHERE dataset = ?1 AND symbol = ?2 AND start_date = ?3 AND end_date = ?4 AND attempts < ?5",
        rusqlite::params![task.dataset, task.symbol, task.start, task.end, MAX_RETRIES],
    ).unwrap_or(0) == 1
}

fn pending_datasets(state: &AppState) -> Vec<String> {
    let Ok(conn) = state.db.lock() else {
        return Vec::new();
    };
    let Ok(mut stmt) =
        conn.prepare("SELECT DISTINCT dataset FROM source_retry_task ORDER BY dataset")
    else {
        return Vec::new();
    };
    stmt.query_map([], |r| r.get::<_, String>(0))
        .map(|rows| rows.flatten().collect::<Vec<_>>())
        .unwrap_or_default()
}

fn tasks_for(state: &AppState, dataset: &str) -> Vec<RetryTask> {
    let Ok(conn) = state.db.lock() else {
        return Vec::new();
    };
    store::pending_retries(&conn, Some(dataset)).unwrap_or_default()
}

async fn retry_task(state: &AppState, task: &RetryTask) -> Result<(), String> {
    let batch_id = format!(
        "auto-{}-{}-{}",
        task.dataset,
        task.symbol,
        uuid::Uuid::new_v4()
    );
    match task.dataset.as_str() {
        "quote_daily" | "index_quote" => crate::routes::sync_daily_symbol(
            state,
            &task.dataset,
            &task.symbol,
            &task.start,
            &task.end,
            &batch_id,
            task.dataset == "quote_daily",
        )
        .await
        .and_then(|value| {
            if value["gap_count"].as_u64().unwrap_or(0) > 0 {
                Err("trading days still missing".to_string())
            } else {
                Ok(())
            }
        }),
        "snapshot" => {
            let batch = state
                .sources
                .fetch_snapshot(&task.symbol)
                .await
                .map_err(|failure| failure.message())?;
            let conn = state.db.lock().map_err(|_| "database lock".to_string())?;
            store::publish_snapshot(&conn, &batch, &batch_id).map_err(|error| error.message)?;
            store::clear_retry(&conn, "snapshot", &task.symbol, "", "")
                .map_err(|error| error.message)?;
            Ok(())
        }
        other => Err(format!("unsupported retry dataset: {other}")),
    }
}

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(90)).await;
        loop {
            let datasets = pending_datasets(&state);
            for dataset in datasets {
                for task in tasks_for(&state, &dataset) {
                    if !state
                        .db
                        .lock()
                        .map(|conn| claim_retry(&conn, &task))
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    match retry_task(&state, &task).await {
                        Ok(()) => {
                            if let Ok(conn) = state.db.lock() {
                                let _ = store::clear_retry(
                                    &conn,
                                    &task.dataset,
                                    &task.symbol,
                                    &task.start,
                                    &task.end,
                                );
                            }
                        }
                        Err(error) => {
                            if let Ok(conn) = state.db.lock() {
                                let _ = store::queue_retry(
                                    &conn,
                                    &task.dataset,
                                    &task.symbol,
                                    &task.start,
                                    &task.end,
                                    &error,
                                );
                            }
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                if let Ok(conn) = state.db.lock() {
                    let pending = store::has_pending(&conn, &dataset).unwrap_or(true);
                    if pending {
                        let _ = store::mark_partial(&conn, &dataset, "retry pending");
                    } else {
                        let _ = store::mark_fresh(&conn, &dataset, "auto", "auto");
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_budget_counts_attempts_and_stops_at_five() {
        let conn = crate::db::init_db(":memory:").unwrap();
        store::queue_retry(
            &conn,
            "quote_daily",
            "600519.SH",
            "2026-10-01",
            "2026-10-22",
            "timeout",
        )
        .unwrap();
        let task = store::pending_retries(&conn, None).unwrap().remove(0);
        assert_eq!(task.attempts, 0);
        for expected in 1..=MAX_RETRIES {
            assert!(claim_retry(&conn, &task));
            store::queue_retry(
                &conn,
                &task.dataset,
                &task.symbol,
                &task.start,
                &task.end,
                "still missing",
            )
            .unwrap();
            assert_eq!(
                store::pending_retries(&conn, None).unwrap()[0].attempts,
                expected
            );
        }
        assert!(!claim_retry(&conn, &task));
        assert_eq!(
            store::pending_retries(&conn, None).unwrap()[0].attempts,
            MAX_RETRIES
        );
    }
}
