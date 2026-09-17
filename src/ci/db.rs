use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Postgres, Transaction, postgres::PgPoolOptions};
use uuid::Uuid;

use super::config::{PipelineConfig, SubmitPipeline};

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct Pipeline {
    pub id: Uuid,
    pub repository_url: String,
    pub revision: String,
    pub revision_number: i64,
    pub pipeline_name: Option<String>,
    pub category: Option<String>,
    pub runner_os: Option<String>,
    pub branch: Option<String>,
    pub previous_revision: Option<String>,
    pub trigger_patterns: Vec<String>,
    pub changed_paths: Vec<String>,
    pub changed_path_count: i32,
    pub working_directory: Option<String>,
    pub sparse_view_name: Option<String>,
    #[serde(default, skip_serializing)]
    pub sparse_view_rules: Option<String>,
    #[serde(default, skip_serializing)]
    pub graph_definition: Option<String>,
    pub status: String,
    pub worker_id: Option<Uuid>,
    pub submitted_by: Option<Uuid>,
    pub lease_until: Option<DateTime<Utc>>,
    pub cancel_requested: bool,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

pub struct SelectedPipeline {
    pub pipeline_name: String,
    pub category: String,
    pub runner_os: String,
    pub trigger_patterns: Vec<String>,
    pub working_directory: String,
    pub graph_definition: String,
    pub sparse_view_name: Option<String>,
    pub sparse_view_rules: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct Job {
    pub id: Uuid,
    pub pipeline_id: Uuid,
    pub position: i32,
    pub name: String,
    pub stage: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Log {
    pub id: i64,
    pub pipeline_id: Uuid,
    pub job_id: Option<Uuid>,
    pub stream: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Runner {
    pub id: Uuid,
    pub name: String,
    pub os: String,
    pub arch: String,
    pub version: String,
    pub docker_available: Option<bool>,
    pub status: String,
    pub current_pipeline_id: Option<Uuid>,
    pub started_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveRunner {
    Removed,
    NotFound,
    Online,
    Busy,
}

pub async fn connect(url: &str) -> anyhow::Result<PgPool> {
    let pool = connect_existing(url).await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}

/// Runners consume the schema owned and migrated by the coordinator.
pub async fn connect_existing(url: &str) -> anyhow::Result<PgPool> {
    Ok(PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(url)
        .await?)
}

pub async fn submit(pool: &PgPool, input: &SubmitPipeline) -> sqlx::Result<Pipeline> {
    sqlx::query_as(
        "INSERT INTO pipelines (id, repository_url, revision, branch) VALUES ($1,$2,$3,$4) RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(&input.repository_url)
    .bind(input.revision.to_ascii_lowercase())
    .bind(&input.branch)
    .fetch_one(pool)
    .await
}

pub async fn submit_for_user(
    pool: &PgPool,
    input: &SubmitPipeline,
    user_id: Uuid,
) -> sqlx::Result<Pipeline> {
    sqlx::query_as(
        "INSERT INTO pipelines (id, repository_url, revision, branch, submitted_by) VALUES ($1,$2,$3,$4,$5) RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(&input.repository_url)
    .bind(input.revision.to_ascii_lowercase())
    .bind(&input.branch)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

pub async fn submit_selected_for_user(
    tx: &mut Transaction<'_, Postgres>,
    input: &SubmitPipeline,
    user_id: Uuid,
    selected: &SelectedPipeline,
) -> sqlx::Result<Pipeline> {
    sqlx::query_as(
        "INSERT INTO pipelines (id, repository_url, revision, branch, submitted_by, pipeline_name, category, runner_os, trigger_patterns, working_directory, graph_definition, sparse_view_name, sparse_view_rules) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13) RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(&input.repository_url)
    .bind(input.revision.to_ascii_lowercase())
    .bind(&input.branch)
    .bind(user_id)
    .bind(&selected.pipeline_name)
    .bind(&selected.category)
    .bind(&selected.runner_os)
    .bind(&selected.trigger_patterns)
    .bind(&selected.working_directory)
    .bind(&selected.graph_definition)
    .bind(&selected.sparse_view_name)
    .bind(&selected.sparse_view_rules)
    .fetch_one(&mut **tx)
    .await
}

// A crashed worker is failed, never automatically replayed: scripts may have side effects.
pub async fn reap(pool: &PgPool) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    let expired: Vec<Uuid> = sqlx::query_scalar("UPDATE pipelines SET status = CASE WHEN cancel_requested THEN 'canceled' ELSE 'failed' END, error = 'worker lease expired', finished_at = now(), lease_until = NULL WHERE status = 'running' AND lease_until < now() RETURNING id")
        .fetch_all(&mut *tx).await?;
    sqlx::query("UPDATE jobs j SET status = CASE WHEN p.status = 'canceled' THEN 'canceled' WHEN j.status = 'running' THEN 'failed' ELSE 'skipped' END, finished_at = now() FROM pipelines p WHERE j.pipeline_id = p.id AND j.pipeline_id = ANY($1) AND j.status IN ('queued','running')")
        .bind(&expired).execute(&mut *tx).await?;
    tx.commit().await
}

pub async fn register_runner(
    pool: &PgPool,
    id: Uuid,
    name: &str,
    os: &str,
    arch: &str,
    version: &str,
    docker_available: bool,
) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO runners (id,name,os,arch,version,docker_available) VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, os = EXCLUDED.os, arch = EXCLUDED.arch, version = EXCLUDED.version, docker_available = EXCLUDED.docker_available, started_at = now(), last_seen = now(), stopped_at = NULL")
        .bind(id)
        .bind(name)
        .bind(os)
        .bind(arch)
        .bind(version)
        .bind(docker_available)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn touch_runner(pool: &PgPool, id: Uuid) -> sqlx::Result<bool> {
    let updated =
        sqlx::query("UPDATE runners SET last_seen = now(), stopped_at = NULL WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
    Ok(updated.rows_affected() == 1)
}

pub async fn stop_runner(pool: &PgPool, id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE runners SET last_seen = now(), stopped_at = now() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_runners(pool: &PgPool) -> sqlx::Result<Vec<Runner>> {
    sqlx::query_as(
        "SELECT r.id, r.name, r.os, r.arch, r.version, r.docker_available, CASE WHEN r.stopped_at IS NULL AND r.last_seen >= now() - interval '15 seconds' THEN 'online' ELSE 'offline' END AS status, active.id AS current_pipeline_id, r.started_at, r.last_seen FROM runners r LEFT JOIN LATERAL (SELECT id FROM pipelines WHERE worker_id = r.id AND status = 'running' ORDER BY started_at DESC LIMIT 1) active ON true ORDER BY (r.stopped_at IS NULL AND r.last_seen >= now() - interval '15 seconds') DESC, r.name, r.id",
    )
    .fetch_all(pool)
    .await
}

pub async fn remove_runner(pool: &PgPool, id: Uuid) -> sqlx::Result<RemoveRunner> {
    let mut tx = pool.begin().await?;
    let runner: Option<(bool, bool)> = sqlx::query_as(
        "SELECT stopped_at IS NULL AND last_seen >= now() - interval '15 seconds' AS online, EXISTS(SELECT 1 FROM pipelines WHERE worker_id = $1 AND status = 'running') AS busy FROM runners WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let outcome = match runner {
        None => RemoveRunner::NotFound,
        Some((_, true)) => RemoveRunner::Busy,
        Some((true, false)) => RemoveRunner::Online,
        Some((false, false)) => {
            sqlx::query("DELETE FROM runners WHERE id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            RemoveRunner::Removed
        }
    };
    tx.commit().await?;
    Ok(outcome)
}

pub async fn claim(pool: &PgPool, worker: Uuid) -> sqlx::Result<Option<Pipeline>> {
    sqlx::query_as("UPDATE pipelines SET status = 'running', worker_id = $1, started_at = now(), lease_until = now() + interval '30 seconds' WHERE id = (SELECT id FROM pipelines WHERE status = 'queued' AND NOT cancel_requested AND (runner_os IS NULL OR runner_os = (SELECT os FROM runners WHERE id = $1 AND stopped_at IS NULL)) ORDER BY created_at, id FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING *")
        .bind(worker).fetch_optional(pool).await
}

pub async fn heartbeat(pool: &PgPool, id: Uuid, worker: Uuid) -> sqlx::Result<bool> {
    let updated = sqlx::query("UPDATE pipelines SET lease_until = now() + interval '30 seconds' WHERE id = $1 AND worker_id = $2 AND status = 'running' AND NOT cancel_requested AND lease_until > now()")
        .bind(id).bind(worker).execute(pool).await?;
    Ok(updated.rows_affected() == 1)
}

pub async fn cancel(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<Pipeline>> {
    sqlx::query_as("UPDATE pipelines SET cancel_requested = CASE WHEN status IN ('queued','running') THEN true ELSE cancel_requested END, status = CASE WHEN status = 'queued' THEN 'canceled' ELSE status END, finished_at = CASE WHEN status = 'queued' THEN now() ELSE finished_at END WHERE id = $1 RETURNING *")
        .bind(id).fetch_optional(pool).await
}

pub async fn finish(
    pool: &PgPool,
    id: Uuid,
    worker: Uuid,
    status: &str,
    error: Option<&str>,
) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    let final_status: Option<String> = sqlx::query_scalar("UPDATE pipelines SET status = CASE WHEN cancel_requested THEN 'canceled' ELSE $3 END, error = $4, finished_at = now(), lease_until = NULL WHERE id = $1 AND worker_id = $2 AND status = 'running' AND lease_until > now() RETURNING status")
        .bind(id).bind(worker).bind(status).bind(error).fetch_optional(&mut *tx).await?;
    if let Some(status) = final_status {
        sqlx::query("UPDATE jobs SET status = CASE WHEN $2 = 'canceled' THEN 'canceled' WHEN status = 'running' THEN 'failed' ELSE 'skipped' END, finished_at = now() WHERE pipeline_id = $1 AND status IN ('queued','running')")
            .bind(id).bind(status).execute(&mut *tx).await?;
    }
    tx.commit().await
}

pub async fn create_jobs(
    pool: &PgPool,
    id: Uuid,
    worker: Uuid,
    config: &PipelineConfig,
) -> anyhow::Result<Vec<Job>> {
    let mut tx = pool.begin().await?;
    let active: Option<Uuid> = sqlx::query_scalar("SELECT id FROM pipelines WHERE id = $1 AND worker_id = $2 AND status = 'running' AND NOT cancel_requested AND lease_until > now() FOR UPDATE")
        .bind(id).bind(worker).fetch_optional(&mut *tx).await?;
    anyhow::ensure!(active.is_some(), "pipeline is no longer active");
    let mut jobs = Vec::new();
    for (position, job) in config.jobs.iter().enumerate() {
        jobs.push(sqlx::query_as("INSERT INTO jobs (id,pipeline_id,position,name,stage) VALUES ($1,$2,$3,$4,$5) RETURNING *")
            .bind(Uuid::new_v4()).bind(id).bind(position as i32).bind(&job.name).bind(&job.stage)
            .fetch_one(&mut *tx).await?);
    }
    tx.commit().await?;
    Ok(jobs)
}

pub async fn job_status(
    pool: &PgPool,
    id: Uuid,
    worker: Uuid,
    status: &str,
    code: Option<i32>,
) -> anyhow::Result<()> {
    // Lock the parent first, matching finish/reap and fencing stale workers.
    let mut tx = pool.begin().await?;
    let active: Option<Uuid> = sqlx::query_scalar("SELECT p.id FROM pipelines p JOIN jobs j ON j.pipeline_id = p.id WHERE j.id = $1 AND p.worker_id = $2 AND p.status = 'running' AND NOT p.cancel_requested AND p.lease_until > now() FOR UPDATE OF p")
        .bind(id).bind(worker).fetch_optional(&mut *tx).await?;
    anyhow::ensure!(active.is_some(), "pipeline is no longer active");
    sqlx::query("UPDATE jobs SET status = $2, exit_code = $3, started_at = CASE WHEN $2 = 'running' THEN now() ELSE started_at END, finished_at = CASE WHEN $2 <> 'running' THEN now() ELSE NULL END WHERE id = $1")
        .bind(id).bind(status).bind(code).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn log(
    pool: &PgPool,
    pipeline: Uuid,
    job: Option<Uuid>,
    stream: &str,
    content: &str,
) -> sqlx::Result<()> {
    // PostgreSQL text cannot contain NUL. Keep arbitrary process output readable.
    sqlx::query("INSERT INTO logs (pipeline_id,job_id,stream,content) VALUES ($1,$2,$3,$4)")
        .bind(pipeline)
        .bind(job)
        .bind(stream)
        .bind(content.replace('\0', "�"))
        .execute(pool)
        .await?;
    Ok(())
}

/// Append runner output only while its pipeline lease is active.
///
/// The ownership check and insert are one statement so finish/cancel cannot race
/// a separate authorization query. When a job is supplied, it must belong to the
/// same pipeline.
pub async fn worker_log(
    pool: &PgPool,
    worker: Uuid,
    pipeline: Uuid,
    job: Option<Uuid>,
    stream: &str,
    content: &str,
) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "INSERT INTO logs (pipeline_id,job_id,stream,content) \
         SELECT p.id,$3,$4,$5 FROM pipelines p \
         WHERE p.id=$1 AND p.worker_id=$2 AND p.status='running' \
           AND NOT p.cancel_requested AND p.lease_until>now() \
           AND ($3::uuid IS NULL OR EXISTS(SELECT 1 FROM jobs j WHERE j.id=$3 AND j.pipeline_id=p.id))",
    )
    .bind(pipeline)
    .bind(worker)
    .bind(job)
    .bind(stream)
    .bind(content.replace('\0', "�"))
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}
