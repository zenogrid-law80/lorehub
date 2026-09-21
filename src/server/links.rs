//! Link policy and durable creation progress, separate from Lore's link definitions.
use super::{
    api::{
        self, AddRepositoryLink, ApiError, AppState, require_repository_access, user_access_token,
    },
    auth::AuthSession,
    repositories::RepositoryLinks,
    repository_access, triggers,
};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub(super) fn enabled() -> bool {
    true
}

#[derive(Serialize, sqlx::FromRow)]
pub(super) struct Operation {
    id: Uuid,
    #[serde(skip)]
    root_resource_id: String,
    #[serde(skip)]
    requested_by: Option<Uuid>,
    request: String,
    status: String,
    stage: String,
    source_ready: bool,
    source_path_created: bool,
    result_revision: Option<String>,
    error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl IntoResponse for Operation {
    fn into_response(self) -> Response {
        let status = match self.status.as_str() {
            "succeeded" => StatusCode::OK,
            "running" => StatusCode::ACCEPTED,
            _ => StatusCode::CONFLICT,
        };
        // Preserve the original successful create response's revision field.
        let revision = self.result_revision.clone();
        let mut value = json!(self);
        value["revision"] = json!(revision);
        (status, Json(value)).into_response()
    }
}

#[derive(Deserialize)]
pub(super) struct BranchQuery {
    branch: String,
}

pub(super) async fn operations(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Query(query): Query<BranchQuery>,
) -> Result<Json<Vec<Operation>>, ApiError> {
    let resource = require_repository_access(&state.pool, &name, &session).await?;
    Ok(Json(sqlx::query_as("SELECT * FROM repository_link_operations WHERE root_resource_id=$1 AND root_branch=$2 ORDER BY created_at DESC LIMIT 30")
        .bind(resource).bind(query.branch).fetch_all(&state.pool).await?))
}

async fn operation(pool: &PgPool, id: Uuid) -> Result<Operation, ApiError> {
    sqlx::query_as("SELECT * FROM repository_link_operations WHERE id=$1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "link operation not found".into()))
}

pub(super) async fn stage(
    pool: &PgPool,
    id: Uuid,
    stage: &str,
    ready: bool,
    created: bool,
) -> Result<(), ApiError> {
    sqlx::query("UPDATE repository_link_operations SET stage=$2,source_ready=source_ready OR $3,source_path_created=source_path_created OR $4,updated_at=now() WHERE id=$1")
        .bind(id).bind(stage).bind(ready).bind(created).execute(pool).await?;
    Ok(())
}

pub(super) async fn create(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Json(input): Json<AddRepositoryLink>,
) -> Result<Operation, ApiError> {
    let resource = require_repository_access(&state.pool, &name, &session).await?;
    require_repository_access(&state.pool, &input.source_repository, &session).await?;
    validate(&input)?;
    let request = serde_json::to_string(&input)
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "invalid link request".into()))?;
    let inserted = sqlx::query("INSERT INTO repository_link_operations(id,root_resource_id,root_branch,requested_by,request) VALUES($1,$2,$3,$4,$5) ON CONFLICT(id) DO NOTHING")
        .bind(input.operation_id).bind(&resource).bind(&input.branch).bind(session.user.id).bind(&request).execute(&state.pool).await?.rows_affected();
    if inserted == 0 {
        let existing = operation(&state.pool, input.operation_id).await?;
        if existing.root_resource_id != resource
            || existing.requested_by != Some(session.user.id)
            || existing.request != request
        {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "operation ID is already used by a different request".into(),
            ));
        }
        return Ok(existing);
    }
    run(&state, &session, name, input, false, false).await
}

fn validate(input: &AddRepositoryLink) -> Result<(), ApiError> {
    use crate::ci::config::valid_relative_path;
    if input.path.len() > 4096
        || !valid_relative_path(&input.path)
        || input.source_path.len() > 4096
        || !(input.source_path == "." || valid_relative_path(&input.source_path))
        || input.expected_revision.len() != 64
        || !input
            .expected_revision
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
        || [&input.branch, &input.source_branch]
            .iter()
            .any(|b| b.is_empty() || b.len() > 255 || b.chars().any(char::is_control))
    {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "invalid link path, branch or revision".into(),
        ));
    }
    Ok(())
}

pub(super) async fn retry(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path((name, id)): Path<(String, Uuid)>,
) -> Result<Operation, ApiError> {
    let resource = require_repository_access(&state.pool, &name, &session).await?;
    let previous = operation(&state.pool, id).await?;
    if previous.root_resource_id != resource {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            "link operation not found".into(),
        ));
    }
    let mut input: AddRepositoryLink = serde_json::from_str(&previous.request).map_err(|_| {
        ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            "invalid stored link request".into(),
        )
    })?;
    require_repository_access(&state.pool, &input.source_repository, &session).await?;
    if previous.status == "succeeded" {
        return Ok(previous);
    }
    // Do not replay an in-flight request. A stopped coordinator leaves a recoverable record.
    let claimed = sqlx::query("UPDATE repository_link_operations SET status='running',error=NULL,updated_at=now() WHERE id=$1 AND (status IN ('failed','partial') OR (status='running' AND updated_at < now()-interval '30 minutes'))")
        .bind(id).execute(&state.pool).await?.rows_affected();
    if claimed == 0 {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "link operation is still running".into(),
        ));
    }
    // A retry is an explicit request to reconcile the latest root, under its branch lock.
    input.operation_id = id;
    run(&state, &session, name, input, previous.source_ready, true).await
}

async fn run(
    state: &AppState,
    session: &AuthSession,
    name: String,
    input: AddRepositoryLink,
    ready: bool,
    retry: bool,
) -> Result<Operation, ApiError> {
    let id = input.operation_id;
    match api::add_repository_link(state, session, name, input, ready, retry).await {
        Ok(result) => {
            sqlx::query("UPDATE repository_link_operations SET status='succeeded',stage='complete',result_revision=$2,source_path_created=source_path_created OR $3,error=NULL,updated_at=now() WHERE id=$1")
                .bind(id).bind(result.revision).bind(result.source_path_created).execute(&state.pool).await?;
        }
        Err(error) => {
            sqlx::query("UPDATE repository_link_operations SET status=CASE WHEN source_ready THEN 'partial' ELSE 'failed' END,error=$2,updated_at=now() WHERE id=$1")
                .bind(id).bind(error.1).execute(&state.pool).await?;
        }
    }
    operation(&state.pool, id).await
}

pub(super) async fn save_policy(
    tx: &mut Transaction<'_, Postgres>,
    resource: &str,
    branch: &str,
    path: &str,
    auto: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO repository_link_policies(root_resource_id,root_branch,link_path,auto_update,last_success_at) VALUES($1,$2,$3,$4,now()) ON CONFLICT(root_resource_id,root_branch,link_path) DO UPDATE SET auto_update=$4,last_success_at=now(),updated_at=now(),last_error=NULL")
        .bind(resource).bind(branch).bind(path).bind(auto).execute(&mut **tx).await?;
    Ok(())
}

#[derive(Deserialize)]
pub(super) struct Policy {
    branch: String,
    path: String,
    expected_revision: String,
    auto_update: bool,
}

pub(super) async fn policy(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Json(input): Json<Policy>,
) -> Result<StatusCode, ApiError> {
    let resource = require_repository_access(&state.pool, &name, &session).await?;
    let token = user_access_token(&state, &session).await?;
    let backend = state.repositories.storage_backend(&name, &token).await?;
    let mut tx = state.pool.begin().await?;
    triggers::acquire_link_branch_lock(&mut tx, &resource, &input.branch).await?;
    let links = state
        .repositories
        .links_on(&name, &input.branch, backend, &token)
        .await?;
    if links.revision != input.expected_revision
        || !links.links.iter().any(|l| l.path == input.path)
    {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "link changed; refresh before changing its policy".into(),
        ));
    }
    sqlx::query("INSERT INTO repository_link_policies(root_resource_id,root_branch,link_path,auto_update) VALUES($1,$2,$3,$4) ON CONFLICT(root_resource_id,root_branch,link_path) DO UPDATE SET auto_update=$4,updated_at=now()")
        .bind(&resource).bind(&input.branch).bind(&input.path).bind(input.auto_update).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn summary(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
) -> Result<Json<Value>, ApiError> {
    let ids = repository_access::resource_ids(&state.pool, &session.user.id.to_string()).await?;
    let rows: Vec<(String,String,i64)> = sqlx::query_as("SELECT root_resource_id,root_branch,count(*) FROM repository_link_dependencies WHERE root_resource_id=ANY($1) GROUP BY root_resource_id,root_branch ORDER BY root_resource_id,root_branch")
        .bind(&ids).fetch_all(&state.pool).await?;
    Ok(Json(json!(
        rows.into_iter()
            .map(|(id, branch, count)| json!({"resource_id":id,"branch":branch,"count":count}))
            .collect::<Vec<_>>()
    )))
}

#[derive(sqlx::FromRow)]
struct LinkPolicy {
    link_path: String,
    auto_update: bool,
    last_success_at: Option<DateTime<Utc>>,
    last_error: Option<String>,
}

pub(super) async fn describe(
    state: &AppState,
    session: &AuthSession,
    resource: &str,
    links: RepositoryLinks,
    token: &str,
) -> Result<Value, ApiError> {
    let policies: Vec<LinkPolicy>=sqlx::query_as("SELECT link_path,auto_update,last_success_at,last_error FROM repository_link_policies WHERE root_resource_id=$1 AND root_branch=$2")
        .bind(resource).bind(&links.branch).fetch_all(&state.pool).await?;
    let allowed =
        repository_access::resource_ids(&state.pool, &session.user.id.to_string()).await?;
    let repositories = state.repositories.list(token).await?;
    let mut heads = std::collections::HashMap::new();
    let mut values = Vec::new();
    for link in links.links {
        let source = repositories.iter().find(|r| {
            r.id.trim_start_matches("urc-") == link.source_repository_id.trim_start_matches("urc-")
        });
        let mut branch_name = None;
        let mut latest = None;
        if let Some(source) = source.filter(|s| {
            allowed
                .iter()
                .any(|id| id.trim_start_matches("urc-") == s.id.trim_start_matches("urc-"))
        }) {
            if !heads.contains_key(&source.id) {
                heads.insert(
                    source.id.clone(),
                    state
                        .repositories
                        .branches_on(&source.name, source.storage_backend, token)
                        .await
                        .ok(),
                );
            }
            if let Some(Some(branches)) = heads.get(&source.id)
                && let Some(branch) = branches.iter().find(|b| b.id == link.source_branch_id)
            {
                branch_name = Some(branch.name.clone());
                latest = Some(branch.revision.clone());
            }
        }
        let policy = policies.iter().find(|p| p.link_path == link.path);
        let current = latest
            .as_deref()
            .is_some_and(|revision| revision.eq_ignore_ascii_case(&link.source_revision));
        let error = policy
            .and_then(|p| p.last_error.as_deref())
            .filter(|_| !current);
        let mut value = serde_json::to_value(&link).map_err(|_| {
            ApiError(
                StatusCode::INTERNAL_SERVER_ERROR,
                "cannot describe link".into(),
            )
        })?;
        let object = value.as_object_mut().expect("link is an object");
        object.insert("source_branch_name".into(), json!(branch_name));
        object.insert("latest_revision".into(), json!(latest));
        object.insert(
            "auto_update".into(),
            json!(policy.is_none_or(|p| p.auto_update)),
        );
        object.insert(
            "status".into(),
            json!(if current {
                "current"
            } else if error.is_some() {
                "failed"
            } else if latest.is_some() {
                "outdated"
            } else {
                "unknown"
            }),
        );
        object.insert("last_error".into(), json!(error));
        object.insert(
            "last_success_at".into(),
            json!(policy.and_then(|p| p.last_success_at)),
        );
        values.push(value);
    }
    Ok(json!({"branch":links.branch,"revision":links.revision,"links":values}))
}
