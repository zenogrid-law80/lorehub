//! Explicit, bounded log maintenance. Execution metadata is never deleted.
use anyhow::{Result, ensure};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool};

#[derive(Debug, Default, FromRow, Serialize)]
pub struct LogVolume {
    pub pipelines: i64,
    pub log_rows: i64,
    /// Uncompressed UTF-8 content bytes, not reclaimed PostgreSQL disk space.
    pub content_bytes: i64,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub cutoff: DateTime<Utc>,
    pub batch_size: u32,
    pub applied: bool,
    pub eligible: LogVolume,
    pub deleted: LogVolume,
}

/// A preview is a read-only snapshot. An apply deletes at most one batch.
pub async fn prune_logs(
    pool: &PgPool,
    keep_days: u32,
    batch_size: u32,
    apply: bool,
) -> Result<Report> {
    ensure!(
        (1..=36500).contains(&keep_days),
        "keep-days must be 1..36500"
    );
    ensure!(
        (1..=10000).contains(&batch_size),
        "batch-size must be 1..10000"
    );
    let mut tx = pool.begin().await?;
    if apply {
        // Serialize maintenance processes without blocking coordinator traffic.
        let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(714275036921)")
            .fetch_one(&mut *tx)
            .await?;
        ensure!(
            locked,
            "another log retention batch is running; retry later"
        );
    } else {
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("SET LOCAL statement_timeout = '30s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL lock_timeout = '5s'")
        .execute(&mut *tx)
        .await?;
    // Use database time so coordinator and maintenance host clock skew is irrelevant.
    let cutoff: DateTime<Utc> = sqlx::query_scalar("SELECT now() - make_interval(days => $1)")
        .bind(keep_days as i32)
        .fetch_one(&mut *tx)
        .await?;
    let eligible = sqlx::query_as(
        "SELECT count(DISTINCT p.id) AS pipelines, count(*) AS log_rows, \
         COALESCE(sum(octet_length(l.content)), 0)::bigint AS content_bytes \
         FROM pipelines p JOIN logs l ON l.pipeline_id = p.id \
         WHERE p.status IN ('succeeded', 'failed', 'canceled') AND p.finished_at < $1",
    )
    .bind(cutoff)
    .fetch_one(&mut *tx)
    .await?;
    let deleted = if apply {
        sqlx::query_as(
            "WITH candidates AS ( \
               SELECT l.id FROM pipelines p JOIN logs l ON l.pipeline_id = p.id \
               WHERE p.status IN ('succeeded', 'failed', 'canceled') AND p.finished_at < $1 \
               ORDER BY p.finished_at, p.id, l.id LIMIT $2 FOR UPDATE OF l SKIP LOCKED \
             ), deleted AS ( \
               DELETE FROM logs USING candidates WHERE logs.id = candidates.id \
               RETURNING logs.pipeline_id, octet_length(logs.content) AS bytes \
             ), marked AS ( \
               UPDATE pipelines SET logs_pruned_at = now() \
               WHERE id IN (SELECT pipeline_id FROM deleted) RETURNING id \
             ) \
             SELECT count(DISTINCT pipeline_id) AS pipelines, count(*) AS log_rows, \
                    COALESCE(sum(bytes), 0)::bigint AS content_bytes FROM deleted",
        )
        .bind(cutoff)
        .bind(i64::from(batch_size))
        .fetch_one(&mut *tx)
        .await?
    } else {
        LogVolume::default()
    };
    tx.commit().await?;
    Ok(Report {
        cutoff,
        batch_size,
        applied: apply,
        eligible,
        deleted,
    })
}
