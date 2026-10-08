//! Shared presentation state; moving nodes never edits CI definitions or queues work.
use std::collections::BTreeMap;

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header::CACHE_CONTROL},
};
use serde::{Deserialize, Serialize};
use sqlx::types::Json as DbJson;

use super::{
    api::{self, ApiError, AppState},
    auth::AuthSession,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Point {
    x: f64,
    y: f64,
}

#[derive(Deserialize)]
pub(super) struct Scope {
    branch: String,
    graph: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Change {
    branch: String,
    graph: String,
    #[serde(default)]
    positions: BTreeMap<String, Option<Point>>,
    #[serde(default)]
    reset: bool,
}

#[derive(Default, Serialize)]
pub(super) struct Layout {
    positions: BTreeMap<String, Point>,
}

fn bad(message: &str) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, message.into())
}

fn validate_scope(branch: &str, graph: &str) -> Result<(), ApiError> {
    if branch.is_empty()
        || branch.len() > 255
        || branch.chars().any(char::is_control)
        || graph.len() > 512
    {
        return Err(bad("invalid graph layout scope"));
    }
    let parts: Vec<Option<String>> =
        serde_json::from_str(graph).map_err(|_| bad("invalid graph layout scope"))?;
    match parts.as_slice() {
        [Some(kind)] if kind == "overview" => Ok(()),
        [Some(kind), pipeline]
            if kind == "detail"
                && pipeline
                    .as_ref()
                    .is_none_or(|name| !name.is_empty() && !name.chars().any(char::is_control)) =>
        {
            Ok(())
        }
        _ => Err(bad("invalid graph layout scope")),
    }
}

pub(super) async fn read(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Query(scope): Query<Scope>,
) -> Result<(HeaderMap, Json<Layout>), ApiError> {
    let resource = api::require_repository_access(&state.pool, &name, &session).await?;
    validate_scope(&scope.branch, &scope.graph)?;
    let positions: Option<DbJson<BTreeMap<String, Point>>> = sqlx::query_scalar(
        "SELECT positions FROM ci_graph_layouts WHERE resource_id=$1 AND branch=$2 AND graph=$3",
    )
    .bind(resource)
    .bind(scope.branch)
    .bind(scope.graph)
    .fetch_optional(&state.pool)
    .await?;
    let mut headers = HeaderMap::new();
    headers.insert(CACHE_CONTROL, "no-store".parse().unwrap());
    Ok((
        headers,
        Json(Layout {
            positions: positions.map(|p| p.0).unwrap_or_default(),
        }),
    ))
}

pub(super) async fn save(
    State(state): State<AppState>,
    Extension(session): Extension<AuthSession>,
    Path(name): Path<String>,
    Json(input): Json<Change>,
) -> Result<Json<Layout>, ApiError> {
    let resource = api::require_repository_access(&state.pool, &name, &session).await?;
    if api::repository_is_link_source(&state.pool, &resource).await? {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "CI settings are unavailable for Lore link source repositories.".into(),
        ));
    }
    validate_scope(&input.branch, &input.graph)?;
    if input.positions.len() > 2000 || (input.reset && !input.positions.is_empty()) {
        return Err(bad("invalid layout update"));
    }
    for (key, point) in &input.positions {
        if key.is_empty()
            || key.len() > 512
            || key.chars().any(char::is_control)
            || point.as_ref().is_some_and(|p| {
                !p.x.is_finite()
                    || !p.y.is_finite()
                    || !(0.0..=50000.0).contains(&p.x)
                    || !(0.0..=50000.0).contains(&p.y)
            })
        {
            return Err(bad("invalid node position"));
        }
    }
    let mut tx = state.pool.begin().await?;
    sqlx::query("INSERT INTO ci_graph_layouts(resource_id,branch,graph) VALUES($1,$2,$3) ON CONFLICT DO NOTHING")
        .bind(&resource).bind(&input.branch).bind(&input.graph).execute(&mut *tx).await?;
    let DbJson(mut positions): DbJson<BTreeMap<String, Point>> = sqlx::query_scalar(
        "SELECT positions FROM ci_graph_layouts WHERE resource_id=$1 AND branch=$2 AND graph=$3 FOR UPDATE")
        .bind(&resource).bind(&input.branch).bind(&input.graph).fetch_one(&mut *tx).await?;
    if input.reset {
        positions.clear();
    }
    // Merge under a row lock so simultaneous edits to different nodes are retained.
    for (key, point) in input.positions {
        if let Some(point) = point {
            positions.insert(key, point);
        } else {
            positions.remove(&key);
        }
    }
    if positions.len() > 2000
        || serde_json::to_vec(&positions)
            .map_err(api::internal_error)?
            .len()
            > 240 * 1024
    {
        return Err(bad("graph layout is too large; reset its layout first"));
    }
    let storage_size: i32 = sqlx::query_scalar("SELECT octet_length($1::jsonb::text)")
        .bind(DbJson(&positions))
        .fetch_one(&mut *tx)
        .await?;
    if storage_size > 256 * 1024 {
        return Err(bad("graph layout is too large; reset its layout first"));
    }
    sqlx::query("UPDATE ci_graph_layouts SET positions=$4,updated_by=$5,updated_at=now() WHERE resource_id=$1 AND branch=$2 AND graph=$3")
        .bind(&resource).bind(&input.branch).bind(&input.graph).bind(DbJson(&positions)).bind(session.user.id)
        .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(Layout { positions }))
}
