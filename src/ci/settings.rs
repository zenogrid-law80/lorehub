//! Versioned CI definitions and immutable execution specifications.
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgConnection, types::Json};
use uuid::Uuid;

use super::{
    config::{PipelineConfig, PipelineFile},
    db::Pipeline,
};

#[derive(Debug)]
pub struct Conflict;
impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CI settings changed; reload before saving")
    }
}
impl std::error::Error for Conflict {}

#[derive(Debug, Serialize, FromRow)]
pub struct ConfigHead {
    pub id: Uuid,
    pub source_mode: String,
    pub active_revision_id: Option<Uuid>,
    pub lock_version: i64,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Revision {
    pub id: Uuid,
    pub config_id: Uuid,
    pub version: i64,
    pub schema_version: i32,
    pub definition: Json<PipelineFile>,
    pub source_toml: String,
    pub content_hash: String,
    pub source_revision: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionSpec {
    pub schema_version: i32,
    pub config: PipelineConfig,
    pub working_directory: String,
    pub runner_os: Option<String>,
    pub sparse_view_name: Option<String>,
    pub sparse_view_rules: Option<String>,
}

impl ExecutionSpec {
    pub fn validate(&mut self) -> Result<()> {
        ensure!(
            self.schema_version == 1,
            "unsupported CI execution specification"
        );
        self.config.validate()?;
        ensure!(
            self.working_directory == "."
                || super::config::valid_relative_path(&self.working_directory),
            "invalid working directory"
        );
        ensure!(
            self.runner_os
                .as_deref()
                .is_none_or(|os| matches!(os, "linux" | "macos" | "windows")),
            "invalid runner OS"
        );
        Ok(())
    }
}

// This is also the watcher's repository lock: saves and push admission are ordered.
pub async fn lock(conn: &mut PgConnection, resource: &str) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(resource)
        .execute(conn)
        .await?;
    Ok(())
}

pub async fn head(
    conn: &mut PgConnection,
    resource: &str,
    branch: &str,
) -> Result<Option<ConfigHead>> {
    Ok(sqlx::query_as("SELECT id,source_mode,active_revision_id,lock_version FROM ci_configs WHERE resource_id=$1 AND branch=$2")
        .bind(resource).bind(branch).fetch_optional(conn).await?)
}

pub async fn revision(conn: &mut PgConnection, config: Uuid, id: Uuid) -> Result<Revision> {
    sqlx::query_as("SELECT * FROM ci_config_revisions WHERE config_id=$1 AND id=$2")
        .bind(config)
        .bind(id)
        .fetch_optional(conn)
        .await?
        .context("CI configuration version not found")
}

pub async fn active(
    conn: &mut PgConnection,
    resource: &str,
    branch: &str,
) -> Result<Option<Revision>> {
    let Some(head) = head(conn, resource, branch).await? else {
        return Ok(None);
    };
    if head.source_mode != "db" {
        return Ok(None);
    }
    Ok(Some(
        revision(
            conn,
            head.id,
            head.active_revision_id
                .context("active CI version missing")?,
        )
        .await?,
    ))
}

pub struct Save<'a> {
    pub resource: &'a str,
    pub branch: &'a str,
    pub expected_lock_version: i64,
    pub content: &'a str,
    pub source_revision: Option<&'a str>,
    pub actor: Uuid,
}

pub async fn save(conn: &mut PgConnection, input: Save<'_>) -> Result<Revision> {
    let definition = PipelineFile::parse(input.content)?;
    lock(conn, input.resource).await?;
    let old = head(conn, input.resource, input.branch).await?;
    if old.as_ref().map_or(0, |h| h.lock_version) != input.expected_lock_version {
        return Err(Conflict.into());
    }
    let config_id = old.as_ref().map_or_else(Uuid::new_v4, |h| h.id);
    if old.is_none() {
        sqlx::query(
            "INSERT INTO ci_configs(id,resource_id,branch,source_mode) VALUES ($1,$2,$3,'file')",
        )
        .bind(config_id)
        .bind(input.resource)
        .bind(input.branch)
        .execute(&mut *conn)
        .await?;
    }
    let id = Uuid::new_v4();
    let hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&definition)?));
    sqlx::query("INSERT INTO ci_config_revisions(id,config_id,version,definition,source_toml,content_hash,source_revision,created_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(id).bind(config_id).bind(input.expected_lock_version + 1).bind(Json(definition))
        .bind(input.content).bind(hash).bind(input.source_revision).bind(input.actor).execute(&mut *conn).await?;
    activate(
        conn,
        config_id,
        old.and_then(|h| h.active_revision_id),
        id,
        input.actor,
        "save",
    )
    .await?;
    revision(conn, config_id, id).await
}

pub async fn activate(
    conn: &mut PgConnection,
    config: Uuid,
    previous: Option<Uuid>,
    id: Uuid,
    actor: Uuid,
    action: &str,
) -> Result<()> {
    sqlx::query("UPDATE ci_configs SET active_revision_id=$2,source_mode='db',lock_version=lock_version+1,updated_at=now() WHERE id=$1")
        .bind(config).bind(id).execute(&mut *conn).await?;
    event(conn, config, previous, Some(id), actor, action).await
}

pub async fn event(
    conn: &mut PgConnection,
    config: Uuid,
    previous: Option<Uuid>,
    revision: Option<Uuid>,
    actor: Uuid,
    action: &str,
) -> Result<()> {
    sqlx::query("INSERT INTO ci_config_events(config_id,action,previous_revision_id,revision_id,actor) VALUES($1,$2,$3,$4,$5)")
        .bind(config).bind(action).bind(previous).bind(revision).bind(actor).execute(conn).await?;
    Ok(())
}

pub async fn group(
    conn: &mut PgConnection,
    resource: &str,
    branch: &str,
    code: &str,
    config: Uuid,
    kind: &str,
) -> Result<Option<Uuid>> {
    Ok(sqlx::query_scalar("INSERT INTO ci_run_groups(id,resource_id,branch,code_revision,config_revision_id,trigger_kind) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING RETURNING id")
        .bind(Uuid::new_v4()).bind(resource).bind(branch).bind(code).bind(config).bind(kind).fetch_optional(conn).await?)
}

pub struct Run<'a> {
    pub group: Uuid,
    pub resource: &'a str,
    pub url: &'a str,
    pub branch: &'a str,
    pub code_revision: &'a str,
    pub previous_revision: Option<&'a str>,
    pub actor: Uuid,
    pub revision: &'a Revision,
    pub pipeline_name: Option<&'a str>,
    pub changes: &'a [String],
}

pub async fn enqueue(conn: &mut PgConnection, run: Run<'_>) -> Result<Pipeline> {
    let file = &run.revision.definition;
    let (config, directory, os, view) = file.select(run.pipeline_name)?;
    let selected = file
        .pipelines
        .iter()
        .find(|p| Some(p.name.as_str()) == run.pipeline_name);
    let (view_name, view_rules) =
        crate::server::triggers::sparse_view_snapshot_conn(conn, run.resource, view).await?;
    let spec = ExecutionSpec {
        schema_version: 1,
        config,
        working_directory: directory.to_owned(),
        runner_os: os.map(str::to_owned),
        sparse_view_name: view_name.clone(),
        sparse_view_rules: view_rules.clone(),
    };
    let graph = selected
        .map(crate::server::triggers::pipeline_graph_definition)
        .transpose()?;
    let needs = selected.map_or_else(Vec::new, |p| p.needs.clone());
    let patterns = selected.map_or_else(Vec::new, |p| {
        p.changes
            .iter()
            .filter(|pattern| {
                run.previous_revision.is_none()
                    || run
                        .changes
                        .iter()
                        .any(|path| super::config::change_path_matches(pattern, path))
            })
            .cloned()
            .collect::<Vec<_>>()
    });
    let paths = run
        .changes
        .iter()
        .filter(|path| {
            patterns
                .iter()
                .any(|p| super::config::change_path_matches(p, path))
        })
        .cloned()
        .collect::<Vec<_>>();
    Ok(sqlx::query_as("INSERT INTO pipelines(id,repository_url,revision,branch,submitted_by,pipeline_name,pipeline_needs,category,runner_os,previous_revision,trigger_patterns,changed_paths,changed_path_count,working_directory,graph_definition,sparse_view_name,sparse_view_rules,config_revision_id,run_group_id,execution_spec) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20) RETURNING *")
        .bind(Uuid::new_v4()).bind(run.url).bind(run.code_revision).bind(run.branch).bind(run.actor)
        .bind(run.pipeline_name).bind(needs).bind(selected.map(|p| &p.category)).bind(os).bind(run.previous_revision)
        .bind(patterns).bind(paths.iter().take(32).cloned().collect::<Vec<_>>()).bind(i32::try_from(paths.len())?)
        .bind(directory).bind(graph).bind(view_name).bind(view_rules).bind(run.revision.id).bind(run.group).bind(Json(spec))
        .fetch_one(conn).await?)
}
