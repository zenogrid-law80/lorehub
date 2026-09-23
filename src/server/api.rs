use axum::{
    Extension, Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{AUTHORIZATION, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE},
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    auth::{self, AuthService, AuthSession, User},
    management,
    pipeline_access::PipelineAccess,
    releases::RunnerReleases,
    repositories::{
        Branch, CommandError as RepositoryCommandError, Repository, RepositoryService,
        StorageBackend, link_path_is_below,
    },
    tokens::{IssuedToken, TokenIssuer},
    triggers, web,
};
use crate::ci::{
    config::{PipelineConfig, PipelineFile, SubmitPipeline},
    db::{self, Job, Log, Pipeline, Runner, SelectedPipeline},
};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub repositories: RepositoryService,
    pub tokens: Option<TokenIssuer>,
    pub runner_releases: RunnerReleases,
}

pub fn router(
    pool: PgPool,
    auth: AuthService,
    repositories: RepositoryService,
    tokens: Option<TokenIssuer>,
) -> Router {
    router_with_releases(pool, auth, repositories, tokens, RunnerReleases::default())
}

pub fn router_with_releases(
    pool: PgPool,
    auth: AuthService,
    repositories: RepositoryService,
    tokens: Option<TokenIssuer>,
    runner_releases: RunnerReleases,
) -> Router {
    let state = AppState {
        pool,
        repositories,
        tokens,
        runner_releases,
    };
    let private = Router::new()
        .merge(management::router())
        .route("/api/v1/me", get(me))
        .route("/api/v1/overview", get(super::overview::overview))
        .route("/api/v1/pipelines", post(submit).get(list))
        .route("/api/v1/pipeline-history", get(pipeline_history))
        .route("/api/v1/pipeline-graphs", get(pipeline_graphs))
        .route("/api/v1/pipelines/{id}", get(detail))
        .route(
            "/api/v1/pipelines/{id}/insights",
            get(super::execution::insights),
        )
        .route("/api/v1/pipelines/{id}/cancel", post(cancel))
        .route("/api/v1/pipelines/{id}/logs", get(logs))
        .route("/api/v1/runners", get(list_runners))
        .route("/api/v1/runners/{id}", delete(remove_runner))
        .route("/api/v1/runners/{id}/drain", post(set_runner_draining))
        .route("/downloads/runners/linux-x86_64", get(web::linux_runner))
        .route(
            "/downloads/runners/windows-x86_64",
            get(web::windows_runner),
        )
        .route("/downloads/runners/macos-aarch64", get(web::macos_runner))
        .route(
            "/api/v1/repositories",
            get(list_repositories).post(create_repository),
        )
        .route(
            "/api/v1/repositories/{name}/delete",
            post(delete_repository),
        )
        .route(
            "/api/v1/repositories/{name}/branches",
            get(list_repository_branches),
        )
        .route("/api/v1/repositories/{name}/tree", get(repository_tree))
        .route(
            "/api/v1/repositories/{name}/links",
            get(repository_links).post(super::links::create),
        )
        .route(
            "/api/v1/repository-links/summary",
            get(super::links::summary),
        )
        .route(
            "/api/v1/repositories/{name}/link-operations",
            get(super::links::operations),
        )
        .route(
            "/api/v1/repositories/{name}/link-operations/{id}/retry",
            post(super::links::retry),
        )
        .route(
            "/api/v1/repositories/{name}/links/policy",
            post(super::links::policy),
        )
        .route(
            "/api/v1/repositories/{name}/links/update",
            post(update_repository_link),
        )
        .route(
            "/api/v1/repositories/{name}/links/remove",
            post(remove_repository_link),
        )
        .route(
            "/api/v1/repositories/{name}/ci-config",
            get(repository_ci_config)
                .post(update_repository_ci_config)
                .layer(DefaultBodyLimit::max(300 * 1024)),
        )
        .route(
            "/api/v1/repositories/{name}/ci-config/parse",
            post(parse_repository_ci_config).layer(DefaultBodyLimit::max(300 * 1024)),
        )
        .route(
            "/api/v1/repositories/{name}/ci-config/analyze",
            post(analyze_repository_ci_config).layer(DefaultBodyLimit::max(600 * 1024)),
        )
        .route(
            "/api/v1/repositories/{name}/pipelines",
            get(list_repository_pipelines),
        )
        .route(
            "/api/v1/repositories/{name}/pipeline-branches",
            get(repository_pipeline_branches).post(update_repository_pipeline_branches),
        )
        .route("/api/v1/lore-token", post(issue_lore_token))
        .route_layer(middleware::from_fn_with_state(auth.clone(), require_login));
    let protected_auth = Router::new()
        .route("/auth/logout", post(auth::logout))
        .route_layer(middleware::from_fn_with_state(auth.clone(), require_login))
        .with_state(auth.clone());
    let public_auth = Router::new()
        .route("/auth/cli", get(auth::cli_login))
        .route("/auth/cli/complete", get(auth::cli_complete))
        .route("/auth/google/login", get(auth::login))
        .route("/auth/google/callback", get(auth::callback))
        .with_state(auth);
    let runner = Router::new()
        .route("/api/v1/runner/register", post(runner_register))
        .route("/api/v1/runner/touch", post(runner_touch))
        .route("/api/v1/runner/stop", post(runner_stop))
        .route("/api/v1/runner/claim", post(runner_claim))
        .route(
            "/api/v1/runner/claim/{request_id}",
            post(runner_claim_with_request),
        )
        .route(
            "/api/v1/runner/pipelines/{id}/heartbeat",
            post(runner_pipeline_heartbeat),
        )
        .route(
            "/api/v1/runner/pipelines/{id}/finish",
            post(runner_pipeline_finish),
        )
        .route(
            "/api/v1/runner/pipelines/{id}/jobs",
            post(runner_create_jobs),
        )
        .route(
            "/api/v1/runner/pipelines/{id}/access-token",
            post(runner_access_token),
        )
        .route("/api/v1/runner/jobs/{id}/status", post(runner_job_status))
        .route("/api/v1/runner/logs", post(runner_log))
        .layer(DefaultBodyLimit::max(300 * 1024));
    Router::new()
        .route("/", get(web::index))
        .route("/assets/app.css", get(web::styles))
        .route("/assets/lore-logo.svg", get(web::logo))
        .route("/assets/theme.js", get(web::theme_script))
        .route("/assets/app.js", get(web::script))
        .route("/assets/ci-visual.js", get(web::ci_script))
        .route("/assets/ci-editor.js", get(web::ci_editor_script))
        .route("/assets/execution-graph.js", get(web::execution_script))
        .route(
            "/assets/execution-analysis.js",
            get(web::execution_analysis_script),
        )
        .route("/assets/management.js", get(web::management_script))
        .route("/assets/operations.js", get(web::operations_script))
        .route("/assets/overview.js", get(web::overview_script))
        .route(
            "/assets/repository-context.js",
            get(web::repository_context_script),
        )
        .route("/healthz", get(health))
        .route("/.well-known/openid-configuration", get(oidc_discovery))
        .route("/.well-known/jwks.json", get(jwks))
        .route(
            "/api/v1/runner-updates/{os}/{arch}",
            get(runner_update_manifest),
        )
        .route(
            "/api/v1/runner-updates/{os}/{arch}/binary",
            get(runner_update_binary),
        )
        .merge(public_auth)
        .merge(protected_auth)
        .merge(runner)
        .merge(private)
        .layer(DefaultBodyLimit::max(16 * 1024))
        .with_state(state)
}

async fn runner_update_manifest(
    State(state): State<AppState>,
    Path((os, arch)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    require_runner_token(&state, &headers)?;
    let Some(release) = state.runner_releases.get(&os, &arch) else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    let mut response = Json(release.manifest).into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn runner_update_binary(
    State(state): State<AppState>,
    Path((os, arch)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    require_runner_token(&state, &headers)?;
    let release = state
        .runner_releases
        .get(&os, &arch)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "runner release not found".into()))?;
    let content = tokio::fs::read(&release.path).await.map_err(|error| {
        tracing::error!(path = %release.path.display(), %error, "read runner release failed");
        ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            "runner release is unavailable".into(),
        )
    })?;
    if content.len() as u64 != release.manifest.size {
        tracing::error!(path = %release.path.display(), "runner release changed after coordinator startup");
        return Err(ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            "runner release changed; restart the coordinator".into(),
        ));
    }
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/octet-stream")
        .header(CONTENT_LENGTH, content.len())
        .header(CACHE_CONTROL, "private, no-store")
        .body(Body::from(content))
        .map_err(|error| {
            tracing::error!(%error, "build runner release response failed");
            ApiError(StatusCode::INTERNAL_SERVER_ERROR, "response failed".into())
        })
}

fn require_runner_token(state: &AppState, headers: &HeaderMap) -> Result<Uuid, ApiError> {
    let authorization = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split_once(' '))
        .filter(|(scheme, token)| scheme.eq_ignore_ascii_case("bearer") && !token.is_empty())
        .map(|(_, token)| token)
        .ok_or_else(|| ApiError(StatusCode::UNAUTHORIZED, "runner token required".into()))?;
    let token = token_issuer(state)?
        .verify_access_token(authorization)
        .map_err(|_| ApiError(StatusCode::UNAUTHORIZED, "invalid runner token".into()))?;
    let worker_id = token
        .subject
        .strip_prefix("lorehub-worker:")
        .ok_or_else(|| ApiError(StatusCode::FORBIDDEN, "runner token required".into()))?;
    Uuid::parse_str(worker_id)
        .map_err(|_| ApiError(StatusCode::FORBIDDEN, "invalid runner identity".into()))
}

#[derive(Deserialize)]
struct RunnerRegister {
    name: String,
    os: String,
    arch: String,
    version: String,
    docker_available: bool,
}

async fn runner_register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RunnerRegister>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let worker = require_runner_token(&state, &headers)?;
    if input.name.is_empty()
        || input.name.len() > 128
        || input.name.chars().any(char::is_control)
        || !matches!(input.os.as_str(), "linux" | "macos" | "windows")
        || !matches!(input.arch.as_str(), "x86_64" | "aarch64")
        || input
            .version
            .parse::<crate::version::SemanticVersion>()
            .is_err()
    {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "invalid runner metadata".into(),
        ));
    }
    db::register_runner(
        &state.pool,
        worker,
        &input.name,
        &input.os,
        &input.arch,
        &input.version,
        input.docker_available,
    )
    .await?;
    Ok(Json(serde_json::json!({})))
}

async fn runner_touch(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let worker = require_runner_token(&state, &headers)?;
    Ok(Json(
        serde_json::json!({"active": db::touch_runner(&state.pool, worker).await?}),
    ))
}

async fn runner_stop(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let worker = require_runner_token(&state, &headers)?;
    db::stop_runner(&state.pool, worker).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn runner_claim(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let worker = require_runner_token(&state, &headers)?;
    runner_claim_response(db::claim(&state.pool, worker).await?)
}

async fn runner_claim_with_request(
    State(state): State<AppState>,
    Path(request_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let worker = require_runner_token(&state, &headers)?;
    runner_claim_response(db::claim_with_request(&state.pool, worker, request_id).await?)
}

fn runner_claim_response(pipeline: Option<Pipeline>) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(pipeline) = pipeline else {
        return Ok(Json(serde_json::Value::Null));
    };
    let rules = pipeline.sparse_view_rules.clone();
    let graph = pipeline.graph_definition.clone();
    let mut value = serde_json::to_value(pipeline).map_err(internal_error)?;
    let object = value
        .as_object_mut()
        .expect("pipeline serializes as an object");
    object.insert(
        "sparse_view_rules".into(),
        serde_json::to_value(rules).map_err(internal_error)?,
    );
    object.insert(
        "graph_definition".into(),
        serde_json::to_value(graph).map_err(internal_error)?,
    );
    Ok(Json(value))
}

async fn runner_pipeline_heartbeat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let worker = require_runner_token(&state, &headers)?;
    Ok(Json(
        serde_json::json!({"active": db::heartbeat(&state.pool, id, worker).await?}),
    ))
}

#[derive(Deserialize)]
struct RunnerFinish {
    status: String,
    error: Option<String>,
}

async fn runner_pipeline_finish(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RunnerFinish>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let worker = require_runner_token(&state, &headers)?;
    if !matches!(input.status.as_str(), "succeeded" | "failed") {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "invalid pipeline status".into(),
        ));
    }
    if input
        .error
        .as_ref()
        .is_some_and(|error| error.len() > 16 * 1024)
    {
        return Err(ApiError(
            StatusCode::PAYLOAD_TOO_LARGE,
            "pipeline error is too large".into(),
        ));
    }
    db::finish(
        &state.pool,
        id,
        worker,
        &input.status,
        input.error.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::json!({})))
}

async fn runner_create_jobs(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(mut config): Json<PipelineConfig>,
) -> Result<Json<Vec<Job>>, ApiError> {
    let worker = require_runner_token(&state, &headers)?;
    config
        .validate()
        .map_err(|error| ApiError(StatusCode::BAD_REQUEST, error.to_string()))?;
    Ok(Json(
        db::create_jobs(&state.pool, id, worker, &config)
            .await
            .map_err(internal_error)?,
    ))
}

#[derive(Deserialize)]
struct RunnerJobStatus {
    status: String,
    code: Option<i32>,
}

async fn runner_job_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RunnerJobStatus>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let worker = require_runner_token(&state, &headers)?;
    if !matches!(input.status.as_str(), "running" | "succeeded" | "failed") {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "invalid job status".into(),
        ));
    }
    db::job_status(&state.pool, id, worker, &input.status, input.code)
        .await
        .map_err(internal_error)?;
    Ok(Json(serde_json::json!({})))
}

#[derive(Deserialize)]
struct RunnerLog {
    pipeline: Uuid,
    job: Option<Uuid>,
    stream: String,
    content: String,
}

async fn runner_log(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RunnerLog>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let worker = require_runner_token(&state, &headers)?;
    if !matches!(input.stream.as_str(), "stdout" | "stderr" | "system") {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "invalid log stream".into(),
        ));
    }
    if input.content.len() > 64 * 1024 {
        return Err(ApiError(
            StatusCode::PAYLOAD_TOO_LARGE,
            "log chunk is too large".into(),
        ));
    }
    if !db::worker_log(
        &state.pool,
        worker,
        input.pipeline,
        input.job,
        &input.stream,
        &input.content,
    )
    .await?
    {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "pipeline is no longer active or job does not belong to it".into(),
        ));
    }
    Ok(Json(serde_json::json!({})))
}

async fn runner_access_token(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let worker = require_runner_token(&state, &headers)?;
    let pipeline: Pipeline = sqlx::query_as("SELECT * FROM pipelines WHERE id=$1 AND worker_id=$2 AND status='running' AND NOT cancel_requested AND lease_until>now()")
        .bind(id).bind(worker).fetch_optional(&state.pool).await?
        .ok_or_else(|| ApiError(StatusCode::CONFLICT, "pipeline is no longer active".into()))?;
    let submitter = pipeline
        .submitted_by
        .ok_or_else(|| ApiError(StatusCode::CONFLICT, "pipeline has no submitter".into()))?;
    let repository_name = url::Url::parse(&pipeline.repository_url)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(|mut segments| segments.next_back().map(str::to_owned))
        })
        .ok_or_else(|| {
            ApiError(
                StatusCode::CONFLICT,
                "pipeline repository URL is invalid".into(),
            )
        })?;
    let resource_id =
        super::repository_access::by_name(&state.pool, &submitter.to_string(), &repository_name)
            .await?
            .ok_or_else(|| {
                ApiError(
                    StatusCode::FORBIDDEN,
                    "pipeline submitter no longer has repository access".into(),
                )
            })?;
    let token = token_issuer(&state)?
        .issue_worker(&worker.to_string(), resource_id)
        .map_err(internal_error)?;
    Ok(Json(
        serde_json::json!({"access_token": token.access_token}),
    ))
}

fn internal_error(error: impl std::fmt::Display) -> ApiError {
    tracing::error!(%error, "runner API operation failed");
    ApiError(
        StatusCode::INTERNAL_SERVER_ERROR,
        "runner API operation failed".into(),
    )
}

async fn require_login(
    State(auth): State<AuthService>,
    mut request: Request,
    next: Next,
) -> Response {
    match auth::authenticate(&auth, request.method(), request.headers()).await {
        Ok(session) => {
            request.extensions_mut().insert(session);
            next.run(request).await
        }
        Err(error) => error.into_response(),
    }
}

async fn me(Extension(session): Extension<AuthSession>) -> Json<User> {
    Json(session.user)
}

#[derive(Debug)]
pub(super) struct ApiError(pub(super) StatusCode, pub(super) String);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(serde_json::json!({"error":self.1}))).into_response()
    }
}
impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        tracing::error!(%error, "database operation failed");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "database operation failed".into(),
        )
    }
}
impl From<RepositoryCommandError> for ApiError {
    fn from(error: RepositoryCommandError) -> Self {
        let lower = error.message.to_lowercase();
        let status = if lower.contains("already exists")
            || lower.contains("has changed")
            || lower.contains("conflict")
        {
            StatusCode::CONFLICT
        } else if lower.contains("not found") {
            StatusCode::NOT_FOUND
        } else if lower.contains("repository name")
            || lower.contains("description")
            || lower.contains("invalid .lore-ci.toml")
            || lower.contains(".lore-ci.toml exceeds")
            || lower.contains(".lore-ci.toml must")
            || lower.contains("branch must")
            || lower.contains("revision must")
            || lower.contains("link path")
            || lower.contains("link source path")
        {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::BAD_GATEWAY
        };
        Self(status, error.message)
    }
}

async fn health(State(state): State<AppState>) -> StatusCode {
    match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

async fn oidc_discovery(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(token_issuer(&state)?.discovery()))
}

async fn jwks(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(token_issuer(&state)?.jwks()))
}

async fn issue_lore_token(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
) -> Result<(HeaderMap, Json<IssuedToken>), ApiError> {
    let resources =
        super::repository_access::resource_ids(&state.pool, &session.user.id.to_string()).await?;
    let token = token_issuer(&state)?
        .issue_user(&session.user, resources)
        .map_err(|error| {
            tracing::error!(%error, "Lore access token issuance failed");
            ApiError(
                StatusCode::INTERNAL_SERVER_ERROR,
                "token issuance failed".into(),
            )
        })?;
    let mut headers = HeaderMap::new();
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok((headers, Json(token)))
}

fn token_issuer(state: &AppState) -> Result<&TokenIssuer, ApiError> {
    state.tokens.as_ref().ok_or_else(|| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "Lore authentication is unavailable".into(),
        )
    })
}

pub(super) async fn user_access_token(
    state: &AppState,
    session: &AuthSession,
) -> Result<String, ApiError> {
    let resources =
        super::repository_access::resource_ids(&state.pool, &session.user.id.to_string()).await?;
    token_issuer(state)?
        .issue_user(&session.user, resources)
        .map(|token| token.access_token)
        .map_err(|error| {
            tracing::error!(%error, "Lore access token issuance failed");
            ApiError(
                StatusCode::INTERNAL_SERVER_ERROR,
                "token issuance failed".into(),
            )
        })
}

fn repository_creation_token(state: &AppState, session: &AuthSession) -> Result<String, ApiError> {
    token_issuer(state)?
        .issue_repository_creation(&session.user)
        .map(|token| token.access_token)
        .map_err(|error| {
            tracing::error!(%error, "Lore repository creation token issuance failed");
            ApiError(
                StatusCode::INTERNAL_SERVER_ERROR,
                "token issuance failed".into(),
            )
        })
}

#[derive(Serialize)]
struct RepositoryList {
    server_url: String,
    storage_backends: Vec<StorageBackend>,
    repositories: Vec<Repository>,
}

async fn list_repositories(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
) -> Result<Json<RepositoryList>, ApiError> {
    let access_token = user_access_token(&state, &session).await?;
    let repositories = state.repositories.list(&access_token).await?;
    Ok(Json(RepositoryList {
        server_url: state.repositories.public_server_url().to_owned(),
        storage_backends: state.repositories.available_storage_backends(),
        repositories,
    }))
}

#[derive(Deserialize)]
struct CreateRepository {
    name: String,
    description: Option<String>,
    #[serde(default)]
    storage_backend: StorageBackend,
}

async fn create_repository(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Json(input): Json<CreateRepository>,
) -> Result<(StatusCode, Json<Repository>), ApiError> {
    let access_token = repository_creation_token(&state, &session)?;
    let repository = state
        .repositories
        .create(
            &input.name,
            input.description.as_deref(),
            input.storage_backend,
            &access_token,
        )
        .await?;
    sqlx::query("UPDATE lore_resources SET storage_backend=$1 WHERE resource_id=$2 OR name=$3")
        .bind(input.storage_backend.as_str())
        .bind(&repository.id)
        .bind(&repository.name)
        .execute(&state.pool)
        .await?;
    Ok((StatusCode::CREATED, Json(repository)))
}

async fn delete_repository(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
) -> Result<StatusCode, ApiError> {
    let access_token = user_access_token(&state, &session).await?;
    let backend = state
        .repositories
        .storage_backend(&name, &access_token)
        .await?;
    state
        .repositories
        .delete_on(&name, backend, &access_token)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_repository_branches(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
) -> Result<Json<Vec<Branch>>, ApiError> {
    require_repository_access(&state.pool, &name, &session).await?;
    let access_token = user_access_token(&state, &session).await?;
    let backend = state
        .repositories
        .storage_backend(&name, &access_token)
        .await?;
    Ok(Json(
        state
            .repositories
            .branches_on(&name, backend, &access_token)
            .await?,
    ))
}

#[derive(Deserialize)]
struct RepositoryTreeQuery {
    revision: String,
    #[serde(default)]
    path: String,
}

async fn repository_tree(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Query(query): Query<RepositoryTreeQuery>,
) -> Result<Response, ApiError> {
    require_repository_access(&state.pool, &name, &session).await?;
    let token = user_access_token(&state, &session).await?;
    let backend = state.repositories.storage_backend(&name, &token).await?;
    let entries = state
        .repositories
        .tree_on(&name, &query.revision, &query.path, backend, &token)
        .await?;
    Ok((
        [(CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(entries),
    )
        .into_response())
}

#[derive(Deserialize)]
struct RepositoryLinksQuery {
    branch: String,
}

async fn repository_links(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Query(query): Query<RepositoryLinksQuery>,
) -> Result<Response, ApiError> {
    let resource_id = require_repository_access(&state.pool, &name, &session).await?;
    let access_token = user_access_token(&state, &session).await?;
    let backend = state
        .repositories
        .storage_backend(&name, &access_token)
        .await?;
    let links = state
        .repositories
        .links_on(&name, &query.branch, backend, &access_token)
        .await?;
    let mut headers = HeaderMap::new();
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    let links =
        super::links::describe(&state, &session, &resource_id, links, &access_token).await?;
    Ok((headers, Json(links)).into_response())
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub(super) struct AddRepositoryLink {
    pub branch: String,
    pub expected_revision: String,
    pub path: String,
    pub source_repository: String,
    pub source_path: String,
    pub source_branch: String,
    #[serde(default)]
    pub disable_branching: bool,
    #[serde(default = "super::links::enabled")]
    pub create_source_directory: bool,
    #[serde(default = "super::links::enabled")]
    pub auto_update: bool,
    #[serde(default = "Uuid::new_v4")]
    pub operation_id: Uuid,
}

#[derive(Deserialize)]
struct ChangeRepositoryLink {
    branch: String,
    expected_revision: String,
    path: String,
}

#[derive(Serialize)]
struct RepositoryLinkChange {
    revision: String,
}

#[derive(Serialize)]
pub(super) struct RepositoryLinkAddition {
    pub revision: String,
    pub source_path_created: bool,
}

pub(super) async fn add_repository_link(
    state: &AppState,
    session: &AuthSession,
    name: String,
    input: AddRepositoryLink,
    source_ready: bool,
    retry: bool,
) -> Result<RepositoryLinkAddition, ApiError> {
    require_repository_access(&state.pool, &name, session).await?;
    require_repository_access(&state.pool, &input.source_repository, session).await?;
    if name.eq_ignore_ascii_case(&input.source_repository) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "a repository cannot link to itself".into(),
        ));
    }
    let access_token = user_access_token(state, session).await?;
    let repositories = state.repositories.list(&access_token).await?;
    let root_repository = repositories
        .iter()
        .find(|repository| repository.name == name)
        .ok_or_else(|| {
            ApiError(
                StatusCode::NOT_FOUND,
                format!("repository '{name}' was not found"),
            )
        })?;
    let source_repository = repositories
        .iter()
        .find(|repository| repository.name == input.source_repository)
        .ok_or_else(|| {
            ApiError(
                StatusCode::NOT_FOUND,
                format!("repository '{}' was not found", input.source_repository),
            )
        })?;
    let root_backend = root_repository.storage_backend;
    let source_backend = source_repository.storage_backend;
    let root_resource_id = root_repository
        .id
        .strip_prefix("urc-")
        .unwrap_or(&root_repository.id);
    let root_resource_id = format!("urc-{}", root_resource_id.to_ascii_lowercase());
    let mut link_tx = state.pool.begin().await?;
    triggers::acquire_link_branch_lock(&mut link_tx, &root_resource_id, &input.branch).await?;
    let existing = state
        .repositories
        .links_on(&name, &input.branch, root_backend, &access_token)
        .await?;
    if let Some(link) = existing.links.iter().find(|link| link.path == input.path) {
        let source_branch = state
            .repositories
            .branches_on(&input.source_repository, source_backend, &access_token)
            .await?;
        if source_ready
            && link.source_repository_id.trim_start_matches("urc-")
                == source_repository.id.trim_start_matches("urc-")
            && link.source_path == input.source_path
            && source_branch.iter().any(|branch| {
                branch.name == input.source_branch && branch.id == link.source_branch_id
            })
        {
            super::links::save_policy(
                &mut link_tx,
                &root_resource_id,
                &input.branch,
                &input.path,
                input.auto_update,
            )
            .await?;
            link_tx.commit().await?;
            return Ok(RepositoryLinkAddition {
                revision: existing.revision,
                source_path_created: false,
            });
        }
        return Err(ApiError(
            StatusCode::CONFLICT,
            "target path already has a link; inspect it before retrying".into(),
        ));
    }
    if let Some(parent) = existing
        .links
        .iter()
        .find(|link| link_path_is_below(&input.path, &link.path))
    {
        return Err(ApiError(
            StatusCode::CONFLICT,
            format!(
                "link path '{}' is inside existing link '{}'",
                input.path, parent.path
            ),
        ));
    }
    if !retry && existing.revision != input.expected_revision {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "root branch changed; refresh before creating the link".into(),
        ));
    }
    super::links::stage(&state.pool, input.operation_id, "source", false, false).await?;
    let source_path_created = state
        .repositories
        .prepare_source_directory_on(
            &input.source_repository,
            &input.source_branch,
            &input.source_path,
            source_backend,
            &access_token,
            input.create_source_directory && !source_ready,
        )
        .await?;
    super::links::stage(
        &state.pool,
        input.operation_id,
        "root",
        true,
        source_path_created,
    )
    .await?;
    let source_branch = state
        .repositories
        .branches_on(&input.source_repository, source_backend, &access_token)
        .await?
        .into_iter()
        .find(|candidate| candidate.name == input.source_branch)
        .ok_or_else(|| {
            ApiError(
                StatusCode::BAD_REQUEST,
                format!("source branch '{}' was not found", input.source_branch),
            )
        })?;
    let source_url = state
        .repositories
        .command_repository_url_for(source_backend, &input.source_repository)?;
    // A display name such as `main` is not present in the detached root checkout and Lore
    // interprets it as a revision, producing `revision not found: main`. Pin the full source
    // revision instead. Lore still records the source branch ID discovered from the repository,
    // so automatic updates can continue following that branch.
    let mut expected_revision = existing.revision;
    let mut stale_revision_retries = 0;
    let revision = loop {
        match state
            .repositories
            .add_link_on(
                &name,
                &input.branch,
                &expected_revision,
                &input.path,
                &source_url,
                &input.source_path,
                &source_repository.id,
                &source_branch.id,
                &source_branch.revision,
                input.disable_branching,
                root_backend,
                &access_token,
            )
            .await
        {
            Ok(revision) => break revision,
            Err(error)
                if source_path_created
                    && stale_revision_retries < 2
                    && error.message.contains("has changed from revision") =>
            {
                expected_revision = state
                    .repositories
                    .branches_on(&name, root_backend, &access_token)
                    .await?
                    .into_iter()
                    .find(|candidate| candidate.name == input.branch)
                    .ok_or_else(|| {
                        ApiError(
                            StatusCode::BAD_REQUEST,
                            format!("branch '{}' was not found", input.branch),
                        )
                    })?
                    .revision;
                stale_revision_retries += 1;
            }
            Err(error) => {
                let message = if source_path_created {
                    format!(
                        "source folder '{}' was created and pushed, but root link creation failed: {}",
                        input.source_path, error.message
                    )
                } else {
                    error.message
                };
                return Err(RepositoryCommandError { message }.into());
            }
        }
    };
    super::links::save_policy(
        &mut link_tx,
        &root_resource_id,
        &input.branch,
        &input.path,
        input.auto_update,
    )
    .await?;
    link_tx.commit().await?;
    Ok(RepositoryLinkAddition {
        revision,
        source_path_created,
    })
}

async fn update_repository_link(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Json(input): Json<ChangeRepositoryLink>,
) -> Result<Json<RepositoryLinkChange>, ApiError> {
    let resource = require_repository_access(&state.pool, &name, &session).await?;
    let mut tx = state.pool.begin().await?;
    super::triggers::acquire_link_branch_lock(&mut tx, &resource, &input.branch).await?;
    let access_token = user_access_token(&state, &session).await?;
    let backend = state
        .repositories
        .storage_backend(&name, &access_token)
        .await?;
    let revision = state
        .repositories
        .update_link_on(
            &name,
            &input.branch,
            &input.expected_revision,
            &input.path,
            backend,
            &access_token,
        )
        .await;
    let revision = match revision {
        Ok(revision) => revision,
        Err(error) => {
            sqlx::query("INSERT INTO repository_link_policies(root_resource_id,root_branch,link_path,last_error) VALUES($1,$2,$3,$4) ON CONFLICT(root_resource_id,root_branch,link_path) DO UPDATE SET last_error=$4,updated_at=now()")
                .bind(&resource).bind(&input.branch).bind(&input.path).bind(&error.message).execute(&mut *tx).await?;
            tx.commit().await?;
            return Err(error.into());
        }
    };
    sqlx::query("INSERT INTO repository_link_policies(root_resource_id,root_branch,link_path,last_success_at) VALUES($1,$2,$3,now()) ON CONFLICT(root_resource_id,root_branch,link_path) DO UPDATE SET last_success_at=now(),last_error=NULL,updated_at=now()")
        .bind(&resource).bind(&input.branch).bind(&input.path).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(RepositoryLinkChange { revision }))
}

async fn remove_repository_link(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Json(input): Json<ChangeRepositoryLink>,
) -> Result<Json<RepositoryLinkChange>, ApiError> {
    let resource = require_repository_access(&state.pool, &name, &session).await?;
    let mut tx = state.pool.begin().await?;
    super::triggers::acquire_link_branch_lock(&mut tx, &resource, &input.branch).await?;
    let access_token = user_access_token(&state, &session).await?;
    let backend = state
        .repositories
        .storage_backend(&name, &access_token)
        .await?;
    let revision = state
        .repositories
        .remove_link_on(
            &name,
            &input.branch,
            &input.expected_revision,
            &input.path,
            backend,
            &access_token,
        )
        .await?;
    sqlx::query("DELETE FROM repository_link_policies WHERE root_resource_id=$1 AND root_branch=$2 AND link_path=$3")
        .bind(&resource).bind(&input.branch).bind(&input.path).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(RepositoryLinkChange { revision }))
}

#[derive(Deserialize)]
struct RepositoryCiConfigQuery {
    branch: String,
}

#[derive(Serialize)]
struct RepositoryCiConfig {
    branch: String,
    revision: Option<String>,
    content: Option<String>,
    configuration: Option<PipelineFile>,
    is_link_source: bool,
}

async fn repository_is_link_source(pool: &PgPool, resource: &str) -> Result<bool, sqlx::Error> {
    // Incoming references determine eligibility, regardless of branch, tracking,
    // auto-update policy, or the caller's access to the referencing repository.
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM repository_link_dependencies WHERE source_resource_id=$1 AND root_resource_id<>$1)")
        .bind(resource).fetch_one(pool).await
}

async fn repository_ci_config(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Query(query): Query<RepositoryCiConfigQuery>,
) -> Result<(HeaderMap, Json<RepositoryCiConfig>), ApiError> {
    let resource = require_repository_access(&state.pool, &name, &session).await?;
    let mut headers = HeaderMap::new();
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if repository_is_link_source(&state.pool, &resource).await? {
        return Ok((
            headers,
            Json(RepositoryCiConfig {
                branch: query.branch,
                revision: None,
                content: None,
                configuration: None,
                is_link_source: true,
            }),
        ));
    }
    let access_token = user_access_token(&state, &session).await?;
    let backend = state
        .repositories
        .storage_backend(&name, &access_token)
        .await?;
    let branch = state
        .repositories
        .branches_on(&name, backend, &access_token)
        .await?
        .into_iter()
        .find(|branch| branch.name == query.branch)
        .ok_or_else(|| {
            ApiError(
                StatusCode::NOT_FOUND,
                format!("branch '{}' was not found", query.branch),
            )
        })?;
    let content = state
        .repositories
        .pipeline_source_on(&name, &branch.revision, backend, &access_token)
        .await?;
    let configuration = content
        .as_deref()
        .and_then(|source| PipelineFile::parse(source).ok());
    Ok((
        headers,
        Json(RepositoryCiConfig {
            branch: branch.name,
            revision: Some(branch.revision),
            content,
            configuration,
            is_link_source: false,
        }),
    ))
}

#[derive(Deserialize)]
struct UpdateRepositoryCiConfig {
    branch: String,
    expected_revision: String,
    content: String,
}

async fn update_repository_ci_config(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Json(input): Json<UpdateRepositoryCiConfig>,
) -> Result<Json<RepositoryCiConfig>, ApiError> {
    let resource = require_repository_access(&state.pool, &name, &session).await?;
    if repository_is_link_source(&state.pool, &resource).await? {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "CI settings are unavailable for Lore link source repositories.".into(),
        ));
    }
    let configuration = PipelineFile::parse(&input.content).map_err(|error| {
        ApiError(
            StatusCode::BAD_REQUEST,
            format!("invalid .lore-ci.toml: {error}"),
        )
    })?;
    let access_token = user_access_token(&state, &session).await?;
    let backend = state
        .repositories
        .storage_backend(&name, &access_token)
        .await?;
    let revision = state
        .repositories
        .update_pipeline_source_on(
            &name,
            &input.branch,
            &input.expected_revision,
            &input.content,
            backend,
            &access_token,
        )
        .await?;
    Ok(Json(RepositoryCiConfig {
        branch: input.branch,
        revision: Some(revision),
        content: Some(input.content),
        configuration: Some(configuration),
        is_link_source: false,
    }))
}

#[derive(Deserialize)]
struct ParseRepositoryCiConfig {
    content: String,
}

#[derive(Deserialize)]
struct AnalyzeRepositoryCiConfig {
    content: String,
    #[serde(default)]
    changed_paths: Vec<String>,
}

async fn analyze_repository_ci_config(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Json(input): Json<AnalyzeRepositoryCiConfig>,
) -> Result<Json<crate::ci::analysis::CiAnalysis>, ApiError> {
    require_repository_access(&state.pool, &name, &session).await?;
    crate::ci::analysis::analyze(&input.content, &input.changed_paths)
        .map(Json)
        .map_err(|error| ApiError(StatusCode::BAD_REQUEST, error.to_string()))
}

async fn parse_repository_ci_config(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Json(input): Json<ParseRepositoryCiConfig>,
) -> Result<Json<PipelineFile>, ApiError> {
    require_repository_access(&state.pool, &name, &session).await?;
    PipelineFile::parse(&input.content)
        .map(Json)
        .map_err(|error| {
            ApiError(
                StatusCode::BAD_REQUEST,
                format!("invalid .lore-ci.toml: {error}"),
            )
        })
}

#[derive(Deserialize)]
struct PipelineRevision {
    revision: String,
}

#[derive(Serialize, sqlx::FromRow)]
struct PipelineChoice {
    name: Option<String>,
    category: Option<String>,
    runner_os: Option<String>,
}

async fn list_repository_pipelines(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Query(query): Query<PipelineRevision>,
) -> Result<Json<Vec<PipelineChoice>>, ApiError> {
    if !triggers::valid_hash(&query.revision) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "revision must be a full 64-character Lore revision hash".into(),
        ));
    }
    let resource_id = require_repository_access(&state.pool, &name, &session).await?;
    let routes: Vec<PipelineChoice> = sqlx::query_as(
        "SELECT pipeline_name AS name, category, runner_os FROM ci_pipeline_routes WHERE resource_id = $1 AND revision = $2 ORDER BY category, pipeline_name",
    )
    .bind(&resource_id)
    .bind(&query.revision)
    .fetch_all(&state.pool)
    .await?;
    if !routes.is_empty() {
        return Ok(Json(routes));
    }
    let access_token = user_access_token(&state, &session).await?;
    let backend = state
        .repositories
        .storage_backend(&name, &access_token)
        .await?;
    let config = state
        .repositories
        .pipeline_file_on(&name, &query.revision, backend, &access_token)
        .await?
        .ok_or_else(|| {
            ApiError(
                StatusCode::NOT_FOUND,
                ".lore-ci.toml not found at this revision".into(),
            )
        })?;
    if config.pipelines.is_empty() {
        config
            .select(None)
            .map_err(|error| ApiError(StatusCode::BAD_REQUEST, error.to_string()))?;
        return Ok(Json(vec![PipelineChoice {
            name: None,
            category: None,
            runner_os: None,
        }]));
    }
    Ok(Json(
        config
            .pipelines
            .iter()
            .map(|pipeline| PipelineChoice {
                name: Some(pipeline.name.clone()),
                category: Some(pipeline.category.clone()),
                runner_os: Some(pipeline.runner_os.clone()),
            })
            .collect(),
    ))
}

#[derive(Serialize)]
struct RepositoryPipelineBranches {
    branches: Vec<Branch>,
    selected: Vec<String>,
}

fn canonical_branch_names(values: Vec<String>, branches: &[Branch]) -> Vec<String> {
    let mut names = values
        .into_iter()
        .map(|value| {
            if let Some(branch) = branches
                .iter()
                .find(|branch| branch.name == value || branch.id == value)
            {
                branch.name.clone()
            } else {
                value
            }
        })
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

async fn repository_pipeline_branches(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
) -> Result<Json<RepositoryPipelineBranches>, ApiError> {
    let resource_id = require_repository_access(&state.pool, &name, &session).await?;
    let access_token = user_access_token(&state, &session).await?;
    let backend = state
        .repositories
        .storage_backend(&name, &access_token)
        .await?;
    let branches = state
        .repositories
        .branches_on(&name, backend, &access_token)
        .await?;
    let stored: Option<Vec<String>> = sqlx::query_scalar(
        "SELECT branches FROM ci_repository_pipeline_branches WHERE resource_id=$1",
    )
    .bind(&resource_id)
    .fetch_optional(&state.pool)
    .await?;
    let configured = stored.clone().unwrap_or_else(|| vec!["main".into()]);
    let selected = canonical_branch_names(configured, &branches);
    if stored.as_ref().is_some_and(|stored| stored != &selected) {
        sqlx::query("UPDATE ci_repository_pipeline_branches SET branches=$2,updated_at=now() WHERE resource_id=$1")
            .bind(&resource_id)
            .bind(&selected)
            .execute(&state.pool)
            .await?;
    }
    Ok(Json(RepositoryPipelineBranches { branches, selected }))
}

#[derive(Deserialize)]
struct RepositoryPipelineBranchesInput {
    branches: Vec<String>,
}

async fn update_repository_pipeline_branches(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Json(mut input): Json<RepositoryPipelineBranchesInput>,
) -> Result<StatusCode, ApiError> {
    let resource_id = require_repository_access(&state.pool, &name, &session).await?;
    input.branches.sort();
    input.branches.dedup();
    if input.branches.len() > 200
        || input.branches.iter().any(|branch| {
            branch.is_empty() || branch.len() > 255 || branch.chars().any(char::is_control)
        })
    {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Select at most 200 valid branches.".into(),
        ));
    }
    let access_token = user_access_token(&state, &session).await?;
    let backend = state
        .repositories
        .storage_backend(&name, &access_token)
        .await?;
    let remote_branches = state
        .repositories
        .branches_on(&name, backend, &access_token)
        .await?;
    input.branches = canonical_branch_names(input.branches, &remote_branches);
    if input.branches.iter().any(|selected| {
        !remote_branches
            .iter()
            .any(|branch| &branch.name == selected)
    }) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "One or more selected branches no longer exist.".into(),
        ));
    }
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(&resource_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO ci_repository_pipeline_branches(resource_id,branches) VALUES($1,$2) ON CONFLICT(resource_id) DO UPDATE SET branches=EXCLUDED.branches,updated_at=now()")
        .bind(&resource_id).bind(&input.branches).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM ci_pipeline_routes WHERE resource_id=$1 AND NOT(branch=ANY($2))")
        .bind(&resource_id)
        .bind(&input.branches)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "DELETE FROM ci_pipeline_route_snapshots WHERE resource_id=$1 AND NOT(branch=ANY($2))",
    )
    .bind(&resource_id)
    .bind(&input.branches)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM ci_branch_cursors WHERE resource_id=$1 AND NOT(branch=ANY($2))")
        .bind(&resource_id)
        .bind(&input.branches)
        .execute(&mut *tx)
        .await?;
    for branch in remote_branches
        .iter()
        .filter(|branch| input.branches.contains(&branch.name))
    {
        sqlx::query("INSERT INTO ci_branch_cursors(resource_id,branch,revision) VALUES($1,$2,$3) ON CONFLICT(resource_id,branch) DO NOTHING")
            .bind(&resource_id).bind(&branch.name).bind(&branch.revision).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn require_repository_access(
    pool: &PgPool,
    repository_name: &str,
    session: &AuthSession,
) -> Result<String, ApiError> {
    let resource_id: Option<String> =
        super::repository_access::by_name(pool, &session.user.id.to_string(), repository_name)
            .await?;
    if let Some(resource_id) = resource_id {
        Ok(resource_id)
    } else {
        Err(ApiError(
            StatusCode::FORBIDDEN,
            "repository owner or administrator access required".into(),
        ))
    }
}

async fn submit(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Json(mut input): Json<SubmitPipeline>,
) -> Result<(StatusCode, Json<Pipeline>), ApiError> {
    input
        .validate()
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
    let repository_name = url::Url::parse(&input.repository_url)
        .ok()
        .and_then(|url| url.path_segments()?.next_back().map(str::to_owned))
        .ok_or_else(|| ApiError(StatusCode::BAD_REQUEST, "invalid repository URL".into()))?;
    let storage_backend = state
        .repositories
        .storage_backend_for_public_url(&repository_name, &input.repository_url)
        .ok_or_else(|| {
            ApiError(
                StatusCode::BAD_REQUEST,
                "repository_url must match a configured Lore server".into(),
            )
        })?;
    let expected_repository_url = state
        .repositories
        .public_repository_url_for(storage_backend, &repository_name)?;
    let resource_id = require_repository_access(&state.pool, &repository_name, &session).await?;
    let needs_access_token = input.branch.is_some() || input.pipeline_name.is_some();
    let access_token = if needs_access_token {
        Some(user_access_token(&state, &session).await?)
    } else {
        None
    };
    if let Some(branch) = input.branch.as_deref() {
        let branches = state
            .repositories
            .branches_on(
                &repository_name,
                storage_backend,
                access_token.as_deref().unwrap_or_default(),
            )
            .await?;
        let selected = branches
            .iter()
            .find(|candidate| candidate.name == branch)
            .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "branch not found".into()))?;
        if selected.revision != input.revision.to_ascii_lowercase() {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "branch head changed; select the branch again".into(),
            ));
        }
    }
    input.repository_url = expected_repository_url;
    if let Some(pipeline_name) = input.pipeline_name.as_deref() {
        let config = state
            .repositories
            .pipeline_file_on(
                &repository_name,
                &input.revision,
                storage_backend,
                access_token.as_deref().unwrap_or_default(),
            )
            .await?
            .ok_or_else(|| {
                ApiError(
                    StatusCode::NOT_FOUND,
                    ".lore-ci.toml not found at this revision".into(),
                )
            })?;
        let pipeline = config
            .pipelines
            .iter()
            .find(|pipeline| pipeline.name == pipeline_name)
            .ok_or_else(|| {
                ApiError(
                    StatusCode::BAD_REQUEST,
                    "selected pipeline is missing from this revision".into(),
                )
            })?;
        let mut tx = state.pool.begin().await?;
        let (sparse_view_name, sparse_view_rules) =
            triggers::sparse_view_snapshot(&mut tx, &resource_id, pipeline.sparse_view.as_deref())
                .await
                .map_err(|error| ApiError(StatusCode::BAD_REQUEST, error.to_string()))?;
        let selected = SelectedPipeline {
            pipeline_name: pipeline.name.clone(),
            pipeline_needs: pipeline.needs.clone(),
            category: pipeline.category.clone(),
            runner_os: pipeline.runner_os.clone(),
            trigger_patterns: pipeline.changes.clone(),
            working_directory: pipeline.working_directory.clone(),
            graph_definition: triggers::pipeline_graph_definition(pipeline)
                .map_err(|error| ApiError(StatusCode::BAD_REQUEST, error.to_string()))?,
            sparse_view_name,
            sparse_view_rules,
        };
        let pipeline =
            db::submit_selected_for_user(&mut tx, &input, session.user.id, &selected).await?;
        tx.commit().await?;
        return Ok((StatusCode::CREATED, Json(pipeline)));
    }
    Ok((
        StatusCode::CREATED,
        Json(db::submit_for_user(&state.pool, &input, session.user.id).await?),
    ))
}

#[derive(Deserialize)]
struct LogPage {
    #[serde(default)]
    after: i64,
    #[serde(default = "page_size")]
    limit: i64,
    job_id: Option<Uuid>,
}
fn page_size() -> i64 {
    100
}

#[derive(Deserialize)]
struct PipelinePage {
    before: Option<Uuid>,
    repository_url: Option<String>,
    branch: Option<String>,
    pipeline_name: Option<String>,
    status: Option<String>,
    q: Option<String>,
    #[serde(default = "page_size")]
    limit: i64,
}

#[derive(Serialize)]
struct PipelinePageResponse {
    pipelines: Vec<Pipeline>,
    next_before: Option<Uuid>,
}

async fn list(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
) -> Result<Json<Vec<Pipeline>>, ApiError> {
    let page = pipeline_page(
        &state,
        &session,
        PipelinePage {
            before: None,
            repository_url: None,
            branch: None,
            pipeline_name: None,
            status: None,
            q: None,
            limit: page_size(),
        },
    )
    .await?;
    Ok(Json(page.pipelines))
}

async fn pipeline_history(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Query(page): Query<PipelinePage>,
) -> Result<Json<PipelinePageResponse>, ApiError> {
    Ok(Json(pipeline_page(&state, &session, page).await?))
}

async fn pipeline_page(
    state: &AppState,
    session: &AuthSession,
    page: PipelinePage,
) -> Result<PipelinePageResponse, ApiError> {
    validate_repository_filter(page.repository_url.as_deref())?;
    for (name, value, limit) in [
        ("branch", page.branch.as_deref(), 512),
        ("pipeline_name", page.pipeline_name.as_deref(), 512),
        ("q", page.q.as_deref(), 256),
    ] {
        if value.is_some_and(|value| {
            value.chars().count() > limit || value.chars().any(char::is_control)
        }) {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                format!(
                    "{name} must contain at most {limit} characters without control characters"
                ),
            ));
        }
    }
    if page.status.as_deref().is_some_and(|status| {
        !matches!(
            status,
            "" | "all"
                | "active"
                | "finished"
                | "queued"
                | "running"
                | "succeeded"
                | "failed"
                | "canceled"
        )
    }) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "invalid history status".into(),
        ));
    }
    if !(1..=500).contains(&page.limit) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "limit must be 1..500".into(),
        ));
    }
    let access = PipelineAccess::new(&state.repositories, session.user.id);
    let mut tx = state.pool.begin().await?;
    sqlx::query("SET LOCAL statement_timeout = '5s'")
        .execute(&mut *tx)
        .await?;
    let cursor_created_at = match page.before {
        Some(id) => {
            access
                .query_as::<(DateTime<Utc>,)>(&PipelineAccess::sql(
                    "SELECT created_at FROM accessible_pipelines WHERE id = $4 AND ($5::text IS NULL OR repository_url = $5)",
                ))
                .bind(id)
                .bind(&page.repository_url)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| ApiError(StatusCode::BAD_REQUEST, "unknown pipeline cursor".into()))?
                .0
        }
        None => Utc::now(),
    };
    // Resolve legacy branch IDs before filtering, with the same unambiguous
    // revision-first fallback as execution detail. Search terms are literal text.
    let mut rows: Vec<HistoryPipeline> = access.query_as(&PipelineAccess::sql(
        ", history_pipelines AS (
            SELECT p.*, CASE WHEN p.branch ~ '^[0-9a-fA-F]{32}$' THEN COALESCE(
                (SELECT CASE WHEN count(DISTINCT r.branch) = 1 THEN min(r.branch) END
                 FROM ci_pipeline_routes r WHERE r.repository_url = p.repository_url
                   AND r.pipeline_name = p.pipeline_name AND r.revision = p.revision),
                (SELECT CASE WHEN count(DISTINCT r.branch) = 1 THEN min(r.branch) END
                 FROM ci_pipeline_routes r WHERE r.repository_url = p.repository_url
                   AND r.pipeline_name = p.pipeline_name), p.branch) ELSE p.branch END AS history_branch
            FROM accessible_pipelines p
            WHERE ($4::uuid IS NULL OR (p.created_at, p.id) < ($5, $4))
              AND ($7::text IS NULL OR p.repository_url = $7)
              AND (NULLIF($9::text, '') IS NULL OR p.pipeline_name = $9)
              AND (COALESCE($10::text, '') IN ('', 'all') OR p.status = $10
                   OR ($10 = 'active' AND p.status IN ('queued', 'running'))
                   OR ($10 = 'finished' AND p.status IN ('succeeded', 'failed', 'canceled')))
        ) SELECT * FROM history_pipelines p
          WHERE (NULLIF($8::text, '') IS NULL OR history_branch = $8)
            AND (NULLIF($11::text, '') IS NULL OR EXISTS (
                SELECT 1 FROM unnest(ARRAY[p.id::text, p.repository_url, p.history_branch,
                    p.revision, p.status, p.pipeline_name, p.runner_os, p.sparse_view_name]) value
                WHERE strpos(lower(value), lower($11)) > 0))
          ORDER BY created_at DESC, id DESC LIMIT $6",
    ))
    .bind(page.before)
    .bind(cursor_created_at)
    .bind(page.limit + 1)
    .bind(&page.repository_url)
    .bind(&page.branch)
    .bind(&page.pipeline_name)
    .bind(&page.status)
    .bind(&page.q)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let next_before = if rows.len() > page.limit as usize {
        rows.truncate(page.limit as usize);
        rows.last().map(|row| row.pipeline.id)
    } else {
        None
    };
    let pipelines = rows
        .into_iter()
        .map(|row| Pipeline {
            branch: row.history_branch,
            ..row.pipeline
        })
        .collect();
    Ok(PipelinePageResponse {
        pipelines,
        next_before,
    })
}

#[derive(sqlx::FromRow)]
struct HistoryPipeline {
    #[sqlx(flatten)]
    pipeline: Pipeline,
    history_branch: Option<String>,
}

#[derive(sqlx::FromRow)]
struct PipelineRouteBranch {
    repository_url: String,
    branch: String,
    revision: String,
    pipeline_name: String,
}

async fn normalize_legacy_pipeline_branches(
    pool: &PgPool,
    pipelines: &mut [Pipeline],
) -> Result<(), sqlx::Error> {
    let routes: Vec<PipelineRouteBranch> = sqlx::query_as(
        "SELECT repository_url,branch,revision,pipeline_name FROM ci_pipeline_routes",
    )
    .fetch_all(pool)
    .await?;
    for pipeline in pipelines {
        let Some(branch) = pipeline.branch.as_deref() else {
            continue;
        };
        if !is_legacy_branch_id(branch) {
            continue;
        }
        if let Some(name) = inferred_route_branch(pipeline, &routes) {
            pipeline.branch = Some(name.to_owned());
        }
    }
    Ok(())
}

fn is_legacy_branch_id(branch: &str) -> bool {
    branch.len() == 32 && branch.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn inferred_route_branch<'a>(
    pipeline: &Pipeline,
    routes: &'a [PipelineRouteBranch],
) -> Option<&'a str> {
    let same_repository = |route: &&PipelineRouteBranch| {
        route.repository_url == pipeline.repository_url
            && Some(route.pipeline_name.as_str()) == pipeline.pipeline_name.as_deref()
    };
    unique_route_branch(
        routes
            .iter()
            .filter(same_repository)
            .filter(|route| route.revision == pipeline.revision),
    )
    .or_else(|| unique_route_branch(routes.iter().filter(same_repository)))
}

fn unique_route_branch<'a>(
    routes: impl Iterator<Item = &'a PipelineRouteBranch>,
) -> Option<&'a str> {
    let mut branch = None;
    for route in routes {
        match branch {
            None => branch = Some(route.branch.as_str()),
            Some(current) if current == route.branch => {}
            Some(_) => return None,
        }
    }
    branch
}

async fn list_runners(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
) -> Result<Json<Vec<Runner>>, ApiError> {
    let mut runners = db::list_runners(&state.pool).await?;
    let active: Vec<_> = runners
        .iter()
        .filter_map(|runner| runner.current_pipeline_id)
        .collect();
    if !active.is_empty() {
        let access = PipelineAccess::new(&state.repositories, session.user.id);
        let visible: Vec<(Uuid,)> = access
            .query_as(&PipelineAccess::sql(
                "SELECT id FROM accessible_pipelines WHERE id = ANY($4)",
            ))
            .bind(active)
            .fetch_all(&state.pool)
            .await?;
        let visible: std::collections::HashSet<_> = visible.into_iter().map(|(id,)| id).collect();
        for runner in &mut runners {
            runner.current_pipeline_id =
                runner.current_pipeline_id.filter(|id| visible.contains(id));
        }
    }
    Ok(Json(runners))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RunnerDrain {
    draining: bool,
}

async fn set_runner_draining(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<RunnerDrain>,
) -> Result<StatusCode, ApiError> {
    require_admin(&session)?;
    if !db::set_runner_draining(&state.pool, id, input.draining).await? {
        return Err(ApiError(StatusCode::NOT_FOUND, "runner not found".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_runner(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_admin(&session)?;
    match db::remove_runner(&state.pool, id).await? {
        db::RemoveRunner::Removed => Ok(StatusCode::NO_CONTENT),
        db::RemoveRunner::NotFound => {
            Err(ApiError(StatusCode::NOT_FOUND, "runner not found".into()))
        }
        db::RemoveRunner::Online => Err(ApiError(
            StatusCode::CONFLICT,
            "stop the runner before removing it".into(),
        )),
        db::RemoveRunner::Busy => Err(ApiError(
            StatusCode::CONFLICT,
            "runner has an active pipeline".into(),
        )),
    }
}

#[derive(sqlx::FromRow)]
struct PipelineGraphRow {
    repository_url: String,
    branch: String,
    revision: String,
    revision_number: i64,
    pipeline_name: String,
    category: String,
    runner_os: String,
    trigger_patterns: Vec<String>,
    working_directory: String,
    graph_definition: String,
    updated_at: DateTime<Utc>,
    latest_pipeline_id: Option<Uuid>,
    latest_status: Option<String>,
    latest_created_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct PipelineGraphRoute {
    repository_url: String,
    branch: String,
    revision: String,
    revision_number: i64,
    pipeline_name: String,
    category: String,
    runner_os: String,
    trigger_patterns: Vec<String>,
    working_directory: String,
    graph: serde_json::Value,
    updated_at: DateTime<Utc>,
    latest_pipeline_id: Option<Uuid>,
    latest_status: Option<String>,
    latest_created_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
struct RepositoryFilter {
    repository_url: Option<String>,
}

fn validate_repository_filter(url: Option<&str>) -> Result<(), ApiError> {
    if url.is_some_and(|value| value.is_empty() || value.len() > 2048) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "repository_url must be 1..2048 bytes".into(),
        ));
    }
    Ok(())
}

async fn pipeline_graphs(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Query(filter): Query<RepositoryFilter>,
) -> Result<Json<Vec<PipelineGraphRoute>>, ApiError> {
    validate_repository_filter(filter.repository_url.as_deref())?;
    let access = PipelineAccess::new(&state.repositories, session.user.id);
    let rows: Vec<PipelineGraphRow> = access.query_as(&PipelineAccess::sql(
        "SELECT route.repository_url, route.branch, route.revision, route.revision_number, route.pipeline_name, route.category, route.runner_os, route.trigger_patterns, route.working_directory, route.graph_definition, route.updated_at, latest.id AS latest_pipeline_id, latest.status AS latest_status, latest.created_at AS latest_created_at FROM ci_pipeline_routes route JOIN accessible_repositories repository ON repository.resource_id = route.resource_id AND repository.repository_url = route.repository_url LEFT JOIN LATERAL (SELECT id, status, created_at FROM accessible_pipelines WHERE repository_url = route.repository_url AND pipeline_name = route.pipeline_name AND (branch = route.branch OR (branch ~ '^[0-9a-fA-F]{32}$' AND revision = route.revision)) ORDER BY created_at DESC, id DESC LIMIT 1) latest ON true WHERE ($4::text IS NULL OR route.repository_url=$4) ORDER BY route.repository_url, route.branch, route.category, route.pipeline_name",
    ))
    .bind(&filter.repository_url)
    .fetch_all(&state.pool)
    .await?;
    let routes = rows
        .into_iter()
        .map(|row| {
            let graph = serde_json::from_str(&row.graph_definition).map_err(|error| {
                tracing::error!(pipeline = %row.pipeline_name, %error, "stored route graph is invalid");
                ApiError(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "pipeline graph is unavailable".into(),
                )
            })?;
            Ok(PipelineGraphRoute {
                repository_url: row.repository_url,
                branch: row.branch,
                revision: row.revision,
                revision_number: row.revision_number,
                pipeline_name: row.pipeline_name,
                category: row.category,
                runner_os: row.runner_os,
                trigger_patterns: row.trigger_patterns,
                working_directory: row.working_directory,
                graph,
                updated_at: row.updated_at,
                latest_pipeline_id: row.latest_pipeline_id,
                latest_status: row.latest_status,
                latest_created_at: row.latest_created_at,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(Json(routes))
}

#[derive(Serialize)]
struct Detail {
    pipeline: Pipeline,
    jobs: Vec<Job>,
    graph: Option<serde_json::Value>,
    sparse_view_rules: Option<String>,
    queue_reason: Option<&'static str>,
}
async fn detail(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(id): Path<Uuid>,
) -> Result<Json<Detail>, ApiError> {
    let access = PipelineAccess::new(&state.repositories, session.user.id);
    let mut pipeline = access.pipeline(&state.pool, id).await?;
    let execution_branch = pipeline.branch.clone();
    normalize_legacy_pipeline_branches(&state.pool, std::slice::from_mut(&mut pipeline)).await?;
    let jobs = sqlx::query_as("SELECT * FROM jobs WHERE pipeline_id = $1 ORDER BY position")
        .bind(id)
        .fetch_all(&state.pool)
        .await?;
    let graph = pipeline
        .graph_definition
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|error| {
            tracing::error!(pipeline_id = %pipeline.id, %error, "stored pipeline graph is invalid");
            ApiError(
                StatusCode::INTERNAL_SERVER_ERROR,
                "pipeline graph is unavailable".into(),
            )
        })?;
    let sparse_view_rules = pipeline.sparse_view_rules.clone();
    // Match claim(): only existing upstream runs in the same branch/revision
    // can block a claim. Missing upstream runs are not assumed to be blockers.
    let queue_reason = if pipeline.status != "queued" {
        None
    } else if pipeline.cancel_requested {
        Some("canceling")
    } else {
        let (blocked,): (bool,) = access.query_as(&PipelineAccess::sql(
            "SELECT EXISTS(SELECT 1 FROM unnest($4::text[]) AS dependency(name) JOIN LATERAL (SELECT upstream.status FROM accessible_pipelines upstream WHERE upstream.repository_url = $5 AND upstream.branch IS NOT DISTINCT FROM $6 AND upstream.revision = $7 AND upstream.pipeline_name = dependency.name ORDER BY upstream.created_at DESC, upstream.id DESC LIMIT 1) latest ON true WHERE latest.status <> 'succeeded')",
        ))
        .bind(&pipeline.pipeline_needs)
        .bind(&pipeline.repository_url)
        .bind(&execution_branch)
        .bind(&pipeline.revision)
        .fetch_one(&state.pool)
        .await?;
        Some(if blocked {
            "dependencies"
        } else if pipeline.worker_id.is_none() {
            "runner"
        } else {
            "unknown"
        })
    };
    Ok(Json(Detail {
        pipeline,
        jobs,
        graph,
        sparse_view_rules,
        queue_reason,
    }))
}

async fn cancel(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(id): Path<Uuid>,
) -> Result<Json<Pipeline>, ApiError> {
    let pipeline = PipelineAccess::new(&state.repositories, session.user.id)
        .pipeline(&state.pool, id)
        .await?;
    let submitted_by = pipeline.submitted_by;
    if session.user.role != "admin" && submitted_by != Some(session.user.id) {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "only the pipeline submitter or an administrator can cancel it".into(),
        ));
    }
    Ok(Json(db::cancel(&state.pool, id).await?.ok_or_else(
        || ApiError(StatusCode::NOT_FOUND, "pipeline not found".into()),
    )?))
}

fn require_admin(session: &AuthSession) -> Result<(), ApiError> {
    if session.user.role == "admin" {
        Ok(())
    } else {
        Err(ApiError(
            StatusCode::FORBIDDEN,
            "administrator access required".into(),
        ))
    }
}

async fn logs(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(id): Path<Uuid>,
    Query(page): Query<LogPage>,
) -> Result<Json<Vec<Log>>, ApiError> {
    if page.after < 0 || !(1..=500).contains(&page.limit) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "after must be >= 0; limit must be 1..500".into(),
        ));
    }
    PipelineAccess::new(&state.repositories, session.user.id)
        .pipeline(&state.pool, id)
        .await?;
    Ok(Json(
        sqlx::query_as(
            "SELECT * FROM logs WHERE pipeline_id = $1 AND id > $2 AND ($4::uuid IS NULL OR job_id = $4) ORDER BY id LIMIT $3",
        )
        .bind(id)
        .bind(page.after)
        .bind(page.limit)
        .bind(page.job_id)
        .fetch_all(&state.pool)
        .await?,
    ))
}
