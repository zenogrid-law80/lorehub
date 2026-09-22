//! Administrator diagnostics. Execution rows retain the normal repository scope.
use axum::{Extension, Json, extract::State};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use super::{
    api::{ApiError, AppState},
    auth::AuthSession,
    pipeline_access::PipelineAccess,
};

/// Store only fixed error categories, never Lore output, tokens or command arguments.
pub(super) async fn record_watch(pool: &PgPool, resource: &str, error: Option<&str>) {
    let result = sqlx::query(
        "INSERT INTO ci_repository_watch_health(resource_id,last_success_at,error_code,consecutive_failures) \
         SELECT resource_id, CASE WHEN $2::text IS NULL THEN now() END, $2, CASE WHEN $2::text IS NULL THEN 0 ELSE 1 END \
         FROM lore_resources WHERE resource_id=$1 \
         ON CONFLICT(resource_id) DO UPDATE SET checked_at=now(), \
         last_success_at=CASE WHEN EXCLUDED.error_code IS NULL THEN now() ELSE ci_repository_watch_health.last_success_at END, \
         error_code=EXCLUDED.error_code, \
         consecutive_failures=CASE WHEN EXCLUDED.error_code IS NULL THEN 0 ELSE ci_repository_watch_health.consecutive_failures+1 END",
    ).bind(resource).bind(error).execute(pool).await;
    if let Err(error) = result {
        // Diagnostics must not stop trigger reconciliation.
        tracing::warn!(resource_id=resource, %error, "cannot record repository watch health");
    }
}

#[derive(Serialize, FromRow)]
struct QueueSummary {
    queued: i64,
    running: i64,
    expired_leases: i64,
    oldest_wait_seconds: Option<f64>,
}

#[derive(Serialize, FromRow)]
struct WaitingPipeline {
    id: Uuid,
    repository_url: String,
    pipeline_name: Option<String>,
    runner_os: Option<String>,
    wait_seconds: f64,
    reason: String,
}

#[derive(Serialize, FromRow)]
struct RunnerCapacity {
    os: String,
    online: i64,
    idle: i64,
    draining: i64,
    offline: i64,
    stopped: i64,
}

#[derive(Serialize, FromRow)]
struct WatchHealth {
    resource_id: String,
    name: String,
    checked_at: Option<DateTime<Utc>>,
    last_success_at: Option<DateTime<Utc>>,
    error_code: Option<String>,
    consecutive_failures: i64,
    state: String,
    link_errors: i64,
}

#[derive(Serialize, FromRow)]
struct Storage {
    database_bytes: i64,
    execution_bytes: i64,
    log_bytes: i64,
}

#[derive(Serialize)]
pub(super) struct Snapshot {
    observed_at: DateTime<Utc>,
    queue: QueueSummary,
    waiting: Vec<WaitingPipeline>,
    runners: Vec<RunnerCapacity>,
    repositories: Vec<WatchHealth>,
    repository_count: i64,
    storage: Storage,
}

pub(super) async fn overview(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
) -> Result<Json<Snapshot>, ApiError> {
    let access = PipelineAccess::new(&state.repositories, session.user.id);
    let mut tx = state.pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    let observed_at = sqlx::query_scalar("SELECT now()")
        .fetch_one(&mut *tx)
        .await?;
    let queue = access.query_as(&PipelineAccess::sql(
        "SELECT count(*) FILTER(WHERE status='queued') AS queued, \
         count(*) FILTER(WHERE status='running') AS running, \
         count(*) FILTER(WHERE status='running' AND lease_until < now()) AS expired_leases, \
         EXTRACT(EPOCH FROM now()-min(created_at) FILTER(WHERE status='queued'))::float8 AS oldest_wait_seconds \
         FROM accessible_pipelines WHERE status IN ('queued','running')",
    )).fetch_one(&mut *tx).await?;
    let waiting = access.query_as(&PipelineAccess::sql(
        "SELECT p.id, p.repository_url, p.pipeline_name, p.runner_os, \
         GREATEST(0, EXTRACT(EPOCH FROM now()-p.created_at))::float8 AS wait_seconds, \
         CASE WHEN p.cancel_requested THEN 'canceling' \
         WHEN EXISTS(SELECT 1 FROM unnest(p.pipeline_needs) dependency(name) JOIN LATERAL ( \
             SELECT upstream.status FROM accessible_pipelines upstream \
             WHERE upstream.repository_url=p.repository_url AND upstream.branch IS NOT DISTINCT FROM p.branch \
               AND upstream.revision=p.revision AND upstream.pipeline_name=dependency.name \
             ORDER BY upstream.created_at DESC,upstream.id DESC LIMIT 1 \
         ) latest ON true WHERE latest.status <> 'succeeded') THEN 'dependencies' \
         WHEN NOT EXISTS(SELECT 1 FROM runners r WHERE r.stopped_at IS NULL AND r.last_seen >= now()-interval '15 seconds' \
             AND (p.runner_os IS NULL OR p.runner_os=r.os)) THEN 'no_runner' \
         WHEN NOT EXISTS(SELECT 1 FROM runners r WHERE r.stopped_at IS NULL AND r.last_seen >= now()-interval '15 seconds' \
             AND NOT r.draining AND (p.runner_os IS NULL OR p.runner_os=r.os)) THEN 'draining' \
         WHEN NOT EXISTS(SELECT 1 FROM runners r WHERE r.stopped_at IS NULL AND r.last_seen >= now()-interval '15 seconds' \
             AND NOT r.draining AND (p.runner_os IS NULL OR p.runner_os=r.os) \
             AND NOT EXISTS(SELECT 1 FROM pipelines active WHERE active.status='running' AND active.worker_id=r.id)) THEN 'busy' \
         ELSE 'ready' END AS reason \
         FROM accessible_pipelines p WHERE p.status='queued' ORDER BY p.created_at,p.id LIMIT 100",
    )).fetch_all(&mut *tx).await?;
    let runners = sqlx::query_as(
        "SELECT r.os, count(*) FILTER(WHERE stopped_at IS NULL AND last_seen>=now()-interval '15 seconds') AS online, \
         count(*) FILTER(WHERE stopped_at IS NULL AND last_seen>=now()-interval '15 seconds' AND NOT draining \
            AND NOT EXISTS(SELECT 1 FROM pipelines p WHERE p.status='running' AND p.worker_id=r.id)) AS idle, \
         count(*) FILTER(WHERE draining) AS draining, \
         count(*) FILTER(WHERE stopped_at IS NULL AND last_seen<now()-interval '15 seconds') AS offline, \
         count(*) FILTER(WHERE stopped_at IS NOT NULL) AS stopped FROM runners r GROUP BY r.os ORDER BY r.os",
    ).fetch_all(&mut *tx).await?;
    let repository_count =
        sqlx::query_scalar("SELECT count(*) FROM lore_resources WHERE owner_subject IS NOT NULL")
            .fetch_one(&mut *tx)
            .await?;
    let repositories = sqlx::query_as(
        "SELECT r.resource_id, r.name, h.checked_at, h.last_success_at, h.error_code, \
         COALESCE(h.consecutive_failures,0) AS consecutive_failures, \
         CASE WHEN h.resource_id IS NULL THEN 'unknown' WHEN h.checked_at < now()-interval '2 minutes' THEN 'stale' \
              WHEN h.error_code IS NOT NULL THEN 'error' ELSE 'ok' END AS state, \
         (SELECT count(*) FROM repository_link_policies policy \
          JOIN repository_link_dependencies dependency USING(root_resource_id,root_branch,link_path) \
          WHERE policy.root_resource_id=r.resource_id AND policy.last_error IS NOT NULL) AS link_errors \
         FROM lore_resources r LEFT JOIN ci_repository_watch_health h USING(resource_id) WHERE r.owner_subject IS NOT NULL \
         ORDER BY (h.error_code IS NOT NULL) DESC, \
           (h.resource_id IS NULL OR h.checked_at < now()-interval '2 minutes') DESC, link_errors DESC, r.name,r.resource_id LIMIT 100",
    ).fetch_all(&mut *tx).await?;
    let storage = sqlx::query_as(
        "SELECT pg_database_size(current_database()) AS database_bytes, \
         pg_total_relation_size('pipelines')+pg_total_relation_size('jobs') AS execution_bytes, \
         pg_total_relation_size('logs') AS log_bytes",
    )
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(Snapshot {
        observed_at,
        queue,
        waiting,
        runners,
        repositories,
        repository_count,
        storage,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test]
    #[ignore = "requires disposable PostgreSQL"]
    async fn watch_observations_preserve_success_until_recovery_and_cascade_on_delete(
        pool: PgPool,
    ) {
        sqlx::query("INSERT INTO lore_resources(resource_id,name) VALUES('watch','watch')")
            .execute(&pool)
            .await
            .unwrap();
        record_watch(&pool, "watch", None).await;
        let success: DateTime<Utc> = sqlx::query_scalar(
            "SELECT last_success_at FROM ci_repository_watch_health WHERE resource_id='watch'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        for _ in 0..2 {
            record_watch(&pool, "watch", Some("watch_failed")).await;
        }
        let (last_success, failures, error): (DateTime<Utc>, i64, String) = sqlx::query_as("SELECT last_success_at,consecutive_failures,error_code FROM ci_repository_watch_health WHERE resource_id='watch'").fetch_one(&pool).await.unwrap();
        assert_eq!(last_success, success);
        assert_eq!(failures, 2);
        assert_eq!(error, "watch_failed");
        record_watch(&pool, "watch", None).await;
        let (failures, error): (i64, Option<String>) = sqlx::query_as("SELECT consecutive_failures,error_code FROM ci_repository_watch_health WHERE resource_id='watch'").fetch_one(&pool).await.unwrap();
        assert_eq!(failures, 0);
        assert!(error.is_none());
        sqlx::query("DELETE FROM lore_resources WHERE resource_id='watch'")
            .execute(&pool)
            .await
            .unwrap();
        record_watch(&pool, "watch", Some("watch_failed")).await;
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM ci_repository_watch_health")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[cfg(unix)]
    #[sqlx::test]
    #[ignore = "requires disposable PostgreSQL"]
    async fn watcher_records_backend_and_command_failures(pool: PgPool) {
        use super::super::{repositories::RepositoryService, tokens::TokenIssuer, triggers};
        use tokio_util::sync::CancellationToken;
        let owner = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO users(id,google_sub,email) VALUES($1,'watch-test','watch@zenogrid.co.kr')",
        )
        .bind(owner)
        .execute(&pool)
        .await
        .unwrap();
        for (id, backend) in [("watch-main", "dynamodb_s3"), ("watch-local", "local_file")] {
            sqlx::query("INSERT INTO lore_resources(resource_id,name,owner_subject,storage_backend) VALUES($1,$1,$2,$3)").bind(id).bind(owner.to_string()).bind(backend).execute(&pool).await.unwrap();
        }
        let tokens = TokenIssuer::from_files(
            "tests/fixtures/test-private.pem",
            "tests/fixtures/test-jwks.json",
            "http://127.0.0.1:8080",
            "zenogrid.co.kr",
        )
        .unwrap();
        let stop = CancellationToken::new();
        let task = tokio::spawn(triggers::run(
            pool.clone(),
            "/usr/bin/false".into(),
            RepositoryService::new("/usr/bin/false", "lores://test", "lores://test").unwrap(),
            tokens,
            stop.clone(),
        ));
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let observations: Vec<(String,String)> = sqlx::query_as("SELECT resource_id,error_code FROM ci_repository_watch_health ORDER BY resource_id").fetch_all(&pool).await.unwrap();
                if observations.len() == 2 { break observations; }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        }).await;
        stop.cancel();
        task.await.unwrap();
        assert_eq!(
            result.unwrap(),
            vec![
                ("watch-local".into(), "backend_unavailable".into()),
                ("watch-main".into(), "watch_failed".into())
            ]
        );
    }
}
