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
    releases::RunnerReleases,
    repositories::{
        Branch, CommandError as RepositoryCommandError, Repository, RepositoryService,
        StorageBackend,
    },
    tokens::{IssuedToken, TokenIssuer},
    triggers, web,
};
use crate::ci::{
    config::{PipelineConfig, SubmitPipeline},
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
        .route("/api/v1/pipelines", post(submit).get(list))
        .route("/api/v1/pipeline-history", get(pipeline_history))
        .route("/api/v1/pipeline-graphs", get(pipeline_graphs))
        .route("/api/v1/pipelines/{id}", get(detail))
        .route("/api/v1/pipelines/{id}/cancel", post(cancel))
        .route("/api/v1/pipelines/{id}/logs", get(logs))
        .route("/api/v1/runners", get(list_runners))
        .route("/api/v1/runners/{id}", delete(remove_runner))
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
        .route("/assets/theme.js", get(web::theme_script))
        .route("/assets/app.js", get(web::script))
        .route("/assets/management.js", get(web::management_script))
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
    let Some(pipeline) = db::claim(&state.pool, worker).await? else {
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
        let status = if lower.contains("already exists") {
            StatusCode::CONFLICT
        } else if lower.contains("not found") {
            StatusCode::NOT_FOUND
        } else if lower.contains("repository name") || lower.contains("description") {
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

async fn user_access_token(state: &AppState, session: &AuthSession) -> Result<String, ApiError> {
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

async fn require_repository_access(
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
}
fn page_size() -> i64 {
    100
}

#[derive(Deserialize)]
struct PipelinePage {
    before: Option<Uuid>,
    #[serde(default = "page_size")]
    limit: i64,
}

#[derive(Serialize)]
struct PipelinePageResponse {
    pipelines: Vec<Pipeline>,
    next_before: Option<Uuid>,
}

async fn list(State(state): State<AppState>) -> Result<Json<Vec<Pipeline>>, ApiError> {
    let page = pipeline_page(
        &state,
        PipelinePage {
            before: None,
            limit: page_size(),
        },
    )
    .await?;
    Ok(Json(page.pipelines))
}

async fn pipeline_history(
    State(state): State<AppState>,
    Query(page): Query<PipelinePage>,
) -> Result<Json<PipelinePageResponse>, ApiError> {
    Ok(Json(pipeline_page(&state, page).await?))
}

async fn pipeline_page(
    state: &AppState,
    page: PipelinePage,
) -> Result<PipelinePageResponse, ApiError> {
    if !(1..=500).contains(&page.limit) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "limit must be 1..500".into(),
        ));
    }
    let cursor_created_at = match page.before {
        Some(id) => sqlx::query_scalar("SELECT created_at FROM pipelines WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiError(StatusCode::BAD_REQUEST, "unknown pipeline cursor".into()))?,
        None => Utc::now(),
    };
    let mut pipelines: Vec<Pipeline> = sqlx::query_as(
        "SELECT * FROM pipelines WHERE ($1::uuid IS NULL OR (created_at, id) < ($2, $1)) ORDER BY created_at DESC, id DESC LIMIT $3",
    )
    .bind(page.before)
    .bind(cursor_created_at)
    .bind(page.limit + 1)
    .fetch_all(&state.pool)
    .await?;
    let next_before = if pipelines.len() > page.limit as usize {
        pipelines.truncate(page.limit as usize);
        pipelines.last().map(|pipeline| pipeline.id)
    } else {
        None
    };
    normalize_legacy_pipeline_branches(&state.pool, &mut pipelines).await?;
    Ok(PipelinePageResponse {
        pipelines,
        next_before,
    })
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

async fn list_runners(State(state): State<AppState>) -> Result<Json<Vec<Runner>>, ApiError> {
    Ok(Json(db::list_runners(&state.pool).await?))
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

async fn pipeline_graphs(
    State(state): State<AppState>,
) -> Result<Json<Vec<PipelineGraphRoute>>, ApiError> {
    let rows: Vec<PipelineGraphRow> = sqlx::query_as(
        "SELECT route.repository_url, route.branch, route.revision, route.revision_number, route.pipeline_name, route.category, route.runner_os, route.trigger_patterns, route.working_directory, route.graph_definition, route.updated_at, latest.id AS latest_pipeline_id, latest.status AS latest_status, latest.created_at AS latest_created_at FROM ci_pipeline_routes route LEFT JOIN LATERAL (SELECT id, status, created_at FROM pipelines WHERE repository_url = route.repository_url AND pipeline_name = route.pipeline_name AND (branch = route.branch OR (branch ~ '^[0-9a-fA-F]{32}$' AND revision = route.revision)) ORDER BY created_at DESC, id DESC LIMIT 1) latest ON true ORDER BY route.repository_url, route.branch, route.category, route.pipeline_name",
    )
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
}
async fn detail(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Detail>, ApiError> {
    let mut pipeline: Pipeline = sqlx::query_as("SELECT * FROM pipelines WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "pipeline not found".into()))?;
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
    Ok(Json(Detail {
        pipeline,
        jobs,
        graph,
        sparse_view_rules,
    }))
}

async fn cancel(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(id): Path<Uuid>,
) -> Result<Json<Pipeline>, ApiError> {
    let submitted_by: Option<Option<Uuid>> =
        sqlx::query_scalar("SELECT submitted_by FROM pipelines WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
    let submitted_by =
        submitted_by.ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "pipeline not found".into()))?;
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
    Path(id): Path<Uuid>,
    Query(page): Query<LogPage>,
) -> Result<Json<Vec<Log>>, ApiError> {
    if page.after < 0 || !(1..=500).contains(&page.limit) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "after must be >= 0; limit must be 1..500".into(),
        ));
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pipelines WHERE id = $1)")
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    if !exists {
        return Err(ApiError(StatusCode::NOT_FOUND, "pipeline not found".into()));
    }
    Ok(Json(
        sqlx::query_as(
            "SELECT * FROM logs WHERE pipeline_id = $1 AND id > $2 ORDER BY id LIMIT $3",
        )
        .bind(id)
        .bind(page.after)
        .bind(page.limit)
        .fetch_all(&state.pool)
        .await?,
    ))
}
