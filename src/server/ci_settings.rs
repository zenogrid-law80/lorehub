//! Repository-scoped CI editing, version history and reproducible reruns.
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header::CACHE_CONTROL},
};
use serde::{Deserialize, Serialize};
use sqlx::types::Json as DbJson;
use uuid::Uuid;

use super::{
    api::{self, ApiError, AppState},
    auth::AuthSession,
    pipeline_access::PipelineAccess,
    triggers,
};
use crate::ci::{config::PipelineFile, db::Pipeline, settings};

fn bad(error: impl std::fmt::Display) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, error.to_string())
}
pub(super) fn error(error: anyhow::Error) -> ApiError {
    if error.is::<settings::Conflict>() {
        return ApiError(StatusCode::CONFLICT, error.to_string());
    }
    if error.is::<sqlx::Error>() {
        return api::internal_error(error);
    }
    bad(error)
}

#[derive(Deserialize)]
pub(super) struct Scope {
    branch: String,
}

#[derive(Serialize)]
pub(super) struct ConfigResponse {
    branch: String,
    revision: Option<String>,
    content: Option<String>,
    configuration: Option<PipelineFile>,
    is_link_source: bool,
    source_mode: String,
    lock_version: i64,
    config_revision_id: Option<Uuid>,
    config_version: Option<i64>,
}

async fn context(
    state: &AppState,
    session: &AuthSession,
    name: &str,
    branch: &str,
) -> Result<(String, super::repositories::StorageBackend, String, String), ApiError> {
    let resource = api::require_repository_access(&state.pool, name, session).await?;
    if api::repository_is_link_source(&state.pool, &resource).await? {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "CI settings are unavailable for Lore link source repositories.".into(),
        ));
    }
    let token = api::user_access_token(state, session).await?;
    let backend = state.repositories.storage_backend(name, &token).await?;
    let head = state
        .repositories
        .branches_on(name, backend, &token)
        .await?
        .into_iter()
        .find(|b| b.name == branch)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "branch not found".into()))?;
    Ok((resource, backend, token, head.revision))
}

pub(super) async fn read(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Query(scope): Query<Scope>,
) -> Result<(HeaderMap, Json<ConfigResponse>), ApiError> {
    let resource = api::require_repository_access(&state.pool, &name, &session).await?;
    let mut headers = HeaderMap::new();
    headers.insert(CACHE_CONTROL, "no-store".parse().unwrap());
    if api::repository_is_link_source(&state.pool, &resource).await? {
        return Ok((
            headers,
            Json(ConfigResponse {
                branch: scope.branch,
                revision: None,
                content: None,
                configuration: None,
                is_link_source: true,
                source_mode: "file".into(),
                lock_version: 0,
                config_revision_id: None,
                config_version: None,
            }),
        ));
    }
    let (_, backend, token, code) = context(&state, &session, &name, &scope.branch).await?;
    let mut conn = state.pool.acquire().await?;
    let head = settings::head(&mut conn, &resource, &scope.branch)
        .await
        .map_err(error)?;
    let lock_version = head.as_ref().map_or(0, |h| h.lock_version);
    if let Some(head) = head.filter(|h| h.source_mode == "db") {
        let version = settings::revision(&mut conn, head.id, head.active_revision_id.unwrap())
            .await
            .map_err(error)?;
        return Ok((
            headers,
            Json(response(scope.branch, code, lock_version, version)),
        ));
    }
    drop(conn);
    let content = state
        .repositories
        .pipeline_source_on(&name, &code, backend, &token)
        .await?;
    let configuration = content.as_deref().and_then(|s| PipelineFile::parse(s).ok());
    Ok((
        headers,
        Json(ConfigResponse {
            branch: scope.branch,
            revision: Some(code),
            content,
            configuration,
            is_link_source: false,
            source_mode: "file".into(),
            lock_version,
            config_revision_id: None,
            config_version: None,
        }),
    ))
}

fn response(
    branch: String,
    code: String,
    lock_version: i64,
    version: settings::Revision,
) -> ConfigResponse {
    ConfigResponse {
        branch,
        revision: Some(code),
        content: Some(version.source_toml),
        configuration: Some(version.definition.0),
        is_link_source: false,
        source_mode: "db".into(),
        lock_version,
        config_revision_id: Some(version.id),
        config_version: Some(version.version),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SaveRequest {
    branch: String,
    expected_revision: String,
    #[serde(default)]
    expected_lock_version: i64,
    content: Option<String>,
    configuration: Option<PipelineFile>,
}

pub(super) async fn save(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Json(input): Json<SaveRequest>,
) -> Result<Json<ConfigResponse>, ApiError> {
    let resource = api::require_repository_access(&state.pool, &name, &session).await?;
    if api::repository_is_link_source(&state.pool, &resource).await? {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "CI settings are unavailable for Lore link source repositories.".into(),
        ));
    }
    let content = match (input.content, input.configuration) {
        (Some(content), None) => content,
        (None, Some(config)) => {
            toml::to_string_pretty(&config.validate().map_err(bad)?).map_err(bad)?
        }
        _ => return Err(bad("provide content or configuration, but not both")),
    };
    PipelineFile::parse(&content).map_err(|e| bad(format!("invalid .lore-ci.toml: {e}")))?;
    let (resource, backend, _, code) = context(&state, &session, &name, &input.branch).await?;
    let mut tx = state.pool.begin().await?;
    settings::lock(&mut tx, &resource).await.map_err(error)?;
    let head = settings::head(&mut tx, &resource, &input.branch)
        .await
        .map_err(error)?;
    if head.as_ref().is_none_or(|h| h.source_mode == "file") && input.expected_revision != code {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "branch head changed; reload before importing CI settings".into(),
        ));
    }
    let version = settings::save(
        &mut tx,
        settings::Save {
            resource: &resource,
            branch: &input.branch,
            expected_lock_version: input.expected_lock_version,
            content: &content,
            source_revision: Some(&code),
            actor: session.user.id,
        },
    )
    .await
    .map_err(error)?;
    let url = state
        .repositories
        .public_repository_url_for(backend, &name)?;
    triggers::sync_routes(
        &mut tx,
        &resource,
        &url,
        &input.branch,
        &code,
        Some(&version.definition),
    )
    .await
    .map_err(error)?;
    tx.commit().await?;
    Ok(Json(response(
        input.branch,
        code,
        input.expected_lock_version + 1,
        version,
    )))
}

pub(super) async fn history(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Query(scope): Query<Scope>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let resource = api::require_repository_access(&state.pool, &name, &session).await?;
    let items: Vec<DbJson<serde_json::Value>> = sqlx::query_scalar("SELECT jsonb_build_object('id',r.id,'version',r.version,'content_hash',r.content_hash,'created_by',r.created_by,'created_at',r.created_at,'source_revision',r.source_revision) FROM ci_config_revisions r JOIN ci_configs c ON c.id=r.config_id WHERE c.resource_id=$1 AND c.branch=$2 ORDER BY r.version DESC LIMIT 100")
        .bind(&resource).bind(&scope.branch).fetch_all(&state.pool).await?;
    let events: Vec<DbJson<serde_json::Value>> = sqlx::query_scalar("SELECT to_jsonb(e) FROM ci_config_events e JOIN ci_configs c ON c.id=e.config_id WHERE c.resource_id=$1 AND c.branch=$2 ORDER BY e.id DESC LIMIT 100")
        .bind(&resource).bind(&scope.branch).fetch_all(&state.pool).await?;
    Ok(Json(
        serde_json::json!({"versions": items, "events": events}),
    ))
}

pub(super) async fn version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path((name, id)): Path<(String, Uuid)>,
    Query(scope): Query<Scope>,
) -> Result<Json<settings::Revision>, ApiError> {
    let resource = api::require_repository_access(&state.pool, &name, &session).await?;
    let mut conn = state.pool.acquire().await?;
    let head = settings::head(&mut conn, &resource, &scope.branch)
        .await
        .map_err(error)?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "CI settings not found".into()))?;
    let version = settings::revision(&mut conn, head.id, id)
        .await
        .map_err(|_| ApiError(StatusCode::NOT_FOUND, "CI version not found".into()))?;
    Ok(Json(version))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RestoreRequest {
    branch: String,
    expected_lock_version: i64,
    revision_id: Uuid,
}

pub(super) async fn restore(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Json(input): Json<RestoreRequest>,
) -> Result<Json<ConfigResponse>, ApiError> {
    let (resource, backend, _, code) = context(&state, &session, &name, &input.branch).await?;
    let mut tx = state.pool.begin().await?;
    settings::lock(&mut tx, &resource).await.map_err(error)?;
    let head = checked_head(
        &mut tx,
        &resource,
        &input.branch,
        input.expected_lock_version,
    )
    .await?;
    let version = settings::revision(&mut tx, head.id, input.revision_id)
        .await
        .map_err(error)?;
    version.definition.0.clone().validate().map_err(bad)?;
    settings::activate(
        &mut tx,
        head.id,
        head.active_revision_id,
        version.id,
        session.user.id,
        "restore",
    )
    .await
    .map_err(error)?;
    let url = state
        .repositories
        .public_repository_url_for(backend, &name)?;
    triggers::sync_routes(
        &mut tx,
        &resource,
        &url,
        &input.branch,
        &code,
        Some(&version.definition),
    )
    .await
    .map_err(error)?;
    tx.commit().await?;
    Ok(Json(response(
        input.branch,
        code,
        input.expected_lock_version + 1,
        version,
    )))
}

async fn checked_head(
    conn: &mut sqlx::PgConnection,
    resource: &str,
    branch: &str,
    expected: i64,
) -> Result<settings::ConfigHead, ApiError> {
    let head = settings::head(conn, resource, branch)
        .await
        .map_err(error)?
        .ok_or_else(|| bad("CI settings not found"))?;
    if head.lock_version != expected {
        return Err(error(settings::Conflict.into()));
    }
    Ok(head)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FileRequest {
    branch: String,
    expected_lock_version: i64,
}

pub(super) async fn file_mode(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Json(input): Json<FileRequest>,
) -> Result<StatusCode, ApiError> {
    let (resource, backend, token, code) = context(&state, &session, &name, &input.branch).await?;
    let content = state
        .repositories
        .pipeline_source_on(&name, &code, backend, &token)
        .await?
        .ok_or_else(|| {
            bad("Export and commit the active configuration before switching to file mode")
        })?;
    let file = PipelineFile::parse(&content).map_err(bad)?;
    let mut tx = state.pool.begin().await?;
    settings::lock(&mut tx, &resource).await.map_err(error)?;
    let head = checked_head(
        &mut tx,
        &resource,
        &input.branch,
        input.expected_lock_version,
    )
    .await?;
    let version = settings::revision(
        &mut tx,
        head.id,
        head.active_revision_id
            .ok_or_else(|| bad("No active CI configuration"))?,
    )
    .await
    .map_err(error)?;
    if serde_json::to_value(&file).map_err(bad)?
        != serde_json::to_value(&version.definition).map_err(bad)?
    {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "Commit the active DB configuration as .lore-ci.toml before switching to file mode"
                .into(),
        ));
    }
    sqlx::query("UPDATE ci_configs SET source_mode='file',lock_version=lock_version+1,updated_at=now() WHERE id=$1").bind(head.id).execute(&mut *tx).await?;
    settings::event(
        &mut tx,
        head.id,
        head.active_revision_id,
        head.active_revision_id,
        session.user.id,
        "file",
    )
    .await
    .map_err(error)?;
    let url = state
        .repositories
        .public_repository_url_for(backend, &name)?;
    triggers::sync_routes(&mut tx, &resource, &url, &input.branch, &code, Some(&file))
        .await
        .map_err(error)?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn rerun(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<Pipeline>), ApiError> {
    let original = PipelineAccess::new(&state.repositories, session.user.id)
        .pipeline(&state.pool, id)
        .await?;
    let Some(group) = original.run_group_id else {
        return Err(bad("File-based runs must be started from CI settings"));
    };
    let mut tx = state.pool.begin().await?;
    let (resource, branch, code, config): (String, String, String, Uuid) = sqlx::query_as(
        "SELECT resource_id,branch,code_revision,config_revision_id FROM ci_run_groups WHERE id=$1",
    )
    .bind(group)
    .fetch_one(&mut *tx)
    .await?;
    if api::repository_is_link_source(&state.pool, &resource).await? {
        return Err(bad(
            "CI settings are unavailable for Lore link source repositories.",
        ));
    }
    let new_group = settings::group(&mut tx, &resource, &branch, &code, config, "rerun")
        .await
        .map_err(error)?
        .unwrap();
    // Replay the entire original group so dependencies never borrow another run's result.
    let rows: Vec<Pipeline> =
        sqlx::query_as("SELECT * FROM pipelines WHERE run_group_id=$1 ORDER BY created_at,id")
            .bind(group)
            .fetch_all(&mut *tx)
            .await?;
    let mut selected = None;
    for row in rows {
        let rerun: Pipeline = sqlx::query_as("INSERT INTO pipelines(id,repository_url,revision,branch,submitted_by,pipeline_name,pipeline_needs,category,runner_os,trigger_patterns,working_directory,graph_definition,sparse_view_name,sparse_view_rules,config_revision_id,run_group_id,execution_spec) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17) RETURNING *")
            .bind(Uuid::new_v4()).bind(&row.repository_url).bind(&row.revision).bind(&row.branch).bind(session.user.id)
            .bind(&row.pipeline_name).bind(&row.pipeline_needs).bind(&row.category).bind(&row.runner_os).bind(&row.trigger_patterns)
            .bind(&row.working_directory).bind(&row.graph_definition).bind(&row.sparse_view_name).bind(&row.sparse_view_rules)
            .bind(config).bind(new_group).bind(&row.execution_spec).fetch_one(&mut *tx).await?;
        if row.id == id {
            selected = Some(rerun);
        }
    }
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(selected.ok_or_else(|| api::internal_error("original run disappeared"))?),
    ))
}
