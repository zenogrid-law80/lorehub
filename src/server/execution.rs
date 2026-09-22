//! On-demand, read-only run analysis. No inferred historical dependency bindings.
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use super::api::{ApiError, AppState};
use super::{auth::AuthSession, pipeline_access::PipelineAccess};
use crate::ci::db::{Job, Pipeline};

#[derive(Serialize)]
pub(super) struct RunRecord {
    pipeline: Pipeline,
    jobs: Vec<Job>,
}

#[derive(Serialize, FromRow)]
pub(super) struct RelatedRun {
    name: String,
    run_id: Option<Uuid>,
    status: Option<String>,
    created_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub(super) struct Insights {
    observed_at: DateTime<Utc>,
    current: RunRecord,
    upstream: Vec<RelatedRun>,
    downstream: Vec<RelatedRun>,
    upstream_truncated: bool,
    downstream_truncated: bool,
    previous: Option<RunRecord>,
    comparison_warnings: Vec<&'static str>,
    comparable: bool,
}

pub(super) async fn insights(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(id): Path<Uuid>,
) -> Result<Json<Insights>, ApiError> {
    let access = PipelineAccess::new(&state.repositories, session.user.id);
    read_insights(&state.pool, id, &access).await.map(Json)
}

async fn read_insights(
    pool: &PgPool,
    id: Uuid,
    access: &PipelineAccess,
) -> Result<Insights, ApiError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let observed_at = sqlx::query_scalar("SELECT transaction_timestamp()")
        .fetch_one(&mut *tx)
        .await?;
    let pipeline = access.pipeline(&mut *tx, id).await?;
    let jobs = sqlx::query_as("SELECT * FROM jobs WHERE pipeline_id = $1 ORDER BY position")
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;
    // Keep branch identity as stored, just as claim() does (including legacy IDs).
    // Do not fall back to another branch/revision when a prerequisite is absent.
    let upstream_truncated = pipeline.pipeline_needs.len() > 100;
    let names: Vec<_> = pipeline.pipeline_needs.iter().take(100).cloned().collect();
    let upstream = access.query_as(&PipelineAccess::sql(
        "SELECT dependency.name, latest.id AS run_id, latest.status, latest.created_at FROM unnest($4::text[]) WITH ORDINALITY AS dependency(name, pos) LEFT JOIN LATERAL (SELECT id,status,created_at FROM accessible_pipelines WHERE repository_url = $5 AND branch IS NOT DISTINCT FROM $6 AND revision = $7 AND pipeline_name = dependency.name ORDER BY created_at DESC,id DESC LIMIT 1) latest ON true ORDER BY dependency.pos"
    )).bind(&names).bind(&pipeline.repository_url).bind(&pipeline.branch).bind(&pipeline.revision)
        .fetch_all(&mut *tx).await?;
    let mut downstream: Vec<RelatedRun> = access.query_as(&PipelineAccess::sql(
        "SELECT pipeline_name AS name,id AS run_id,status,created_at FROM (SELECT DISTINCT ON (pipeline_name) * FROM accessible_pipelines WHERE repository_url = $4 AND branch IS NOT DISTINCT FROM $5 AND revision = $6 AND pipeline_name IS NOT NULL ORDER BY pipeline_name,created_at DESC,id DESC) latest WHERE $7 = ANY(pipeline_needs) AND id <> $8 ORDER BY pipeline_name LIMIT 101"
    )).bind(&pipeline.repository_url).bind(&pipeline.branch).bind(&pipeline.revision).bind(&pipeline.pipeline_name).bind(id)
        .fetch_all(&mut *tx).await?;
    let downstream_truncated = downstream.len() > 100;
    downstream.truncate(100);
    // The baseline must already have completed when this run was submitted.
    // A later retry/completion must not silently change an old run's comparison.
    let previous_pipeline: Option<Pipeline> = access.query_as(&PipelineAccess::sql(
        "SELECT * FROM accessible_pipelines WHERE repository_url = $4 AND branch IS NOT DISTINCT FROM $5 AND pipeline_name IS NOT DISTINCT FROM $6 AND (created_at,id) < ($7,$8) AND finished_at <= $7 AND status IN ('succeeded','failed','canceled') ORDER BY created_at DESC,id DESC LIMIT 1"
    )).bind(&pipeline.repository_url).bind(&pipeline.branch).bind(&pipeline.pipeline_name).bind(pipeline.created_at).bind(id)
        .fetch_optional(&mut *tx).await?;
    let previous = if let Some(previous_pipeline) = previous_pipeline {
        let previous_jobs =
            sqlx::query_as("SELECT * FROM jobs WHERE pipeline_id = $1 ORDER BY position")
                .bind(previous_pipeline.id)
                .fetch_all(&mut *tx)
                .await?;
        Some(RunRecord {
            pipeline: previous_pipeline,
            jobs: previous_jobs,
        })
    } else {
        None
    };
    tx.commit().await?;
    let current = RunRecord { pipeline, jobs };
    let comparison_warnings = previous
        .as_ref()
        .map(|previous| comparison_warnings(&current, previous))
        .unwrap_or_default();
    let comparable = previous.is_some()
        && comparison_warnings
            .iter()
            .all(|reason| ["revision", "runner"].contains(reason));
    Ok(Insights {
        observed_at,
        current,
        upstream,
        downstream,
        upstream_truncated,
        downstream_truncated,
        previous,
        comparison_warnings,
        comparable,
    })
}

fn comparison_warnings(current: &RunRecord, previous: &RunRecord) -> Vec<&'static str> {
    let a = &current.pipeline;
    let b = &previous.pipeline;
    let mut warnings = Vec::new();
    if a.runner_os != b.runner_os {
        warnings.push("runner_os");
    }
    if a.working_directory != b.working_directory {
        warnings.push("working_directory");
    }
    if a.sparse_view_name != b.sparse_view_name || a.sparse_view_rules != b.sparse_view_rules {
        warnings.push("sparse_view");
    }
    let parse = |value: &Option<String>| {
        value
            .as_deref()
            .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
    };
    match (parse(&a.graph_definition), parse(&b.graph_definition)) {
        (Some(a), Some(b)) if a != b => warnings.push("layout"),
        (Some(_), Some(_)) => {}
        _ => warnings.push("snapshot_missing"),
    }
    if !current.jobs.is_empty() && !previous.jobs.is_empty() {
        let job_keys = |jobs: &[Job]| {
            let mut keys: Vec<_> = jobs
                .iter()
                .map(|job| (job.stage.clone(), job.name.clone()))
                .collect();
            keys.sort();
            keys
        };
        if job_keys(&current.jobs) != job_keys(&previous.jobs) && !warnings.contains(&"layout") {
            warnings.push("layout");
        }
    }
    if a.revision != b.revision {
        warnings.push("revision");
    }
    if a.worker_id != b.worker_id {
        warnings.push("runner");
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn seed(
        pool: &PgPool,
        name: &str,
        branch: &str,
        revision: &str,
        seconds_ago: i32,
        status: &str,
    ) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO pipelines(id,repository_url,pipeline_name,branch,revision,created_at,status,graph_definition,runner_os,working_directory) VALUES($1,'lores://fixture/repo',$2,$3,$4,now()-make_interval(secs => $5::double precision),$6,'{\"stages\":[]}', 'linux','.')")
            .bind(id).bind(name).bind(branch).bind(revision).bind(seconds_ago as f64).bind(status).execute(pool).await.unwrap();
        id
    }

    #[sqlx::test]
    #[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
    async fn insights_scope_baseline_and_comparison(pool: PgPool) {
        let user = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO users(id,google_sub,email) VALUES($1,'insights','insights@example.test')",
        )
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO lore_resources(resource_id,name,owner_subject,created_at) VALUES('insights','repo',$1,now()-interval '1 day')")
            .bind(user.to_string()).execute(&pool).await.unwrap();
        let repositories = super::super::repositories::RepositoryService::new(
            "/usr/bin/false",
            "lores://fixture",
            "lores://fixture",
        )
        .unwrap();
        let access = PipelineAccess::new(&repositories, user);
        let previous = seed(&pool, "server", "main", "old", 300, "succeeded").await;
        sqlx::query("UPDATE pipelines SET finished_at=now()-interval '200 seconds' WHERE id=$1")
            .bind(previous)
            .execute(&pool)
            .await
            .unwrap();
        let current = seed(&pool, "server", "main", "current", 100, "running").await;
        sqlx::query("UPDATE pipelines SET pipeline_needs=ARRAY['tools','missing'],started_at=now()-interval '90 seconds' WHERE id=$1").bind(current).execute(&pool).await.unwrap();
        let upstream = seed(&pool, "tools", "main", "current", 110, "running").await;
        seed(&pool, "tools", "other", "current", 1, "succeeded").await;
        seed(&pool, "tools", "main", "different", 1, "succeeded").await;
        let downstream = seed(&pool, "deploy", "main", "current", 40, "queued").await;
        let obsolete = seed(&pool, "publish", "main", "current", 50, "queued").await;
        for id in [downstream, obsolete] {
            sqlx::query("UPDATE pipelines SET pipeline_needs=ARRAY['server'] WHERE id=$1")
                .bind(id)
                .execute(&pool)
                .await
                .unwrap();
        }
        // Only the latest run of each downstream name defines the relationship.
        seed(&pool, "publish", "main", "current", 20, "queued").await;
        let overlapping = seed(&pool, "server", "main", "old", 150, "succeeded").await;
        let later = seed(&pool, "server", "main", "current", 20, "succeeded").await;
        for id in [overlapping, later] {
            sqlx::query("UPDATE pipelines SET finished_at=now()-interval '10 seconds' WHERE id=$1")
                .bind(id)
                .execute(&pool)
                .await
                .unwrap();
        }
        let wrong_branch = seed(&pool, "server", "other", "old", 200, "succeeded").await;
        sqlx::query("UPDATE pipelines SET finished_at=now()-interval '110 seconds' WHERE id=$1")
            .bind(wrong_branch)
            .execute(&pool)
            .await
            .unwrap();
        let result = read_insights(&pool, current, &access).await.unwrap();
        assert_eq!(result.current.pipeline.id, current);
        assert_eq!(result.previous.unwrap().pipeline.id, previous);
        assert_eq!(result.upstream.len(), 2);
        assert_eq!(result.upstream[0].run_id, Some(upstream));
        assert_eq!(result.upstream[1].run_id, None);
        assert_eq!(result.downstream.len(), 1);
        assert_eq!(result.downstream[0].run_id, Some(downstream));
        assert!(result.comparable);
        assert_eq!(result.comparison_warnings, ["revision"]);
        // Different execution conditions disable numeric delta calculations.
        sqlx::query("UPDATE pipelines SET runner_os='windows',working_directory='elsewhere',sparse_view_name='Different',graph_definition=NULL WHERE id=$1").bind(previous).execute(&pool).await.unwrap();
        let result = read_insights(&pool, current, &access).await.unwrap();
        assert!(!result.comparable);
        for reason in [
            "runner_os",
            "working_directory",
            "sparse_view",
            "snapshot_missing",
        ] {
            assert!(result.comparison_warnings.contains(&reason));
        }
        let absent = read_insights(&pool, upstream, &access).await.unwrap();
        assert!(absent.previous.is_none());
        assert!(!absent.comparable);
        assert!(read_insights(&pool, Uuid::new_v4(), &access).await.is_err());
        let status: String = sqlx::query_scalar("SELECT status FROM pipelines WHERE id=$1")
            .bind(current)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "running");
    }
}
