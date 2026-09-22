//! Permission-scoped workspace home, independent of history pagination.
use axum::{Extension, Json, extract::State};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

use super::{
    api::{ApiError, AppState},
    auth::AuthSession,
    pipeline_access::PipelineAccess,
};

#[derive(Serialize, FromRow)]
struct Summary {
    repositories: i64,
    running: i64,
    queued: i64,
    online_runners: i64,
    failed: i64,
    waiting: i64,
    link_errors: i64,
}

#[derive(Serialize, FromRow)]
struct RunAlert {
    id: Uuid,
    repository_url: String,
    pipeline_name: Option<String>,
    branch: Option<String>,
    status: String,
}

#[derive(Serialize, FromRow)]
struct LinkAlert {
    repository_url: String,
    branch: String,
    path: String,
}

#[derive(Serialize)]
pub(super) struct Snapshot {
    observed_at: DateTime<Utc>,
    summary: Summary,
    failed: Vec<RunAlert>,
    waiting: Vec<RunAlert>,
    links: Vec<LinkAlert>,
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
    sqlx::query("SET LOCAL statement_timeout = '5s'")
        .execute(&mut *tx)
        .await?;
    let observed_at = sqlx::query_scalar("SELECT now()")
        .fetch_one(&mut *tx)
        .await?;
    // Reuse the execution access boundary, including recreated repositories.
    // Link errors only include still-present dependencies; never return raw errors.
    let scope = |statement: &str| {
        PipelineAccess::sql(&format!(
            ", failed_runs AS (SELECT * FROM accessible_pipelines WHERE status='failed' AND COALESCE(finished_at,created_at)>=now()-interval '24 hours'), \
         waiting_runs AS (SELECT * FROM accessible_pipelines WHERE status='queued' AND NOT cancel_requested AND created_at<now()-interval '5 minutes'), \
         failed_links AS (SELECT r.repository_url, p.root_branch AS branch, p.link_path AS path \
           FROM repository_link_policies p JOIN repository_link_dependencies d USING(root_resource_id,root_branch,link_path) \
           JOIN accessible_repositories r ON r.resource_id=p.root_resource_id \
           WHERE r.repository_url IS NOT NULL AND p.last_error IS NOT NULL) {statement}"
        ))
    };
    let summary = access.query_as(&scope(
        "SELECT (SELECT count(*) FROM accessible_repositories WHERE repository_url IS NOT NULL) AS repositories, \
         count(*) FILTER(WHERE status='running') AS running, count(*) FILTER(WHERE status='queued') AS queued, \
         (SELECT count(*) FROM runners WHERE stopped_at IS NULL AND last_seen>=now()-interval '15 seconds') AS online_runners, \
         (SELECT count(*) FROM failed_runs) AS failed, (SELECT count(*) FROM waiting_runs) AS waiting, \
         (SELECT count(*) FROM failed_links) AS link_errors FROM accessible_pipelines"
    )).fetch_one(&mut *tx).await?;
    let failed = access.query_as(&scope(
        "SELECT id,repository_url,pipeline_name,branch,status FROM failed_runs ORDER BY COALESCE(finished_at,created_at) DESC,id DESC LIMIT 5"
    )).fetch_all(&mut *tx).await?;
    let waiting = access.query_as(&scope(
        "SELECT id,repository_url,pipeline_name,branch,status FROM waiting_runs ORDER BY created_at,id LIMIT 5"
    )).fetch_all(&mut *tx).await?;
    let links = access
        .query_as(&scope(
            "SELECT * FROM failed_links ORDER BY repository_url,branch,path LIMIT 5",
        ))
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(Snapshot {
        observed_at,
        summary,
        failed,
        waiting,
        links,
    }))
}
